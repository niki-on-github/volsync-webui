# VolSync WebUI

A web-based management interface for VolSync replication and backup operations on Kubernetes.

![Screenshot](preview.png)

## Features

- **Dashboard Table**: View all ReplicationSources in a sortable table with status, last backup time, duration, and result
- **Detail Panel**: Click a row to see snapshot history, backup status, and restore controls
- **Backup Operations**: Trigger manual backups for individual apps
- **Restore Operations**: Restore from any available snapshot timestamp via the detail panel
- **Auto-Refresh**: Periodically updates the app list with configurable interval (default: 1 hour, no-overlap guard)
- **Manual Refresh**: Refresh button in header to fetch latest data on demand

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

- Nix (for development shelll)

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

A example deployment of this repository is in the `./example/` direcotry.

### Kubernetes

The container expects to run inside a Kubernetes cluster with access to the Kubernetes API.

Apply the RBAC configuration from the section below, then deploy the container:

```bash
# Create the ClusterRole and ClusterRoleBinding (see RBAC section below)
kubectl apply -f - <<EOF
# ... paste the YAML from the RBAC section ...
EOF
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
    resources: ["replicationsources", "replicationdestinations", "replicationdestinations/status"]
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
  - apiGroups: [""]
    resources: ["persistentvolumeclaims"]
    verbs: ["get", "list", "delete"]
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
| `VOLSYNC_PVC_SUFFIX` | `-pvc` | Suffix on PVC name (e.g. `gitea-pvc`). The PVC name is derived as `{base_app_name}{suffix}`. |
| `REFRESH_INTERVAL_SECS` | `3600` | Frontend auto-refresh interval in seconds (1 hour default) |
| `BACKUP_ALL_CONCURRENCY` | `5` | Max concurrent backups for backup-all |
| `POLL_TIMEOUT_SECS` | `300` | Backup/restore poll timeout in seconds |
| `POLL_INTERVAL_SECS` | `2` | Backup/restore poll interval in seconds |
| `POD_STARTUP_TIMEOUT_SECS` | `60` | Snapshot pod startup timeout in seconds |
| `RESTIC_IMAGE` | `restic/restic:latest` | Restic container image for snapshot pods |

### Suffix Configuration

The app name shown in the dashboard is the ReplicationSource CRD name (e.g. `gitea-backup`).
The destination CRD name is derived by stripping `VOLSYNC_SOURCE_SUFFIX` and appending `VOLSYNC_DEST_SUFFIX`:

```
gitea-backup  → strips -backup  → gitea  → appends -bootstrap  → gitea-bootstrap
```

## Restore Workflow

The restore operation requires a specific PVC setup using Kubernetes Volume Populators. The application PVC must be defined with a `dataSourceRef` pointing to the `ReplicationDestination`:

```yaml
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: "${APP_NAME}-pvc"          # PVC name = {app_name} + VOLSYNC_PVC_SUFFIX (default "-pvc")
  namespace: ${APP_NAMESPACE}
spec:
  accessModes:
    - "ReadWriteOnce"
  resources:
    requests:
      storage: "${PVC_CAPACITY:-1Gi}"
  storageClassName: "openebs-zfspv"
  dataSourceRef:
    kind: ReplicationDestination
    apiGroup: volsync.backube
    name: "${APP_NAME}-bootstrap"   # Must match the ReplicationDestination CRD name
```

This `dataSourceRef` tells Kubernetes to populate the PVC from the ReplicationDestination's latest VolumeSnapshot when the PVC is created. The ReplicationDestination name must match the derived name: `{base_app_name}{VOLSYNC_DEST_SUFFIX}`.

### Optimized Restore Flow

The restore follows a zero-downtime sequence:

```
 1. Trigger restore on ReplicationDestination   ← app stays running
 2. Poll until restore completes                 ← VolSync downloads backup into temp PVC
 3. Suspend HelmRelease                          ← prevent Flux interference
 4. Scale down deployments to 0                  ← detach app from old PVC
 5. DELETE the application PVC                   ← old volume removed
 6. Unsuspend HelmRelease                        ← Flux wakes up, reconciles:
    ├─ Flux/Helm recreates PVC with dataSourceRef
    ├─ K8s binds PVC to the new VolSync snapshot
    └─ Flux scales deployment back to desired replicas
```

Key details:
- **Step 1-2**: VolSync restores data into a temporary PVC while the application continues serving traffic. The app only goes down at step 4.
- **Step 5**: Deleting the PVC is essential — Kubernetes only evaluates `dataSourceRef` at PVC creation time. Without deletion, the old PV stays bound.
- **Step 6**: Flux handles both PVC recreation and scaling. Manual scale-up is unnecessary.
- **HelmRelease required**: Restore only works for Flux-managed apps. Non-Flux apps will receive an error.

### Errors During Post-Restore

If PVC deletion fails, the restore task fails and the deployment is scaled back to its original replica count for safety. If PVC deletion succeeds but HelmRelease unsuspension fails, the restore data is ready but manual intervention is required: unsuspend the HelmRelease to trigger PVC recreation.

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
