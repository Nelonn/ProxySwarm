package main

import (
	"context"
	"crypto/sha256"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"log"
	"net"
	"net/http"
	"os"
	"path/filepath"
	"proxyswarm/registry/internal/pb"
	"strings"
	"sync"
	"time"

	"github.com/improbable-eng/grpc-web/go/grpcweb"
	"google.golang.org/grpc"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/metadata"
	"google.golang.org/grpc/status"
)

const defaultListenAddr = ":9191"
const defaultManageListenAddr = ":9291"
const modeSharedPort = "1"
const modeSplitPorts = "2"
const registryMasterKeyHeader = "x-registry-master-key"

var defaultDataDir = "./data"

type logLevel int

const (
	logLevelDebug logLevel = iota
	logLevelInfo
	logLevelWarn
	logLevelError
)

type registryStore struct {
	mu        sync.Mutex
	path      string
	config    *pb.RegistryServiceConfig
	updatedAt int64
}

type telemetryStore struct {
	mu   sync.Mutex
	path string
}

type subscriptionTelemetryEntry struct {
	Time  int64  `json:"time_unix"`
	UserAgent string `json:"user_agent"`
	UserId    string `json:"user_id"`
	Ip        string `json:"ip"`
}

type persistedRegistryState struct {
	Config        *pb.RegistryServiceConfig `json:"config"`
	UpdatedAtUnix int64                     `json:"updated_at_unix"`
}

type registryManagementServer struct {
	pb.UnimplementedRegistryManagementServiceServer
	store         *registryStore
	masterKeyHash string
}

type loggingResponseWriter struct {
	http.ResponseWriter
	statusCode int
}

func (w *loggingResponseWriter) WriteHeader(statusCode int) {
	w.statusCode = statusCode
	w.ResponseWriter.WriteHeader(statusCode)
}

func main() {
	listenAddr := strings.TrimSpace(os.Getenv("REGISTRY_LISTEN"))
	if listenAddr == "" {
		listenAddr = defaultListenAddr
	}
	manageListenAddr := strings.TrimSpace(os.Getenv("REGISTRY_MANAGE_LISTEN"))
	if manageListenAddr == "" {
		manageListenAddr = defaultManageListenAddr
	}
	mode := strings.TrimSpace(os.Getenv("REGISTRY_MODE"))
	if mode == "" {
		mode = modeSharedPort
	}
	if mode != modeSharedPort && mode != modeSplitPorts {
		log.Fatalf("invalid REGISTRY_MODE=%q (supported: %s, %s)", mode, modeSharedPort, modeSplitPorts)
	}
	masterKeyHash := hashMasterKey(strings.TrimSpace(os.Getenv("PS_MASTER_KEY")))
	if masterKeyHash == "" {
		log.Fatal("PS_MASTER_KEY is required")
	}

	store, err := newRegistryStore()
	if err != nil {
		log.Fatalf("failed to initialize store: %v", err)
	}
	telemetry := newTelemetryStore()

	server := grpc.NewServer(
		grpc.MaxRecvMsgSize(32*1024*1024),
		grpc.MaxSendMsgSize(32*1024*1024),
	)
	pb.RegisterRegistryManagementServiceServer(server, &registryManagementServer{
		store:         store,
		masterKeyHash: masterKeyHash,
	})

	wrappedServer := grpcweb.WrapServer(
		server,
		grpcweb.WithOriginFunc(func(origin string) bool { return true }),
	)
	userHandler := makeUserAPIHandler(store, telemetry)
	manageHandler := makeManageAPIHandler(wrappedServer)

	if mode == modeSharedPort {
		lis, err := net.Listen("tcp", listenAddr)
		if err != nil {
			log.Fatalf("failed to listen: %v", err)
		}
		handler := http.HandlerFunc(func(res http.ResponseWriter, req *http.Request) {
			if isManageRequest(req, wrappedServer) {
				manageHandler.ServeHTTP(res, req)
				return
			}
			userHandler.ServeHTTP(res, req)
		})
		httpServer := &http.Server{
			Addr:    listenAddr,
			Handler: withRequestLogging(handler),
		}

		logInfof("registry mode=%s user+manage listening on %s", mode, listenAddr)
		if err := httpServer.Serve(lis); err != nil && !errors.Is(err, http.ErrServerClosed) {
			log.Fatal(err)
		}
		return
	}

	userLis, err := net.Listen("tcp", listenAddr)
	if err != nil {
		log.Fatalf("failed to listen for user API: %v", err)
	}
	manageLis, err := net.Listen("tcp", manageListenAddr)
	if err != nil {
		log.Fatalf("failed to listen for manage API: %v", err)
	}

	userServer := &http.Server{
		Addr:    listenAddr,
		Handler: withRequestLogging(userHandler),
	}
	managementServer := &http.Server{
		Addr:    manageListenAddr,
		Handler: withRequestLogging(manageHandler),
	}

	errCh := make(chan error, 2)
	go func() {
		if err := userServer.Serve(userLis); err != nil && !errors.Is(err, http.ErrServerClosed) {
			errCh <- fmt.Errorf("user API server failed: %w", err)
		}
	}()
	go func() {
		if err := managementServer.Serve(manageLis); err != nil && !errors.Is(err, http.ErrServerClosed) {
			errCh <- fmt.Errorf("manage API server failed: %w", err)
		}
	}()

	logInfof("registry mode=%s user listening on %s, manage listening on %s", mode, listenAddr, manageListenAddr)
	if err := <-errCh; err != nil {
		log.Fatal(err)
	}
}

