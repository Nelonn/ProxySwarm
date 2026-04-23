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

var defaultDataDir = "/var/proxyswarm/registry"

type registryStore struct {
	mu        sync.Mutex
	path      string
	config    *pb.RegistryServiceConfig
	updatedAt int64
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
	userHandler := makeUserAPIHandler(store)
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
			Handler: handler,
		}

		log.Printf("registry mode=%s user+manage listening on %s", mode, listenAddr)
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
		Handler: userHandler,
	}
	managementServer := &http.Server{
		Addr:    manageListenAddr,
		Handler: manageHandler,
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

	log.Printf("registry mode=%s user listening on %s, manage listening on %s", mode, listenAddr, manageListenAddr)
	if err := <-errCh; err != nil {
		log.Fatal(err)
	}
}

func makeUserAPIHandler(store *registryStore) http.Handler {
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

func renderTemplateLink(template string, account *pb.Account) string {
	replacer := strings.NewReplacer(
		"{{token}}", account.GetToken(),
		"{{name}}", account.GetName(),
		"{{id}}", account.GetId(),
		"{token}", account.GetToken(),
		"{name}", account.GetName(),
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
		if strings.TrimSpace(account.GetName()) != "" {
			title = account.GetName()
		} else if strings.TrimSpace(account.GetId()) != "" {
			title = account.GetId()
		}
	}
	return "base64:" + base64.StdEncoding.EncodeToString([]byte(title))
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
}

func setManageCORSHeaders(res http.ResponseWriter) {
	res.Header().Set("Access-Control-Allow-Origin", "*")
	res.Header().Set("Access-Control-Allow-Headers", "Content-Type, X-Grpc-Web, X-User-Agent, X-Registry-Master-Key")
	res.Header().Set("Access-Control-Allow-Methods", "POST, OPTIONS, GET")
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
			Name:       account.Name,
			AllowedIps: append([]string(nil), account.AllowedIps...),
			ExpiryTime: account.ExpiryTime,
			Token:      account.Token,
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
		})
	}
	return cloned
}
