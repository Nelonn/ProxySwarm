PROTOC = "protoc"
GOPATH = $(shell go env GOPATH)

.PHONY: proto-node-go proto-registry-go proto-rust build-node build-registry build-manager docker-node

all: proto-node-go proto-registry-go proto-rust build-node build-registry build-manager

proto-node-go:
	cd proto && $(PROTOC) -I. --go_out=../node/internal/pb --go_opt=paths=source_relative --go-grpc_out=../node/internal/pb --go-grpc_opt=paths=source_relative account.proto node/common.proto node/vless.proto node/hysteria2.proto node/trusttunnel.proto node/naiveproxy.proto node/wireguard.proto node/socks5.proto node/shadowsocks.proto node/service.proto

proto-registry-go:
	cd proto && $(PROTOC) -I. --go_out=../registry/internal/pb --go_opt=paths=source_relative account.proto node/common.proto
	cd proto && $(PROTOC) -I. --go_out=../registry/internal/pb --go_opt=paths=source_relative --go-grpc_out=../registry/internal/pb --go-grpc_opt=paths=source_relative registry/registry.proto

proto-rust:
	cd manager && set PROTOC=$(PROTOC) && cargo build

build-node:
	cd node && go build -o ../proxyswarm-node ./cmd/node

build-registry:
	cd registry && go build -o ../proxyswarm-registry ./cmd/registry

build-manager:
	cd manager && trunk build

docker-node:
	docker build -t proxyswarm-node:latest -f node/Dockerfile .

docker-registry:
	docker build -t proxyswarm-registry:latest -f registry/Dockerfile .