func currentLogLevel() logLevel {
	value := strings.TrimSpace(os.Getenv("PS_LOG_LEVEL"))
	if value == "" {
		value = strings.TrimSpace(os.Getenv("LOG_LEVEL"))
	}
	switch strings.ToUpper(value) {
	case "DEBUG":
		return logLevelDebug
	case "WARN", "WARNING":
		return logLevelWarn
	case "ERROR", "ERR":
		return logLevelError
	default:
		return logLevelInfo
	}
}

func logInfof(format string, args ...any) {
	if currentLogLevel() <= logLevelInfo {
		log.Printf(format, args...)
	}
}

func withRequestLogging(next http.Handler) http.Handler {
	return http.HandlerFunc(func(res http.ResponseWriter, req *http.Request) {
		start := time.Now()
		writer := &loggingResponseWriter{
			ResponseWriter: res,
			statusCode:     http.StatusOK,
		}
		next.ServeHTTP(writer, req)
		logInfof(
			"[http] method=%s path=%s status=%d duration=%s remote=%s ua=%q",
			req.Method,
			req.URL.RequestURI(),
			writer.statusCode,
			time.Since(start),
			req.RemoteAddr,
			req.UserAgent(),
		)
	})
}

func makeUserAPIHandler(store *registryStore, telemetry *telemetryStore) http.Handler {
	mux := http.NewServeMux()
	mux.HandleFunc("/v1/subscription", func(res http.ResponseWriter, req *http.Request) {
		setUserCORSHeaders(res)
		if req.Method == http.MethodOptions {
			res.WriteHeader(http.StatusNoContent)
			return
		}
		if req.Method != http.MethodGet {
			writeErrorJSON(res, http.StatusMethodNotAllowed, "method not allowed")
			return
		}
		var telemetryAccount *pb.Account
		defer func() {
			recordSubscriptionTelemetry(telemetry, req, telemetryAccount)
		}()
		token := strings.TrimSpace(req.URL.Query().Get("token"))
		if token == "" {
			writeErrorJSON(res, http.StatusForbidden, "invalid token")
			return
		}
		config, _, err := store.get()
		if err != nil {
			writeErrorJSON(res, http.StatusInternalServerError, err.Error())
			return
		}
		account, ok := findAccountByToken(config, token)
		telemetryAccount = account
		if !ok {
			writeErrorJSON(res, http.StatusForbidden, "invalid token")
			return
		}
		links := buildSubscriptionLinks(config, account)
		if len(links) == 0 {
			writeErrorJSON(res, http.StatusNotFound, "no templates available")
			return
		}
		res.Header().Set("Content-Type", "text/plain; charset=utf-8")
		res.Header().Set("subscription-userinfo", buildSubscriptionUserInfo(account))
		res.Header().Set("profile-title", buildProfileTitle(account))
		res.WriteHeader(http.StatusOK)
		_, _ = res.Write([]byte(strings.Join(links, "\n")))
	})
	return http.HandlerFunc(func(res http.ResponseWriter, req *http.Request) {
		switch req.URL.Path {
		case "/v1/subscription":
			mux.ServeHTTP(res, req)
			return
		default:
			http.NotFound(res, req)
		}
	})
}

