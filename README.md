# VolSync WebUI

A web-based management interface for VolSync replication and backup operations on Kubernetes.

> [!CAUTION]
> This project is 101% AI generated. The code was not yet reviewed by me. I make the repo public to allow easy testing. Dont expect to be anything functional.

## Features

- **Application Management**: List and select applications backed by VolSync ReplicationSources
- **Snapshot Viewer**: View Restic snapshots for each application
- **Backup Operations**: Trigger manual backups for individual apps or all apps
- **Restore Operations**: Restore from any available snapshot timestamp
- **Namespace Filtering**: View and filter resources across Kubernetes namespaces

## Architecture

```
volsync-webui/
├── backend/              # Axum-based REST API server
│   └── src/
│       ├── main.rs       # Server entry point, router setup
│       ├── api.rs        # HTTP handlers for all endpoints
│       ├── kubectl.rs    # Kubernetes API client (raw HTTP via reqwest)
│       └── models.rs     # Request/response data structures
├── frontend/             # Yew-based WASM web UI
│   ├── src/
│   │   ├── main.rs       # WASM entry point
│   │   ├── lib.rs        # Library exports
│   │   ├── api.rs        # Frontend API client (fetch)
│   │   └── components/   # Yew UI components
│   │       ├── app.rs           # Main application shell
│   │       ├── app_selector.rs  # Application dropdown
│   │       ├── backup_panel.rs  # Backup controls
│   │       ├── restore_panel.rs # Restore controls
│   │       ├── snapshot_list.rs # Snapshot table
│   │       └── namespace.rs     # Namespace selector
│   └── index.html        # HTML shell
├── kube-manifests/       # Kubernetes YAML manifests
│   ├── 00-namespace-rbac.yaml   # Namespace, ServiceAccount, ClusterRole
│   ├── 01-clusterrolebinding.yaml
│   ├── 02-deployment.yaml       # Deployment with probes
│   └── 03-service.yaml
├── helm-charts/         # Helm chart for deployment
│   └── volsync-webui/
└── Dockerfile           # Multi-stage container build
```

## Tech Stack

| Layer | Technology |
|-------|------------|
| Backend | Axum 0.7, Tokio, reqwest |
| Frontend | Yew 0.21 (WASM), TailwindCSS |
| Kubernetes | Raw HTTP API via reqwest (in-cluster) |
| Container | Debian bullseye-slim |

## API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Health check |
| GET | `/api/namespaces` | List all namespaces |
| GET | `/api/apps` | List all ReplicationSources |
| GET | `/api/apps/:app/:ns/snapshots` | Get snapshots for an app |
| POST | `/api/apps/:app/:ns/backup` | Trigger backup for app |
| POST | `/api/apps/:app/:ns/restore` | Trigger restore for app |
| POST | `/api/apps/backup-all` | Trigger backup for all apps |

## Development

### Prerequisites

- Rust 1.75+
- Node.js 18+
- Nix (for development shell)

### Nix Dev Shell

```bash
cd volsync-webui
nix develop --accept-flake-config -c cargo check
```

### Build Commands

```bash
# Check both crates
nix develop --accept-flake-config -c cargo check

# Check individual crates
nix develop --accept-flake-config -c cargo check -p volsync-webui-backend
nix develop --accept-flake-config -c cargo check -p volsync-webui-frontend

# Build backend
nix develop --accept-flake-config -c cargo build -p volsync-webui-backend

# Format code
nix develop --accept-flake-config -c cargo fmt
```

### Local Development (without Nix)

```bash
# Install Rust dependencies
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup install 1.75

# Build frontend WASM
cargo install trunk wasm-bindgen-cli
cd frontend && trunk build

# Run backend
cd backend && cargo run
```

## Deployment

### Docker

```bash
docker build -t volsync-webui:latest volsync-webui/
docker run -p 8080:8080 volsync-webui:latest
```

The container expects to run inside a Kubernetes cluster with access to the Kubernetes API.

### Kubernetes Manifests

```bash
kubectl apply -f kube-manifests/
```

This creates:
- `volsync-webui` namespace
- `volsync-webui` ServiceAccount
- `volsync-webui` ClusterRole (broad read access to pods, secrets, ReplicationSources, HelmReleases)
- ClusterRoleBinding linking SA to ClusterRole
- Deployment with readiness/liveness probes
- ClusterIP Service on port 8080

### Helm

```bash
helm install volsync-webui ./helm-charts/volsync-webui/
```

## RBAC Permissions

The ServiceAccount requires cluster-wide permissions:

| Resource | Verbs |
|----------|-------|
| pods | get, list, delete, create, watch |
| secrets | get, list, create, delete |
| replicationsources | get, list, patch, update |
| replicationdestinations | get, list, create, patch, update |
| helmreleases | get, list, patch |

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `RUST_LOG` | `info` | Logging level (trace, debug, info, warn, error) |
| `KUBERNETES_SERVICE_HOST` | auto-detected | Kubernetes API server host |
| `KUBERNETES_SERVICE_PORT` | `443` | Kubernetes API server port |
| `VOLSYNC_SECRET_SUFFIX` | `-volsync-secret` | Suffix appended to app name to form secret name (e.g., `{app}-volsync-secret`) |

## Building Frontend (Yew/WASM)

The frontend is compiled to WebAssembly and served by the backend.

```bash
# Using trunk (recommended)
cd frontend
trunk build

# Output goes to frontend/dist/
```

### TailwindCSS

TailwindCSS is used for styling. The config is in `frontend/tailwind.config.js`.

To rebuild styles during development:
```bash
cd frontend
npx tailwindcss -i ./src/style.css -o ./dist/style.css --watch
```

## Kubernetes Compatibility

- Tested with Kubernetes 1.25+
- Uses `networking.k8s.io/v1` for Ingress (if added later)
- Uses `replication.storage.io/v1alpha1` for VolSync CRDs
- Uses `source.toolkit.fluxcd.io/v1beta2` for HelmRelease

## Security Considerations

1. **ServiceAccount**: Runs with a dedicated SA, not default
2. **ClusterRole**: Requires broad read access - restrict in production
3. **No TLS**: Backend runs HTTP; TLS should be handled by ingress/controller
4. **No Authentication**: Currently no auth - expose via auth proxy (e.g., OAuth2 proxy) in production
5. **Secret Access**: Reads secrets named `{app}{VOLSYNC_SECRET_SUFFIX}` (default: `{app}-volsync-secret`) in app namespaces. Configurable via `VOLSYNC_SECRET_SUFFIX` env var.

## License

MIT
