package engine

import (
	"testing"

	"proxyswarm/node/internal/pb"
)

func TestAccountStatusesLockedIncludesConfiguredAccountsWithoutSamples(t *testing.T) {
	m := &Manager{
		metricsState: newPersistedMetricsState(),
		currentConfig: &pb.FullConfig{
			Accounts: []*pb.Account{
				{Id: "ac40ca0f", Name: "Alice"},
				{Id: "082bb867"},
			},
		},
	}

	accounts := m.accountStatusesLocked()
	if len(accounts) != 2 {
		t.Fatalf("expected 2 account statuses, got %d", len(accounts))
	}
	if accounts[0].GetName() != "Alice" {
		t.Fatalf("expected first account name Alice, got %q", accounts[0].GetName())
	}
	if accounts[1].GetName() != "082bb867" {
		t.Fatalf("expected second account fallback to id, got %q", accounts[1].GetName())
	}
	if accounts[0].GetTraffic() == nil || accounts[1].GetTraffic() == nil {
		t.Fatal("expected zeroed traffic stats for configured accounts")
	}
}
