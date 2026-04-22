package main

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/metadata"
	"google.golang.org/grpc/status"
	"proxyswarm/registry/internal/pb"
)

func TestFindAccountByToken_EnabledOnly(t *testing.T) {
	services := []*pb.RegistryService{
		{
			Id:      "disabled",
			Name:    "disabled",
			Enabled: false,
			Accounts: []*pb.Account{
				{Id: "a1", Name: "u1", Token: "tok-disabled"},
			},
		},
		{
			Id:      "enabled",
			Name:    "enabled",
			Enabled: true,
			Accounts: []*pb.Account{
				{Id: "a2", Name: "u2", Token: "tok-enabled"},
			},
		},
	}

	account, ok := findAccountByToken(services, "tok-enabled")
	if !ok {
		t.Fatal("expected token to be found in enabled service")
	}
	if account.GetId() != "a2" {
		t.Fatalf("unexpected account id: %q", account.GetId())
	}

	if _, ok := findAccountByToken(services, "tok-disabled"); ok {
		t.Fatal("expected token from disabled service to be ignored")
	}
}

func TestBuildSubscriptionLinks_RendersAndDeduplicates(t *testing.T) {
	account := &pb.Account{Id: "acc-1", Name: "alice", Token: "tok-1"}
	services := []*pb.RegistryService{
		{
			Id:      "s1",
			Name:    "svc-1",
			Enabled: true,
			TemplateLinks: []*pb.RegistryTemplateLink{
				{Template: "vless://{{token}}@host:443#{{name}}"},
				{Template: "vless://{{token}}@host:443#{{name}}"}, // duplicate
				{Template: "trojan://{token}@node:443#{id}"},
			},
		},
		{
			Id:      "s2",
			Name:    "svc-2",
			Enabled: false,
			TemplateLinks: []*pb.RegistryTemplateLink{
				{Template: "vless://{{token}}@disabled:443"},
			},
		},
	}

	links := buildSubscriptionLinks(services, account)
	if len(links) != 2 {
		t.Fatalf("expected 2 unique links, got %d: %#v", len(links), links)
	}
	if links[0] != "vless://tok-1@host:443#alice" {
		t.Fatalf("unexpected first link: %q", links[0])
	}
	if links[1] != "trojan://tok-1@node:443#acc-1" {
		t.Fatalf("unexpected second link: %q", links[1])
	}
}

func TestSubscriptionEndpoint_InvalidTokenReturns403(t *testing.T) {
	handler := makeUserAPIHandler(testStore([]*pb.RegistryService{
		{
			Id:      "s1",
			Name:    "svc",
			Enabled: true,
			Accounts: []*pb.Account{
				{Id: "a1", Name: "alice", Token: "tok-1"},
			},
			TemplateLinks: []*pb.RegistryTemplateLink{
				{Template: "vless://{{token}}@host:443"},
			},
		},
	}))

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
	handler := makeUserAPIHandler(testStore([]*pb.RegistryService{
		{
			Id:      "s1",
			Name:    "svc",
			Enabled: true,
			Accounts: []*pb.Account{
				{Id: "a1", Name: "alice", Token: "tok-1", ExpiryTime: 1735689600},
			},
			TemplateLinks: []*pb.RegistryTemplateLink{
				{Template: "vless://{{token}}@host-a:443#{{name}}"},
				{Template: "trojan://{token}@host-b:443#{id}"},
			},
		},
	}))

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
	if got := rec.Header().Get("profile-title"); got != "base64:YWxpY2U=" {
		t.Fatalf("unexpected profile-title: %q", got)
	}

	lines := strings.Split(strings.TrimSpace(rec.Body.String()), "\n")
	if len(lines) != 2 {
		t.Fatalf("expected 2 links, got %d: %q", len(lines), rec.Body.String())
	}
	if lines[0] != "vless://tok-1@host-a:443#alice" {
		t.Fatalf("unexpected first link: %q", lines[0])
	}
	if lines[1] != "trojan://tok-1@host-b:443#a1" {
		t.Fatalf("unexpected second link: %q", lines[1])
	}
}

func TestOnlySubscriptionEndpointExposed(t *testing.T) {
	handler := makeUserAPIHandler(testStore(nil))

	for _, path := range []string{"/healthz", "/v1/subscriptions"} {
		rec := httptest.NewRecorder()
		req := httptest.NewRequest(http.MethodGet, path, nil)
		handler.ServeHTTP(rec, req)
		if rec.Code != http.StatusNotFound {
			t.Fatalf("%s: expected 404, got %d", path, rec.Code)
		}
	}
}

func testStore(services []*pb.RegistryService) *registryStore {
	items := make(map[string]*pb.RegistryService, len(services))
	for _, service := range services {
		if service == nil {
			continue
		}
		items[service.Id] = cloneRegistryService(service)
	}
	return &registryStore{services: items}
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
