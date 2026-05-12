# VolSync WebUI

A web-based management interface for VolSync replication and backup operations on Kubernetes.

> [!CAUTION]
> This project is 101% AI generated. The code was not yet reviewed by me. I make the repo public to allow easy testing. Dont expect to be anything functional.

## Features

- **Dashboard Table**: View all ReplicationSources in a sortable table with status, last backup time, duration, and result
- **Detail Panel**: Click a row to see snapshot history, backup status, and restore controls
- **Backup Operations**: Trigger manual backups for individual apps
- **Restore Operations**: Restore from any available snapshot timestamp via the detail panel
- **Auto-Refresh**: Periodically updates the app list with configurable interval (default: 1 hour, no-overlap guard)
- **Manual Refresh**: Refresh button in header to fetch latest data on demand

## Architecture

```
volsync-webui/
├── backend/                    # Axum-based REST API server
│   └── src/
│       ├── main.rs             # Server entry point, router setup
│       ├── api.rs              # HTTP handlers for all endpoints
│       ├── kubectl.rs          # Kubernetes API client (raw HTTP via reqwest)
│       └── models.rs           # Request/response data structures
├── frontend/                   # React + Vite web UI
│   ├── index.html
│   ├── package.json
│   ├── vite.config.ts
│   ├── tailwind.config.ts
│   └── src/
│       ├── main.tsx            # Entry point
│       ├── App.tsx             # Main layout with master-detail view
│       ├── api.ts              # Frontend API client (fetch)
│       ├── types.ts            # TypeScript interfaces
│       ├── index.css           # Tailwind + CSS variables
│       ├── lib/utils.ts        # cn() helper
│       └── components/
│           ├── apps-table.tsx  # Dashboard table with all apps
│           ├── app-detail.tsx  # Detail panel with snapshots/backup/restore
│           ├── snapshot-list.tsx
│           └── ui/             # shadcn primitives (Button, Card, Select, Table, Badge)
└── Dockerfile                  # Multi-stage container build
```

## Tech Stack

| Layer | Technology |
|-------|------------|
| Backend | Axum 0.7, Tokio, reqwest |
| Frontend | React 18, TypeScript, Vite 6, TailwindCSS 3, shadcn/ui |
| Kubernetes | Raw HTTP API via reqwest (in-cluster) |
| Container | Debian bookworm-slim |

## API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Health check |
| GET | `/api/config` | Frontend configuration (`{ refresh_interval_secs }`) |
| GET | `/api/namespaces` | List all namespaces |
| GET | `/api/apps` | List all ReplicationSources with full status |
| GET | `/api/apps/:app/:ns/snapshots` | Get snapshots for an app |
| POST | `/api/apps/:app/:ns/backup` | Trigger backup for app |
| POST | `/api/apps/:app/:ns/restore` | Trigger restore for app |
| POST | `/api/apps/backup-all` | Trigger backup for all apps |

### App Response Fields

The `/api/apps` endpoint returns extended status information for each ReplicationSource:

| Field | Type | Source |
|-------|------|--------|
| `name` | string | `metadata.name` |
| `namespace` | string | `metadata.namespace` |
| `last_sync_time` | string\|null | `status.lastSyncTime` |
| `last_sync_duration` | string\|null | `status.lastSyncDuration` (formatted as `{n.n}s`) |
| `last_result` | string\|null | `status.latestMoverStatus.result` |
| `next_sync_time` | string\|null | `status.nextSyncTime` |
| `in_progress` | bool | `status.conditions[].type == "Synchronizing" && status == "True"` |
| `paused` | bool | `spec.paused` |

## Development

### Prerequisites

- Rust 1.75+
- Node.js 18+
- Nix (for development shell, optional)

### Nix Dev Shell

```bash
cd volsync-webui
nix develop --accept-flake-config
```

### Build Commands

```bash
# Backend
cargo check -p volsync-webui-backend
cargo build -p volsync-webui-backend

# Frontend
cd frontend && npm install && npx tsc -b && npx vite build

# Format code
cargo fmt
```

### Frontend Dev Server

```bash
cd frontend
npm install
npm run dev
```

The Vite dev server runs on `http://localhost:5173` and proxies API calls to the backend running on port 8080. Make sure the backend is running separately:

