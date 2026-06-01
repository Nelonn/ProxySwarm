# syntax=docker/dockerfile:1.7

ARG GO_VERSION=1.26.2
ARG ALPINE_VERSION=3.22
ARG SING_BOX_VERSION=1.13.8

# Stage 1: Build proxy-node
FROM golang:${GO_VERSION}-alpine AS node-builder

WORKDIR /src/node

COPY node/go.mod node/go.sum ./
RUN --mount=type=cache,target=/go/pkg/mod \
    --mount=type=cache,target=/root/.cache/go-build \
    go mod download

COPY node/ ./

RUN --mount=type=cache,target=/go/pkg/mod \
    --mount=type=cache,target=/root/.cache/go-build \
    CGO_ENABLED=0 GOOS=linux go build \
    -ldflags "\
    -X 'proxyswarm/node/internal/engine.defaultDataDir=/data' \
    -X 'proxyswarm/node/internal/engine.defaultXrayAssetDir=/usr/local/bin' \
    -X 'proxyswarm/node/internal/engine.TrustTunnelEndpointBinary=/usr/local/bin/trusttunnel_endpoint' \
    -X 'proxyswarm/node/internal/engine.TrustTunnelClientBinary=/usr/local/bin/trusttunnel_client' \
    -X 'proxyswarm/node/internal/engine.SingBoxBinary=/usr/local/bin/sing-box'" \
    -o /out/proxy-node ./cmd/node

# Stage 2: Download TrustTunnel
FROM alpine:${ALPINE_VERSION} AS trusttunnel-endpoint-downloader

RUN apk add --no-cache bash curl tar

COPY node/scripts/download_trusttunnel_endpoint.sh /download_trusttunnel_endpoint.sh
RUN chmod +x /download_trusttunnel_endpoint.sh \
    && /download_trusttunnel_endpoint.sh \
    && install -Dm755 trusttunnel_endpoint /out/trusttunnel_endpoint

# Stage 3: Download TrustTunnelClient
FROM alpine:${ALPINE_VERSION} AS trusttunnel-client-downloader

RUN apk add --no-cache bash curl tar

COPY node/scripts/download_trusttunnel_client.sh /download_trusttunnel_client.sh
RUN chmod +x /download_trusttunnel_client.sh \
    && /download_trusttunnel_client.sh \
    && install -Dm755 trusttunnel_client /out/trusttunnel_client

# Stage 4: Build sing-box
FROM golang:${GO_VERSION}-alpine AS singbox-builder

ARG SING_BOX_VERSION

RUN apk add --no-cache git

RUN --mount=type=cache,target=/go/pkg/mod \
    --mount=type=cache,target=/root/.cache/go-build \
    CGO_ENABLED=0 GOOS=linux \
    go install -tags with_quic,with_wireguard,with_gvisor github.com/sagernet/sing-box/cmd/sing-box@v${SING_BOX_VERSION}

RUN install -Dm755 /go/bin/sing-box /out/sing-box

# Stage 5: Download Xray geodata
FROM scratch AS geodata-downloader

ADD https://raw.githubusercontent.com/Loyalsoldier/v2ray-rules-dat/release/geoip.dat /out/geoip.dat
ADD https://raw.githubusercontent.com/Loyalsoldier/v2ray-rules-dat/release/geosite.dat /out/geosite.dat

# Stage 6: Final runtime image
FROM alpine:${ALPINE_VERSION}

RUN apk add --no-cache ca-certificates

COPY --from=node-builder /out/proxy-node /usr/local/bin/proxy-node
COPY --from=trusttunnel-endpoint-downloader /out/trusttunnel_endpoint /usr/local/bin/trusttunnel_endpoint
COPY --from=trusttunnel-client-downloader /out/trusttunnel_client /usr/local/bin/trusttunnel_client
COPY --from=singbox-builder /out/sing-box /usr/local/bin/sing-box
COPY --from=geodata-downloader /out/geoip.dat /usr/local/bin/geoip.dat
COPY --from=geodata-downloader /out/geosite.dat /usr/local/bin/geosite.dat

ENTRYPOINT ["proxy-node"]
ENV GRPC_LISTEN=:9090
