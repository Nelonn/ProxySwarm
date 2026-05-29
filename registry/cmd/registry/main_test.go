package main

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"

	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/metadata"
	"google.golang.org/grpc/status"
	"proxyswarm/registry/internal/pb"
)

func TestFindAccountByToken(t *testing.T) {
	config := &pb.RegistryServiceConfig{
		Accounts: []*pb.Account{
			{Id: "a1", Token: "tok-1"},
			{Id: "a2", Token: "tok-2"},
		},
	}

	account, ok := findAccountByToken(config, "tok-2")
	if !ok {
		t.Fatal("expected token to be found")
	}
	if account.GetId() != "a2" {
		t.Fatalf("unexpected account id: %q", account.GetId())
	}

	if _, ok := findAccountByToken(config, "missing"); ok {
		t.Fatal("expected missing token to be ignored")
	}
}

func TestBuildSubscriptionLinks_RendersAndDeduplicates(t *testing.T) {
	account := &pb.Account{Id: "acc-1", Token: "tok-1"}
	config := &pb.RegistryServiceConfig{
		TemplateLinks: []*pb.RegistryTemplateLink{
			{Template: "vless://{{token}}@host:443#{{name}}"},
			{Template: "vless://{{token}}@host:443#{{name}}"},
			{Template: "trojan://{token}@node:443#{id}"},
		},
	}

	links := buildSubscriptionLinks(config, account)
	if len(links) != 2 {
		t.Fatalf("expected 2 unique links, got %d: %#v", len(links), links)
	}
	if links[0] != "vless://tok-1@host:443#acc-1" {
		t.Fatalf("unexpected first link: %q", links[0])
	}
	if links[1] != "trojan://tok-1@node:443#acc-1" {
		t.Fatalf("unexpected second link: %q", links[1])
	}
}

func TestSubscriptionEndpoint_InvalidTokenReturns403(t *testing.T) {
	handler := makeUserAPIHandler(testStore(&pb.RegistryServiceConfig{
		Accounts: []*pb.Account{
			{Id: "a1", Token: "tok-1"},
		},
		TemplateLinks: []*pb.RegistryTemplateLink{
			{Template: "vless://{{token}}@host:443"},
		},
	}), nil)

	rec := httptest.NewRecorder()
	req := httptest.NewRequest(http.MethodGet, "/v1/subscription?token=wrong", nil)
	handler.ServeHTTP(rec, req)

	if rec.Code != http.StatusForbidden {
		t.Fatalf("expected 403, got %d", rec.Code)
	}

	var body map[string]any
	if err := json.Unmarshal(rec.Body.Bytes(), &body); err != nil {
		t.Fatalf("failed to decode error response: %v", err)
	}
	if body["error"] != "invalid token" {
		t.Fatalf("unexpected error payload: %#v", body)
	}
}

func TestSubscriptionEndpoint_ValidTokenReturnsLinks(t *testing.T) {
	handler := makeUserAPIHandler(testStore(&pb.RegistryServiceConfig{
		Accounts: []*pb.Account{
			{Id: "a1", Token: "tok-1", ExpiryTime: 1735689600},
		},
		TemplateLinks: []*pb.RegistryTemplateLink{
			{Template: "vless://{{token}}@host-a:443#{{name}}"},
			{Template: "trojan://{token}@host-b:443#{id}"},
		},
	}), nil)

	rec := httptest.NewRecorder()
	req := httptest.NewRequest(http.MethodGet, "/v1/subscription?token=tok-1", nil)
	handler.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", rec.Code)
	}
	if !strings.HasPrefix(rec.Header().Get("Content-Type"), "text/plain") {
		t.Fatalf("unexpected content-type: %q", rec.Header().Get("Content-Type"))
	}
	if got := rec.Header().Get("subscription-userinfo"); got != "upload=0; download=0; total=0; expire=1735689600" {
		t.Fatalf("unexpected subscription-userinfo: %q", got)
	}
	if got := rec.Header().Get("profile-title"); got != "base64:YTE=" {
		t.Fatalf("unexpected profile-title: %q", got)
	}

	lines := strings.Split(strings.TrimSpace(rec.Body.String()), "\n")
	if len(lines) != 2 {
		t.Fatalf("expected 2 links, got %d: %q", len(lines), rec.Body.String())
	}
	if lines[0] != "vless://tok-1@host-a:443#a1" {
		t.Fatalf("unexpected first link: %q", lines[0])
	}
	if lines[1] != "trojan://tok-1@host-b:443#a1" {
		t.Fatalf("unexpected second link: %q", lines[1])
	}
}

