# syntax=docker/dockerfile:1.7

ARG GO_VERSION=1.26.2
ARG ALPINE_VERSION=3.22

FROM golang:${GO_VERSION}-alpine AS registry-builder

WORKDIR /src/registry

COPY registry/go.mod registry/go.sum ./
RUN --mount=type=cache,target=/go/pkg/mod \
    --mount=type=cache,target=/root/.cache/go-build \
    go mod download

COPY registry/ ./

RUN --mount=type=cache,target=/go/pkg/mod \
    --mount=type=cache,target=/root/.cache/go-build \
    CGO_ENABLED=0 GOOS=linux go build \
    -ldflags "-X 'main.defaultDataDir=/data'" \
    -o /out/proxyswarm-registry ./cmd/registry

FROM alpine:${ALPINE_VERSION}

RUN apk add --no-cache ca-certificates \
    && adduser -D -h /var/lib/proxyswarm proxyswarm \
    && mkdir -p /var/lib/proxyswarm/.config/proxyswarm \
    && chown -R proxyswarm:proxyswarm /var/lib/proxyswarm

COPY --from=registry-builder /out/proxyswarm-registry /usr/local/bin/proxyswarm-registry

USER proxyswarm
WORKDIR /var/lib/proxyswarm

ENV HOME=/var/lib/proxyswarm
ENV REGISTRY_MODE=1
ENV REGISTRY_LISTEN=:9191
ENV REGISTRY_MANAGE_LISTEN=:9291

EXPOSE 9191 9291

ENTRYPOINT ["proxyswarm-registry"]
