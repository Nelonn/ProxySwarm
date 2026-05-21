package logging

import (
	"log"
	"os"
	"strings"

	"go.uber.org/zap/zapcore"
)

type Level int

const (
	DebugLevel Level = iota
	InfoLevel
	WarnLevel
	ErrorLevel
)

func CurrentLevel() Level {
	return parseLevel(envLogLevel())
}

func Debugf(format string, args ...any) {
	if CurrentLevel() <= DebugLevel {
		log.Printf(format, args...)
	}
}

func Infof(format string, args ...any) {
	if CurrentLevel() <= InfoLevel {
		log.Printf(format, args...)
	}
}

func Warnf(format string, args ...any) {
	if CurrentLevel() <= WarnLevel {
		log.Printf(format, args...)
	}
}

func Errorf(format string, args ...any) {
	if CurrentLevel() <= ErrorLevel {
		log.Printf(format, args...)
	}
}

func XrayLogLevel() string {
	switch CurrentLevel() {
	case DebugLevel:
		return "debug"
	case WarnLevel:
		return "warning"
	case ErrorLevel:
		return "error"
	default:
		return "info"
	}
}

func ZapLevel() zapcore.Level {
	switch CurrentLevel() {
	case DebugLevel:
		return zapcore.DebugLevel
	case WarnLevel:
		return zapcore.WarnLevel
	case ErrorLevel:
		return zapcore.ErrorLevel
	default:
		return zapcore.InfoLevel
	}
}

func envLogLevel() string {
	if value := strings.TrimSpace(os.Getenv("PS_LOG_LEVEL")); value != "" {
		return value
	}
	return strings.TrimSpace(os.Getenv("LOG_LEVEL"))
}

func parseLevel(value string) Level {
	switch strings.ToUpper(strings.TrimSpace(value)) {
	case "DEBUG":
		return DebugLevel
	case "WARN", "WARNING":
		return WarnLevel
	case "ERROR", "ERR":
		return ErrorLevel
	default:
		return WarnLevel
	}
}