func findAccountByToken(config *pb.RegistryServiceConfig, token string) (*pb.Account, bool) {
	if config == nil {
		return nil, false
	}
	for _, account := range config.Accounts {
		if account == nil {
			continue
		}
		if strings.TrimSpace(account.Token) == token {
			return account, true
		}
	}
	return nil, false
}

func buildSubscriptionLinks(config *pb.RegistryServiceConfig, account *pb.Account) []string {
	if config == nil {
		return nil
	}
	links := make([]string, 0)
	seen := make(map[string]struct{})
	for _, templateLink := range config.TemplateLinks {
		if templateLink == nil {
			continue
		}
		if !groupsIntersect(account.GetGroups(), templateLink.GetGroups()) {
			continue
		}
		template := strings.TrimSpace(templateLink.Template)
		if template == "" {
			continue
		}
		link := renderTemplateLink(template, account)
		if strings.TrimSpace(link) == "" {
			continue
		}
		if _, exists := seen[link]; exists {
			continue
		}
		seen[link] = struct{}{}
		links = append(links, link)
	}
	return links
}

func groupsIntersect(accountGroups []string, templateGroups []string) bool {
	accountSet := normalizeGroups(accountGroups)
	templateSet := normalizeGroups(templateGroups)
	if len(templateSet) == 0 {
		return true
	}
	for _, group := range accountSet {
		for _, candidate := range templateSet {
			if group == candidate {
				return true
			}
		}
	}
	return false
}

func normalizeGroups(values []string) []string {
	groups := make([]string, 0, len(values))
	seen := make(map[string]struct{}, len(values))
	for _, value := range values {
		value = strings.TrimSpace(value)
		if value == "" {
			continue
		}
		if _, ok := seen[value]; ok {
			continue
		}
		seen[value] = struct{}{}
		groups = append(groups, value)
	}
	return groups
}

func renderTemplateLink(template string, account *pb.Account) string {
	displayName := account.GetName()
	if displayName == "" {
		displayName = account.GetId()
	}
	replacer := strings.NewReplacer(
		"{{token}}", account.GetToken(),
		"{{name}}", displayName,
		"{{id}}", account.GetId(),
		"{token}", account.GetToken(),
		"{name}", displayName,
		"{id}", account.GetId(),
	)
	return replacer.Replace(template)
}

func buildSubscriptionUserInfo(account *pb.Account) string {
	if account == nil {
		return "upload=0; download=0; total=0; expire=0"
	}
	expire := account.GetExpiryTime()
	if expire < 0 {
		expire = 0
	}
	return fmt.Sprintf("upload=0; download=0; total=0; expire=%d", expire)
}

func buildProfileTitle(account *pb.Account) string {
	title := "ProxySwarm"
	if account != nil {
		if strings.TrimSpace(account.GetId()) != "" {
			title = account.GetId()
		}
	}
	return "base64:" + base64.StdEncoding.EncodeToString([]byte(title))
}

func newTelemetryStore() *telemetryStore {
	return &telemetryStore{path: defaultTelemetryPath()}
}

