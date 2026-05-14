# API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Health check |
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
