package engine

import (
	"path/filepath"
	"testing"

	"proxyswarm/node/internal/pb"
)

func TestPersistedConfigRoundTrip(t *testing.T) {
	manager := NewManager()
	manager.statePath = filepath.Join(t.TempDir(), "deployed_config.json")

	config := &pb.FullConfig{
		Inbounds: []*pb.InboundConfig{
			{
				Name:    "main",
				Listen:  "0.0.0.0",
				Port:    443,
				Enabled: true,
			},
		},
		Accounts: []*pb.Account{
			{
				Id:       "1",
				Name:     "user",
				Token: "access",
			},
		},
	}

	if err := manager.savePersistedConfig(config); err != nil {
		t.Fatalf("savePersistedConfig returned error: %v", err)
	}

	loaded, err := manager.loadPersistedConfig()
	if err != nil {
		t.Fatalf("loadPersistedConfig returned error: %v", err)
	}
	if loaded == nil {
		t.Fatal("expected config to be loaded")
	}
	if got := len(loaded.Inbounds); got != 1 {
		t.Fatalf("expected 1 inbound, got %d", got)
	}
	if loaded.Inbounds[0].Name != "main" || loaded.Inbounds[0].Port != 443 || !loaded.Inbounds[0].Enabled {
		t.Fatalf("unexpected inbound contents: %#v", loaded.Inbounds[0])
	}
	if got := len(loaded.Accounts); got != 1 || loaded.Accounts[0].Token != "access" {
		t.Fatalf("unexpected accounts: %#v", loaded.Accounts)
	}
}