func defaultTelemetryPath() string {
	if dataDir := defaultDataRoot(); dataDir != "" {
		return filepath.Join(dataDir, "telemetry.json")
	}
	return "telemetry.json"
}

func recordSubscriptionTelemetry(store *telemetryStore, req *http.Request, account *pb.Account) {
	if store == nil || req == nil {
		return
	}
	now := time.Now().UTC()
	entry := subscriptionTelemetryEntry{
		Time:  now.Unix(),
		UserAgent: req.UserAgent(),
		Ip:        forwardedClientIP(req),
	}
	if account != nil {
		entry.UserId = strings.TrimSpace(account.GetId())
	}
	if err := store.append(entry); err != nil {
		logInfof("failed to record subscription telemetry: %v", err)
	}
}

func forwardedClientIP(req *http.Request) string {
	for _, header := range []string{"CF-Connecting-IP", "X-Real-IP", "X-Forwarded-For"} {
		value := strings.TrimSpace(req.Header.Get(header))
		if value == "" {
			continue
		}
		for _, part := range strings.Split(value, ",") {
			part = strings.TrimSpace(part)
			if part != "" {
				return strings.Trim(part, "\"")
			}
		}
	}
	if forwarded := strings.TrimSpace(req.Header.Get("Forwarded")); forwarded != "" {
		for _, item := range strings.Split(forwarded, ";") {
			key, value, ok := strings.Cut(strings.TrimSpace(item), "=")
			if !ok || !strings.EqualFold(strings.TrimSpace(key), "for") {
				continue
			}
			value = strings.Trim(strings.TrimSpace(value), "\"")
			if value != "" {
				return strings.Trim(value, "[]")
			}
		}
	}
	remote := strings.TrimSpace(req.RemoteAddr)
	if host, _, err := net.SplitHostPort(remote); err == nil {
		return host
	}
	return remote
}

func (s *telemetryStore) append(entry subscriptionTelemetryEntry) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	entries := make([]subscriptionTelemetryEntry, 0)
	data, err := os.ReadFile(s.path)
	if err != nil && !errors.Is(err, os.ErrNotExist) {
		return err
	}
	if len(data) > 0 {
		if err := json.Unmarshal(data, &entries); err != nil {
			return err
		}
	}
	entries = append(entries, entry)

	data, err = json.MarshalIndent(entries, "", "  ")
	if err != nil {
		return err
	}
	dir := filepath.Dir(s.path)
	if dir != "." && dir != "" {
		if err := os.MkdirAll(dir, 0o755); err != nil {
			return err
		}
	}
	tmpPath := s.path + ".tmp"
	if err := os.WriteFile(tmpPath, data, 0o600); err != nil {
		return err
	}
	return os.Rename(tmpPath, s.path)
}

func makeManageAPIHandler(wrappedServer *grpcweb.WrappedGrpcServer) http.Handler {
	return http.HandlerFunc(func(res http.ResponseWriter, req *http.Request) {
		setManageCORSHeaders(res)
		if req.Method == http.MethodOptions {
			res.WriteHeader(http.StatusNoContent)
			return
		}
		if wrappedServer.IsGrpcWebRequest(req) {
			wrappedServer.ServeHTTP(res, req)
			return
		}
		if strings.HasPrefix(req.URL.Path, "/proxyswarm.RegistryManagementService/") {
			http.Error(res, "gRPC-Web request required", http.StatusBadRequest)
			return
		}
		http.NotFound(res, req)
	})
}

func isManageRequest(req *http.Request, wrappedServer *grpcweb.WrappedGrpcServer) bool {
	if wrappedServer.IsGrpcWebRequest(req) || wrappedServer.IsAcceptableGrpcCorsRequest(req) {
		return true
	}
	return strings.HasPrefix(req.URL.Path, "/proxyswarm.RegistryManagementService/")
}

