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
	if accounts[0].GetId() != "ac40ca0f" {
		t.Fatalf("expected first account id ac40ca0f, got %q", accounts[0].GetId())
	}
	if accounts[1].GetId() != "082bb867" {
		t.Fatalf("expected second account id 082bb867, got %q", accounts[1].GetId())
	}
	if accounts[0].GetTraffic() == nil || accounts[1].GetTraffic() == nil {
		t.Fatal("expected zeroed traffic stats for configured accounts")
	}
}
