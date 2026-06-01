<p align="center">
<img src="./icon.svg" height="128" alt="icon"> 
</p>

<h1 align="center">ProxySwarm</h1>

ProxySwarm lets you run a fleet of self-hosted proxy nodes from a single control plane. The manager owns all
configuration state; nodes consume it and converge to it. Same config in, same behavior out - every time.

> Deterministic proxy infrastructure - one manager, many nodes, zero config drift.

## What's included

| Component            | Stack           | Role                                                                    |
|----------------------|-----------------|-------------------------------------------------------------------------|
| **Node runtime**     | Go + gRPC       | Runs proxy engines, applies configs, reports live telemetry             |
| **Registry service** | Go + gRPC-Web   | Stores subscription services and exposes registry management RPCs       |
| **Manager UI**       | Rust + Yew/WASM | Browser app for config, certificates, routing, accounts, and deployment |

**Supported inbound protocols:** VLESS, Hysteria2  
**Proxy engines:** xray-core, sing-box, TrustTunnel  
**Routing:** automatic WARP outbound flows in manager-driven configs

---

## Repository layout

```
proxyswarm/
├── manager/  # Web UI (Yew + Trunk)
├── node/     # Runtime node service (Go)
├── registry/ # Subscription registry service (Go)
└── proto/    # Shared gRPC protocol
```

---

## Core concepts

**Single source of truth.** The manager builds one `FullConfig` model covering inbounds, outbounds, routing, accounts,
certificates, and DNS. Nodes never hold their own state.

**Deterministic nodes.** Each node receives its config via the `NodeService` RPC (`proto/node/service.proto`) and
applies it
exactly as declared - no local overrides.

**Multi-engine execution.** The node selects the right engine implementation per inbound core: `XRAY` or `SING_BOX`.
Protocol intent (e.g. VLESS/Hysteria2) is described in the manager and rendered into engine-specific runtime config on
the node.

**Certificate management.** The built-in cert manager supports ACME and custom certificates for consistent TLS across
all nodes.

**Operational loop.**

```
Manager deploys config  →  Node applies config  →  Manager reads status / traffic / session telemetry
```

---

## Prerequisites

- Go **1.26+**
- Rust (stable) + Cargo
- [`trunk`](https://trunk-rs.github.io/trunk/) - web build/serve
- `protoc` - Protocol Buffers compiler
- GNU Make *(optional, for convenience targets)*
- Podman or Docker *(optional, for containerized node)*

---

## Quick start

### 1. Run the node (local)

```bash
cd node
PS_MASTER_KEY = "change-me" go run .\cmd\node
```

| Variable      | Default      | Description                            |
|---------------|--------------|----------------------------------------|
| `PS_MASTER_KEY`  | *(required)* | Shared secret between manager and node |
| `GRPC_LISTEN` | `:9090`      | gRPC-Web listen address                |

The node exposes gRPC-Web on the same port.

### 2. Run the manager UI (local)

```bash
cd manager
trunk serve --config Trunk.toml
```

Open the Trunk URL in your browser, add the node address (e.g. `http://127.0.0.1:9090`) and the matching master key.

### 3. Run the registry service (local)

```bash
cd registry
PS_MASTER_KEY="change-me" go run .\cmd\registry
```

| Variable                    | Default      | Description                                           |
|-----------------------------|--------------|-------------------------------------------------------|
| `PS_MASTER_KEY`             | *(required)* | Shared secret between manager and registry manage API |
| `REGISTRY_MODE`             | `1`          | `1`: user + manage on same port, `2`: split ports     |
| `REGISTRY_LISTEN`           | `:9191`      | User API listen address (mode `1`: shared address)    |
| `REGISTRY_MANAGE_LISTEN`    | `:9291`      | Manage gRPC-Web listen address (mode `2` only)        |
| `REGISTRY_SUB_UPDATE_HOURS` | `1`          | Subscription profile update interval in hours         |

### 4. Run via Docker

> [!CAUTION]
> Be careful with GRPC_LISTEN! It’s better to listen only on the local network and use a reverse proxy with HTTPS or an
> SSH tunnel (ssh -L 9091:localhost:9090 user@host)

```yaml
services:
  proxy-node:
    image: ghcr.io/nelonn/proxyswarm/proxyswarm-node:nightly
    restart: unless-stopped
    network_mode: host
    environment:
      - PS_MASTER_KEY=${PS_MASTER_KEY:-my-secret-key}
      - GRPC_LISTEN=127.0.0.1:9090
    volumes:
      - ./proxyswarm-node-data:/data
```

```bash
docker compose up --build
```

`docker-compose.yaml` starts `proxy-node` with `PS_MASTER_KEY`, `GRPC_LISTEN=:9090`, and host networking. The default
`PS_MASTER_KEY` in the compose file is for local testing only - override it in production.

---

## Build commands

Run from the repository root:

```bash
make proto-node-go     # Generate Go protobuf bindings
make proto-registry-go # Generate registry Go protobuf bindings
make proto-rust        # Regenerate Rust protobuf bindings (via manager build)
make build-node        # Build node binary → gateway-node
make build-registry    # Build registry binary → proxyswarm-registry
make build-manager     # Build manager web bundle
```

> The current `Makefile` references a Windows `PROTOC` path. Adjust the `PROTOC` variable if your environment differs.

---

## API reference

### NodeService RPC methods

| Method                 | Request type               | Description                       |
|------------------------|----------------------------|-----------------------------------|
| `UpdateConfig`         | `FullConfig`               | Push a full config to the node    |
| `GetStatus`            | `StatusRequest`            | Read live node status and metrics |
| `IssueAcmeCertificate` | `AcmeIssueRequest`         | Trigger ACME certificate issuance |
| `RegisterWarp`         | `WarpRegisterRequest`      | Register a WARP identity          |
| `UpdateWarpLicense`    | `WarpLicenseUpdateRequest` | Update the WARP license           |

### RegistryManagementService RPC methods

| Method          | Request type                   | Description                           |
|-----------------|--------------------------------|---------------------------------------|
| `ListServices`  | `RegistryListServicesRequest`  | List configured subscription services |
| `UpsertService` | `RegistryUpsertServiceRequest` | Create/update subscription service    |
| `DeleteService` | `RegistryDeleteServiceRequest` | Delete subscription service by ID     |

---

## Development workflow

1. **Update schemas** in `proto/` if any contract changes.
2. **Regenerate outputs** for both Go and Rust.
3. **Update manager and node logic** as needed.
4. **Verify:**

```bash
cd node    && go test ./...
cd manager && cargo check
```

---

## License

MIT - see [LICENSE](./LICENSE)
