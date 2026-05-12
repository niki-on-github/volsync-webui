# VolSync WebUI - Development Guide

## Project Structure

```
volsync-webui/
├── backend/                    # Axum REST API
│   └── src/
│       ├── main.rs             # Entry point, router setup
│       ├── api.rs              # HTTP handlers
│       ├── kubectl.rs          # Kubernetes client (reqwest)
│       └── models.rs           # Data structures
├── frontend/                   # React + Vite + shadcn/ui
│   ├── index.html
│   ├── package.json
│   ├── vite.config.ts
│   ├── tailwind.config.ts
│   ├── tsconfig*.json
│   └── src/
│       ├── main.tsx
│       ├── App.tsx
│       ├── api.ts
│       ├── types.ts
│       ├── index.css
│       ├── lib/utils.ts
│       └── components/
│           ├── ui/             # shadcn primitives
│           ├── apps-table.tsx
│           └── app-detail.tsx
├── flake.nix                   # Nix dev shell
└── Dockerfile                  # Multi-stage build
```

## Development Commands

```bash
cd volsync-webui

# Enter dev shell
nix develop --accept-flake-config

# Backend
cargo check -p volsync-webui-backend
cargo build -p volsync-webui-backend

# Frontend
cd frontend && npm install && npm run dev

# Build frontend for production
cd frontend && npx tsc -b && npx vite build

# Format
cargo fmt
```

## Key Dependencies

### Backend
- `axum 0.7` - Web framework
- `tokio` - Async runtime
- `reqwest` - HTTP client (Kubernetes API)
- `serde`/`serde_json` - Serialization
- `chrono` - Date/time
- `tower-http` - CORS middleware

### Frontend
- `react 18` - UI framework
- `vite 6` - Build tool
- `tailwindcss 3` - CSS framework
- `shadcn/ui` - Component library
- `@radix-ui/react-select` - Select primitive
- `lucide-react` - Icons

## Kubernetes API Paths

| Endpoint | Purpose |
|----------|---------|
| `/api/v1/namespaces` | List namespaces |
| `/api/v1/namespaces/{ns}/pods` | Pod operations |
| `/api/v1/namespaces/{ns}/pods/{name}/log` | Pod logs |
| `/apis/volsync.backube/v1alpha1/replicationsources` | VolSync sources |
| `/apis/volsync.backube/v1alpha1/replicationdestinations` | VolSync destinations |
| `/apis/helm.toolkit.fluxcd.io/v2/helmreleases` | Flux HelmReleases |
| `/apis/apps/v1/deployments` | Deployments |

## Docker Build

```bash
docker build -t volsync-webui:latest .
docker run -p 8080:8080 volsync-webui:latest
```

## Code Conventions

- Backend uses custom `KubeError` enum (not `thiserror`)
- Frontend uses React function components with hooks
- All API responses are JSON
- Timestamp format: RFC3339 strings

## Common Issues

- **SSL errors**: System certs not in nix shell - use `nix develop --accept-flake-config`
- **K8s client**: Backend expects `KUBERNETES_SERVICE_HOST` env var (auto-set in cluster)
- **Frontend dev**: Run backend on port 8080, Vite proxies `/api` requests automatically
