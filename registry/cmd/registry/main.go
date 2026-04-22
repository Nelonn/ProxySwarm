package main

import (
	"context"
	"crypto/rand"
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
	"sort"
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
const defaultRefreshIntervalSeconds = int32(3600)
const modeSharedPort = "1"
const modeSplitPorts = "2"
const registryMasterKeyHeader = "x-registry-master-key"

type registryStore struct {
	mu       sync.Mutex
	path     string
	services map[string]*pb.RegistryService
}

type persistedRegistryState struct {
	Services []*pb.RegistryService `json:"services"`
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
			if wrappedServer.IsGrpcWebRequest(req) {
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
		services, err := store.list()
		if err != nil {
			writeErrorJSON(res, http.StatusInternalServerError, err.Error())
			return
		}
		account, ok := findAccountByToken(services, token)
		if !ok {
			writeErrorJSON(res, http.StatusForbidden, "invalid token")
			return
		}
		links := buildSubscriptionLinks(services, account)
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

func findAccountByToken(services []*pb.RegistryService, token string) (*pb.Account, bool) {
	for _, service := range services {
		if service == nil || !service.Enabled {
			continue
		}
		for _, account := range service.Accounts {
			if account == nil {
				continue
			}
			if strings.TrimSpace(account.Token) == token {
				return account, true
			}
		}
	}
	return nil, false
}

func buildSubscriptionLinks(services []*pb.RegistryService, account *pb.Account) []string {
	links := make([]string, 0)
	seen := make(map[string]struct{})
	for _, service := range services {
		if service == nil || !service.Enabled {
			continue
		}
		for _, templateLink := range service.TemplateLinks {
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
		http.NotFound(res, req)
	})
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

func (s *registryManagementServer) ListServices(ctx context.Context, _ *pb.RegistryListServicesRequest) (*pb.RegistryListServicesResponse, error) {
	if err := s.authorize(ctx); err != nil {
		return nil, err
	}
	services, err := s.store.list()
	if err != nil {
		return nil, status.Error(codes.Internal, err.Error())
	}
	return &pb.RegistryListServicesResponse{Services: services}, nil
}

func (s *registryManagementServer) UpsertService(ctx context.Context, req *pb.RegistryUpsertServiceRequest) (*pb.RegistryUpsertServiceResponse, error) {
	if err := s.authorize(ctx); err != nil {
		return nil, err
	}
	if req == nil || req.Service == nil {
		return nil, status.Error(codes.InvalidArgument, "service is required")
	}
	service, err := s.store.upsert(req.Service)
	if err != nil {
		return nil, status.Error(codes.InvalidArgument, err.Error())
	}
	return &pb.RegistryUpsertServiceResponse{Service: service}, nil
}

func (s *registryManagementServer) DeleteService(ctx context.Context, req *pb.RegistryDeleteServiceRequest) (*pb.RegistryDeleteServiceResponse, error) {
	if err := s.authorize(ctx); err != nil {
		return nil, err
	}
	if req == nil || strings.TrimSpace(req.Id) == "" {
		return nil, status.Error(codes.InvalidArgument, "id is required")
	}
	if err := s.store.delete(req.Id); err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return nil, status.Error(codes.NotFound, "service not found")
		}
		return nil, status.Error(codes.Internal, err.Error())
	}
	return &pb.RegistryDeleteServiceResponse{}, nil
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
		path:     defaultStorePath(),
		services: make(map[string]*pb.RegistryService),
	}
	if err := store.load(); err != nil {
		return nil, err
	}
	return store, nil
}

func defaultStorePath() string {
	configDir, err := os.UserConfigDir()
	if err == nil && strings.TrimSpace(configDir) != "" {
		return filepath.Join(configDir, "proxyswarm", "registry_services.json")
	}
	return "registry_services.json"
}

func (s *registryStore) list() ([]*pb.RegistryService, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	out := make([]*pb.RegistryService, 0, len(s.services))
	for _, service := range s.services {
		out = append(out, cloneRegistryService(service))
	}
	sort.Slice(out, func(i, j int) bool {
		left := strings.ToLower(strings.TrimSpace(out[i].Name))
		right := strings.ToLower(strings.TrimSpace(out[j].Name))
		if left == right {
			return out[i].Id < out[j].Id
		}
		return left < right
	})
	return out, nil
}

func (s *registryStore) upsert(in *pb.RegistryService) (*pb.RegistryService, error) {
	if in == nil {
		return nil, errors.New("service is required")
	}

	name := strings.TrimSpace(in.Name)
	if name == "" {
		return nil, errors.New("name is required")
	}
	subscriptionURL := strings.TrimSpace(in.SubscriptionUrl)
	if subscriptionURL == "" {
		return nil, errors.New("subscription_url is required")
	}

	id := strings.TrimSpace(in.Id)
	if id == "" {
		randomID, err := newID()
		if err != nil {
			return nil, fmt.Errorf("failed to generate id: %w", err)
		}
		id = randomID
	}

	interval := in.RefreshIntervalSeconds
	if interval <= 0 {
		interval = defaultRefreshIntervalSeconds
	}

	service := &pb.RegistryService{
		Id:                     id,
		Name:                   name,
		SubscriptionUrl:        subscriptionURL,
		Enabled:                in.Enabled,
		RefreshIntervalSeconds: interval,
		UpdatedAtUnix:          time.Now().Unix(),
		Accounts:               cloneRegistryAccounts(in.Accounts),
		TemplateLinks:          cloneRegistryTemplateLinks(in.TemplateLinks),
	}

	s.mu.Lock()
	defer s.mu.Unlock()

	s.services[id] = cloneRegistryService(service)
	if err := s.persistLocked(); err != nil {
		return nil, err
	}
	return cloneRegistryService(service), nil
}

func (s *registryStore) delete(id string) error {
	trimmed := strings.TrimSpace(id)
	if trimmed == "" {
		return errors.New("id is required")
	}

	s.mu.Lock()
	defer s.mu.Unlock()

	if _, exists := s.services[trimmed]; !exists {
		return os.ErrNotExist
	}

	delete(s.services, trimmed)
	return s.persistLocked()
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

	for _, service := range state.Services {
		if service == nil {
			continue
		}
		if strings.TrimSpace(service.Id) == "" ||
			strings.TrimSpace(service.Name) == "" ||
			strings.TrimSpace(service.SubscriptionUrl) == "" {
			continue
		}
		if service.RefreshIntervalSeconds <= 0 {
			service.RefreshIntervalSeconds = defaultRefreshIntervalSeconds
		}
		s.services[service.Id] = cloneRegistryService(service)
	}
	return nil
}

func (s *registryStore) persistLocked() error {
	state := persistedRegistryState{
		Services: make([]*pb.RegistryService, 0, len(s.services)),
	}
	for _, service := range s.services {
		state.Services = append(state.Services, cloneRegistryService(service))
	}

	sort.Slice(state.Services, func(i, j int) bool {
		left := strings.ToLower(strings.TrimSpace(state.Services[i].Name))
		right := strings.ToLower(strings.TrimSpace(state.Services[j].Name))
		if left == right {
			return state.Services[i].Id < state.Services[j].Id
		}
		return left < right
	})

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

func newID() (string, error) {
	var bytes [16]byte
	if _, err := rand.Read(bytes[:]); err != nil {
		return "", err
	}
	return hex.EncodeToString(bytes[:]), nil
}

func hashMasterKey(masterKey string) string {
	if strings.TrimSpace(masterKey) == "" {
		return ""
	}
	sum := sha256.Sum256([]byte(masterKey))
	return hex.EncodeToString(sum[:])
}

func cloneRegistryService(in *pb.RegistryService) *pb.RegistryService {
	if in == nil {
		return nil
	}
	return &pb.RegistryService{
		Id:                     in.Id,
		Name:                   in.Name,
		SubscriptionUrl:        in.SubscriptionUrl,
		Enabled:                in.Enabled,
		RefreshIntervalSeconds: in.RefreshIntervalSeconds,
		UpdatedAtUnix:          in.UpdatedAtUnix,
		Accounts:               cloneRegistryAccounts(in.Accounts),
		TemplateLinks:          cloneRegistryTemplateLinks(in.TemplateLinks),
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
