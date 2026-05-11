# VolSync WebUI - Development Guide

## Project Structure

```
volsync-webui/
├── backend/                    # Axum REST API
│   └── src/
│       ├── main.rs             # Entry point, router setup
│       ├── api.rs              # HTTP handlers
│       ├── kubectl.rs           # Kubernetes client (reqwest)
│       └── models.rs            # Data structures
├── frontend/                   # Yew WASM UI
│   └── src/
│       ├── main.rs             # WASM entry point
│       ├── lib.rs              # Exports AppComponent
│       ├── api.rs              # Frontend fetch client
│       └── components/          # UI components
│           ├── app.rs
│           ├── app_selector.rs
│           ├── backup_panel.rs
│           ├── restore_panel.rs
│           ├── snapshot_list.rs
│           └── namespace.rs
├── kube-manifests/             # K8s YAML
├── helm-charts/                # Helm chart
├── flake.nix                   # Nix dev shell
└── Dockerfile                  # Multi-stage build
```

## Development Commands

```bash
cd volsync-webui

# Enter dev shell
nix develop --accept-flake-config

# Check compilation (both crates)
cargo check

# Check individual crates
cargo check -p volsync-webui-backend
cargo check -p volsync-webui-frontend

# Build
cargo build -p volsync-webui-backend
cargo build -p volsync-webui-frontend

# Format
cargo fmt

# Run tests (if any)
cargo test
```

## Key Dependencies

### Backend
- `axum 0.7` - Web framework
- `tokio` - Async runtime
- `reqwest` - HTTP client (Kubernetes API)
- `serde`/`serde_json` - Serialization
- `chrono` - Date/time
- `tower`/`tower-http` - Middleware (CORS)

### Frontend
- `yew 0.21` - WASM UI framework
- `wasm-bindgen` - WASM bindings
- `reqwasm` - WASM HTTP requests
- `web-sys` - Web APIs
- `log`/`wasm-logger` - Logging

## Kubernetes API Paths

The backend uses raw HTTP via reqwest to interact with K8s in-cluster:

| Endpoint | Purpose |
|----------|---------|
| `/api/v1/namespaces` | List namespaces |
| `/api/v1/namespaces/{ns}/secrets/{name}` | Get secrets |
| `/api/v1/namespaces/{ns}/pods/{name}` | Pod operations |
| `/apis/replication.storage.io/v1alpha1/replicationsources` | VolSync sources |
| `/apis/source.toolkit.fluxcd.io/v1beta2/helmreleases` | Flux HelmReleases |

## Frontend Build (Trunk)

```bash
# Install trunk and wasm-bindgen-cli first
cargo install trunk wasm-bindgen-cli

# Build WASM frontend
cd frontend && trunk build

# Output: frontend/dist/
```

## Docker Build

```bash
docker build -t volsync-webui:latest volsync-webui/
docker run -p 8080:8080 volsync-webui:latest
```

## Code Conventions

- Backend uses `thiserror` pattern for error enums
- Frontend uses function components with hooks (`use_state`, `use_effect_with`)
- All API responses are JSON
- Timestamp format: RFC3339 strings
- Yew components use `#[function_component]` and `#[derive(Properties, Clone, PartialEq)]`

## Common Issues

- **SSL errors**: System certs not in nix shell - use `nix develop --accept-flake-config`
- **WASM build**: Need `wasm-bindgen-cli` in PATH
- **K8s client**: Backend expects `KUBERNETES_SERVICE_HOST` env var (auto-set in cluster)