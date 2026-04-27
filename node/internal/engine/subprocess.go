package engine

import (
	"context"
	"fmt"
	"io"
	"os"
	"os/exec"
	"proxyswarm/node/internal/logging"
	"sync"
	"time"
)

type supervisedProcess struct {
	label       string
	mu          sync.Mutex
	cmd         *exec.Cmd
	stopping    bool
	generation  uint64
	lastCommand []string
	cancel      context.CancelFunc
}

func newSupervisedProcess(label string) *supervisedProcess {
	return &supervisedProcess{label: label}
}

func (p *supervisedProcess) restart(ctx context.Context, command []string) error {
	p.mu.Lock()
	defer p.mu.Unlock()
	_ = ctx

	p.stopping = false
	p.generation++
	p.stopLocked()
	return p.startLocked(command)
}

func (p *supervisedProcess) startLocked(command []string) error {
	if len(command) == 0 {
		return fmt.Errorf("%s start requires command", p.label)
	}
	processCtx, cancel := context.WithCancel(context.Background())
	cmd := exec.CommandContext(processCtx, command[0], command[1:]...)
	stdout, err := cmd.StdoutPipe()
	if err != nil {
		cancel()
		return fmt.Errorf("failed to capture %s stdout: %w", p.label, err)
	}
	stderr, err := cmd.StderrPipe()
	if err != nil {
		cancel()
		return fmt.Errorf("failed to capture %s stderr: %w", p.label, err)
	}
	if err := cmd.Start(); err != nil {
		cancel()
		return fmt.Errorf("failed to start %s: %w", p.label, err)
	}
	p.cmd = cmd
	p.cancel = cancel
	p.lastCommand = append([]string(nil), command...)
	generation := p.generation
	go io.Copy(os.Stdout, stdout)
	go io.Copy(os.Stderr, stderr)
	go p.monitorProcess(cmd, generation)
	logging.Debugf("[%s] started pid=%d args=%q", p.label, cmd.Process.Pid, command)
	return nil
}

func (p *supervisedProcess) monitorProcess(cmd *exec.Cmd, generation uint64) {
	err := cmd.Wait()
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.cmd == cmd {
		p.cmd = nil
	}
	if p.stopping || generation != p.generation {
		if err != nil {
			logging.Debugf("[%s] process exit ignored during stop/reload gen=%d err=%v", p.label, generation, err)
		}
		return
	}
	logging.Warnf("[%s] process exited gen=%d err=%v; restarting", p.label, generation, err)
	if len(p.lastCommand) == 0 {
		logging.Errorf("[%s] restart skipped: missing cached command", p.label)
		return
	}
	time.AfterFunc(500*time.Millisecond, func() {
		p.mu.Lock()
		defer p.mu.Unlock()
		if p.stopping || generation != p.generation || p.cmd != nil {
			return
		}
		if err := p.startLocked(p.lastCommand); err != nil {
			logging.Errorf("[%s] restart failed gen=%d err=%v", p.label, generation, err)
			return
		}
		logging.Infof("[%s] restarted gen=%d", p.label, generation)
	})
}

func (p *supervisedProcess) isAlive() bool {
	p.mu.Lock()
	defer p.mu.Unlock()
	return p.cmd != nil && p.cmd.Process != nil
}

func (p *supervisedProcess) stop() {
	p.mu.Lock()
	defer p.mu.Unlock()
	p.stopLocked()
}

func (p *supervisedProcess) stopLocked() {
	p.stopping = true
	p.generation++
	if p.cancel != nil {
		p.cancel()
		p.cancel = nil
	}
	if p.cmd != nil && p.cmd.Process != nil {
		_ = p.cmd.Process.Kill()
	}
	p.cmd = nil
}