func setUserCORSHeaders(res http.ResponseWriter) {
	res.Header().Set("Access-Control-Allow-Origin", "*")
	res.Header().Set("Access-Control-Allow-Headers", "Content-Type")
	res.Header().Set("Access-Control-Allow-Methods", "GET, OPTIONS")
	setNoCacheHeaders(res)
}

func setManageCORSHeaders(res http.ResponseWriter) {
	res.Header().Set("Access-Control-Allow-Origin", "*")
	res.Header().Set("Access-Control-Allow-Headers", "Content-Type, X-Grpc-Web, X-User-Agent, X-Registry-Master-Key")
	res.Header().Set("Access-Control-Allow-Methods", "POST, OPTIONS, GET")
	setNoCacheHeaders(res)
}

func setNoCacheHeaders(res http.ResponseWriter) {
	res.Header().Set("Cache-Control", "private, no-store, no-cache, must-revalidate, max-age=0")
	res.Header().Set("CDN-Cache-Control", "no-store")
	res.Header().Set("Cloudflare-CDN-Cache-Control", "no-store")
	res.Header().Set("Pragma", "no-cache")
	res.Header().Set("Expires", "0")
}

func writeErrorJSON(res http.ResponseWriter, statusCode int, message string) {
	writeJSON(res, statusCode, map[string]any{"error": message})
}

func writeJSON(res http.ResponseWriter, statusCode int, payload any) {
	res.Header().Set("Content-Type", "application/json")
	res.WriteHeader(statusCode)
	_ = json.NewEncoder(res).Encode(payload)
}

func (s *registryManagementServer) UpdateConfig(ctx context.Context, req *pb.RegistryUpdateConfigRequest) (*pb.RegistryUpdateConfigResponse, error) {
	if err := s.authorize(ctx); err != nil {
		return nil, err
	}
	if req == nil || req.Config == nil {
		return nil, status.Error(codes.InvalidArgument, "config is required")
	}
	config, err := s.store.update(req.Config)
	if err != nil {
		return nil, status.Error(codes.InvalidArgument, err.Error())
	}
	return &pb.RegistryUpdateConfigResponse{Config: config}, nil
}

func (s *registryManagementServer) Status(ctx context.Context, _ *pb.RegistryStatusRequest) (*pb.RegistryStatusResponse, error) {
	if err := s.authorize(ctx); err != nil {
		return nil, err
	}
	config, updatedAtUnix, err := s.store.get()
	if err != nil {
		return nil, status.Error(codes.Internal, err.Error())
	}
	response := &pb.RegistryStatusResponse{
		UpdatedAtUnix: updatedAtUnix,
	}
	if config != nil {
		response.Configured = true
		response.Accounts = uint32(len(config.Accounts))
		response.TemplateLinks = uint32(len(config.TemplateLinks))
	}
	return response, nil
}

func (s *registryManagementServer) authorize(ctx context.Context) error {
	if strings.TrimSpace(s.masterKeyHash) == "" {
		return status.Error(codes.Unauthenticated, "unauthorized")
	}
	md, ok := metadata.FromIncomingContext(ctx)
	if !ok {
		return status.Error(codes.Unauthenticated, "unauthorized")
	}
	values := md.Get(registryMasterKeyHeader)
	if len(values) == 0 {
		return status.Error(codes.Unauthenticated, "unauthorized")
	}
	if strings.TrimSpace(values[0]) != s.masterKeyHash {
		return status.Error(codes.Unauthenticated, "unauthorized")
	}
	return nil
}

func newRegistryStore() (*registryStore, error) {
	store := &registryStore{
		path: defaultStorePath(),
	}
	if err := store.load(); err != nil {
		return nil, err
	}
	return store, nil
}

func defaultStorePath() string {
	if dataDir := defaultDataRoot(); dataDir != "" {
		return filepath.Join(dataDir, "registry_services.json")
	}
	return "registry_services.json"
}

func defaultDataRoot() string {
	if envDir := strings.TrimSpace(os.Getenv("PS_REGISTRY_DATA_DIR")); envDir != "" {
		return envDir
	}
	if buildDir := strings.TrimSpace(defaultDataDir); buildDir != "" {
		return buildDir
	}
	return ""
}

