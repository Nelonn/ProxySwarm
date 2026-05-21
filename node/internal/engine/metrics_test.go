package engine

import (
	"testing"

	"proxyswarm/node/internal/pb"
)

func TestApplyTrafficDeltaUsesFirstSampleAsBaseline(t *testing.T) {
	lastRaw := make(map[string]trafficTotals)
	aggregate := &pb.TrafficStats{}

	delta := applyTrafficDelta(lastRaw, "xray:inbound:main", &pb.TrafficStats{Rx: 1000, Tx: 2000}, aggregate, 2)
	if delta.Rx != 0 || delta.Tx != 0 {
		t.Fatalf("first sample delta = %d/%d, want 0/0", delta.Rx, delta.Tx)
	}
	if aggregate.Rx != 0 || aggregate.Tx != 0 || aggregate.RxRate != 0 || aggregate.TxRate != 0 {
		t.Fatalf("first sample aggregate = %+v, want zero", aggregate)
	}

	delta = applyTrafficDelta(lastRaw, "xray:inbound:main", &pb.TrafficStats{Rx: 1500, Tx: 2600}, aggregate, 2)
	if delta.Rx != 500 || delta.Tx != 600 {
		t.Fatalf("second sample delta = %d/%d, want 500/600", delta.Rx, delta.Tx)
	}
	if aggregate.Rx != 500 || aggregate.Tx != 600 || aggregate.RxRate != 250 || aggregate.TxRate != 300 {
		t.Fatalf("second sample aggregate = %+v, want 500/600 at 250/300 Bps", aggregate)
	}
}

func TestApplyTrafficDeltaHandlesCounterReset(t *testing.T) {
	lastRaw := map[string]trafficTotals{"xray:inbound:main": {Rx: 1000, Tx: 2000}}
	aggregate := &pb.TrafficStats{}

	delta := applyTrafficDelta(lastRaw, "xray:inbound:main", &pb.TrafficStats{Rx: 100, Tx: 200}, aggregate, 2)
	if delta.Rx != 100 || delta.Tx != 200 {
		t.Fatalf("reset delta = %d/%d, want 100/200", delta.Rx, delta.Tx)
	}
}