func TestSubscriptionEndpointRecordsTelemetry(t *testing.T) {
	telemetry := &telemetryStore{path: filepath.Join(t.TempDir(), "telemetry.json")}
	handler := makeUserAPIHandler(testStore(&pb.RegistryServiceConfig{
		Accounts: []*pb.Account{
			{Id: "a1", Name: "Alice", Token: "tok-1"},
		},
		TemplateLinks: []*pb.RegistryTemplateLink{
			{Template: "vless://{{token}}@host:443"},
		},
	}), telemetry)

	rec := httptest.NewRecorder()
	req := httptest.NewRequest(http.MethodGet, "/v1/subscription?token=tok-1", nil)
	req.Header.Set("User-Agent", "test-client/1.0")
	req.Header.Set("X-Forwarded-For", "203.0.113.8, 10.0.0.1")
	handler.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", rec.Code)
	}
	data, err := os.ReadFile(telemetry.path)
	if err != nil {
		t.Fatalf("failed to read telemetry: %v", err)
	}
	var entries []subscriptionTelemetryEntry
	if err := json.Unmarshal(data, &entries); err != nil {
		t.Fatalf("failed to decode telemetry: %v", err)
	}
	if len(entries) != 1 {
		t.Fatalf("expected one telemetry entry, got %d", len(entries))
	}
	entry := entries[0]
	if entry.UserId != "a1" || entry.UserAgent != "test-client/1.0" || entry.Ip != "203.0.113.8" || entry.Time == 0 {
		t.Fatalf("unexpected telemetry entry: %#v", entry)
	}
}

func TestOnlySubscriptionEndpointExposed(t *testing.T) {
	handler := makeUserAPIHandler(testStore(nil), nil)

	for _, path := range []string{"/healthz", "/v1/subscriptions"} {
		rec := httptest.NewRecorder()
		req := httptest.NewRequest(http.MethodGet, path, nil)
		handler.ServeHTTP(rec, req)
		if rec.Code != http.StatusNotFound {
			t.Fatalf("%s: expected 404, got %d", path, rec.Code)
		}
	}
}

func testStore(config *pb.RegistryServiceConfig) *registryStore {
	return &registryStore{
		config: cloneRegistryConfig(config),
	}
}

func TestRegistryManagementAuthorize_RejectsMissingOrWrongKey(t *testing.T) {
	server := &registryManagementServer{masterKeyHash: hashMasterKey("secret")}

	if err := server.authorize(context.Background()); status.Code(err) != codes.Unauthenticated {
		t.Fatalf("expected unauthenticated for missing metadata, got %v", err)
	}

	ctx := metadata.NewIncomingContext(context.Background(), metadata.Pairs(registryMasterKeyHeader, "wrong"))
	if err := server.authorize(ctx); status.Code(err) != codes.Unauthenticated {
		t.Fatalf("expected unauthenticated for wrong key, got %v", err)
	}
}

func TestRegistryManagementAuthorize_AcceptsHashedKey(t *testing.T) {
	server := &registryManagementServer{masterKeyHash: hashMasterKey("secret")}
	ctx := metadata.NewIncomingContext(
		context.Background(),
		metadata.Pairs(registryMasterKeyHeader, hashMasterKey("secret")),
	)

	if err := server.authorize(ctx); err != nil {
		t.Fatalf("expected authorize success, got %v", err)
	}
}

func TestDefaultStorePath_UsesEnvOverride(t *testing.T) {
	t.Setenv("PS_REGISTRY_DATA_DIR", filepath.Join(string(filepath.Separator), "tmp", "registry-data"))

	got := defaultStorePath()
	want := filepath.Join(string(filepath.Separator), "tmp", "registry-data", "registry_services.json")
	if got != want {
		t.Fatalf("expected %q, got %q", want, got)
	}
}

func TestDefaultStorePath_UsesBuildDefault(t *testing.T) {
	t.Setenv("PS_REGISTRY_DATA_DIR", "")

	original := defaultDataDir
	defaultDataDir = filepath.Join(string(filepath.Separator), "var", "proxyswarm", "registry-test")
	t.Cleanup(func() { defaultDataDir = original })

	got := defaultStorePath()
	want := filepath.Join(string(filepath.Separator), "var", "proxyswarm", "registry-test", "registry_services.json")
	if runtime.GOOS == "windows" {
		want = filepath.Join("\\", "var", "proxyswarm", "registry-test", "registry_services.json")
	}
	if got != want {
		t.Fatalf("expected %q, got %q", want, got)
	}
}