func (s *registryStore) get() (*pb.RegistryServiceConfig, int64, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	return cloneRegistryConfig(s.config), s.updatedAt, nil
}

func (s *registryStore) update(in *pb.RegistryServiceConfig) (*pb.RegistryServiceConfig, error) {
	if in == nil {
		return nil, errors.New("config is required")
	}

	config := &pb.RegistryServiceConfig{
		Accounts:      cloneRegistryAccounts(in.Accounts),
		TemplateLinks: cloneRegistryTemplateLinks(in.TemplateLinks),
	}

	s.mu.Lock()
	defer s.mu.Unlock()

	s.config = cloneRegistryConfig(config)
	s.updatedAt = time.Now().Unix()
	if err := s.persistLocked(); err != nil {
		return nil, err
	}
	return cloneRegistryConfig(config), nil
}

func (s *registryStore) load() error {
	data, err := os.ReadFile(s.path)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return nil
		}
		return err
	}

	var state persistedRegistryState
	if err := json.Unmarshal(data, &state); err != nil {
		return err
	}

	s.config = cloneRegistryConfig(state.Config)
	s.updatedAt = state.UpdatedAtUnix
	if s.config == nil {
		s.updatedAt = 0
	}
	return nil
}

func (s *registryStore) persistLocked() error {
	state := persistedRegistryState{
		Config:        cloneRegistryConfig(s.config),
		UpdatedAtUnix: s.updatedAt,
	}

	data, err := json.MarshalIndent(state, "", "  ")
	if err != nil {
		return err
	}

	dir := filepath.Dir(s.path)
	if dir != "." && dir != "" {
		if err := os.MkdirAll(dir, 0o755); err != nil {
			return err
		}
	}

	tmpPath := s.path + ".tmp"
	if err := os.WriteFile(tmpPath, data, 0o600); err != nil {
		return err
	}
	return os.Rename(tmpPath, s.path)
}

func hashMasterKey(masterKey string) string {
	if strings.TrimSpace(masterKey) == "" {
		return ""
	}
	sum := sha256.Sum256([]byte(masterKey))
	return hex.EncodeToString(sum[:])
}

func cloneRegistryConfig(in *pb.RegistryServiceConfig) *pb.RegistryServiceConfig {
	if in == nil {
		return nil
	}
	return &pb.RegistryServiceConfig{
		Accounts:      cloneRegistryAccounts(in.Accounts),
		TemplateLinks: cloneRegistryTemplateLinks(in.TemplateLinks),
	}
}

func cloneRegistryAccounts(accounts []*pb.Account) []*pb.Account {
	if len(accounts) == 0 {
		return nil
	}
	cloned := make([]*pb.Account, 0, len(accounts))
	for _, account := range accounts {
		if account == nil {
			continue
		}
		cloned = append(cloned, &pb.Account{
			Id:         account.Id,
			AllowedIps: append([]string(nil), account.AllowedIps...),
			Groups:     append([]string(nil), account.Groups...),
			ExpiryTime: account.ExpiryTime,
			Token:      account.Token,
			Name:       account.Name,
		})
	}
	return cloned
}

func cloneRegistryTemplateLinks(links []*pb.RegistryTemplateLink) []*pb.RegistryTemplateLink {
	if len(links) == 0 {
		return nil
	}
	cloned := make([]*pb.RegistryTemplateLink, 0, len(links))
	for _, link := range links {
		if link == nil {
			continue
		}
		cloned = append(cloned, &pb.RegistryTemplateLink{
			NodeId:      link.NodeId,
			NodeName:    link.NodeName,
			InboundId:   link.InboundId,
			InboundName: link.InboundName,
			Protocol:    link.Protocol,
			Template:    link.Template,
			Groups:      append([]string(nil), link.Groups...),
		})
	}
	return cloned
}
