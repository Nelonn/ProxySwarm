PROTOC = "protoc"
GOPATH = $(shell go env GOPATH)

.PHONY: proto-go proto-rust build-node build-manager docker-node

all: proto-go proto-rust build-node build-manager

proto-go:
	cd proto && $(PROTOC) --go_out=../node/internal/pb --go_opt=paths=source_relative --go-grpc_out=../node/internal/pb --go-grpc_opt=paths=source_relative common.proto vless.proto hysteria2.proto trusttunnel.proto naiveproxy.proto wireguard.proto socks5.proto service.proto

proto-rust:
	cd manager && set PROTOC=$(PROTOC) && cargo build

build-node:
	cd node && go build -o ../proxyswarm-node ./cmd/node

build-manager:
	cd manager && trunk build

docker-node:
	docker build -t proxyswarm-node:latest -f node/Dockerfile .