```bash
cargo run -p volsync-webui-backend
```

## Deployment

### Docker

```bash
docker build -t volsync-webui:latest .
docker run -p 8080:8080 volsync-webui:latest
```

The container expects to run inside a Kubernetes cluster with access to the Kubernetes API.

### Kubernetes

Apply the RBAC configuration first, then deploy the container:

```bash
kubectl apply -f kube-manifests/rbac.yaml
kubectl apply -f your-deployment.yaml
```

## RBAC Permissions

The ServiceAccount requires these permissions. Create a `ClusterRole` + `ClusterRoleBinding`:

```yaml
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRole
metadata:
  name: volsync-webui
rules:
  - apiGroups: ["volsync.backube"]
    resources: ["replicationsources", "replicationdestinations"]
    verbs: ["get", "list", "patch", "watch"]
  - apiGroups: [""]
    resources: ["pods", "pods/log"]
    verbs: ["get", "list", "create", "delete"]
  - apiGroups: [""]
    resources: ["namespaces"]
    verbs: ["get", "list"]
  - apiGroups: ["apps"]
    resources: ["deployments", "deployments/scale"]
    verbs: ["get", "list", "patch"]
  - apiGroups: ["helm.toolkit.fluxcd.io"]
    resources: ["helmreleases"]
    verbs: ["get", "list", "patch"]
```

The app runs a startup RBAC check that probes each API endpoint and logs whether permissions are present. Missing permissions are non-fatal (logged as errors but the app continues).

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `RUST_LOG` | `debug` | Logging level (trace, debug, info, warn, error) |
| `KUBERNETES_SERVICE_HOST` | auto-detected | Kubernetes API server host |
| `VOLSYNC_API_GROUP` | `volsync.backube` | API group for VolSync CRDs (set to `replication.storage.io` for older clusters) |
| `VOLSYNC_SOURCE_SUFFIX` | `-backup` | Suffix on ReplicationSource CRD names (e.g. `gitea-backup`) |
| `VOLSYNC_DEST_SUFFIX` | `-bootstrap` | Suffix on ReplicationDestination CRD names (e.g. `gitea-bootstrap`) |
| `REFRESH_INTERVAL_SECS` | `3600` | Frontend auto-refresh interval in seconds (1 hour default) |
| `BACKUP_ALL_CONCURRENCY` | `5` | Max concurrent backups for backup-all |
| `POLL_TIMEOUT_SECS` | `300` | Backup/restore poll timeout in seconds |
| `POLL_INTERVAL_SECS` | `2` | Backup/restore poll interval in seconds |
| `POD_STARTUP_TIMEOUT_SECS` | `60` | Snapshot pod startup timeout in seconds |

### Suffix Configuration

The app name shown in the dashboard is the ReplicationSource CRD name (e.g. `gitea-backup`).
The destination CRD name is derived by stripping `VOLSYNC_SOURCE_SUFFIX` and appending `VOLSYNC_DEST_SUFFIX`:

```
gitea-backup  → strips -backup  → gitea  → appends -bootstrap  → gitea-bootstrap
```

### API Group Configuration

The default `VOLSYNC_API_GROUP=volsync.backube` matches modern VolSync installations. For older clusters still using the legacy API group, set:

```yaml
env:
  - name: VOLSYNC_API_GROUP
    value: "replication.storage.io"
```

## Kubernetes Compatibility

- Tested with Kubernetes 1.25+
- Uses `volsync.backube/v1alpha1` for VolSync CRDs (configurable via `VOLSYNC_API_GROUP`)
- Uses `helm.toolkit.fluxcd.io/v2` for HelmRelease (optional, for Flux-based apps)

## Security Considerations

1. **ServiceAccount**: Runs with a dedicated SA, not default
2. **ClusterRole**: Requires broad read access — restrict in production
3. **No TLS**: Backend runs HTTP; TLS should be handled by ingress/controller
4. **No Authentication**: Currently no auth — expose via auth proxy (e.g., OAuth2 proxy) in production
5. **Secret Access**: Reads the actual secret name from `spec.restic.repository` on each ReplicationSource at snapshot time, so it always uses the correct credentials regardless of naming convention

## License

MIT
