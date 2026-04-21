package stats

import (
	"proxyswarm/node/internal/pb"
	"time"

	"github.com/shirou/gopsutil/v3/cpu"
	"github.com/shirou/gopsutil/v3/mem"
)

type Collector struct {
	startTime time.Time
}

func NewCollector() *Collector {
	return &Collector{
		startTime: time.Now(),
	}
}

func (c *Collector) GetHardwareStats() *pb.HardwareStats {
	vm, _ := mem.VirtualMemory()
	cPercent, _ := cpu.Percent(0, false)
	cpuCounts, _ := cpu.Counts(true)

	cpuUsage := 0.0
	if len(cPercent) > 0 {
		cpuUsage = cPercent[0]
	}

	return &pb.HardwareStats{
		CpuUsage: cpuUsage,
		RamUsed:  vm.Used,
		RamTotal: vm.Total,
		Uptime:   uint64(time.Since(c.startTime).Seconds()),
		CpuCores: uint32(max(cpuCounts, 0)),
	}
}
