export interface App {
  name: string;
  namespace: string;
  last_sync_time: string | null;
  last_sync_duration: string | null;
  last_result: string | null;
  next_sync_time: string | null;
  in_progress: boolean;
  paused: boolean;
}

export interface Snapshot {
  id: string;
  time: string;
  tags: string[];
}

export interface BackupResponse {
  trigger: string;
  status: string;
  result: string | null;
}

export interface RestoreRequest {
  trigger: string;
  timestamp: string | null;
}

export interface RestoreResponse {
  trigger: string;
  status: string;
  result: string | null;
}

export interface AppBackupStatus {
  app: string;
  namespace: string;
  success: boolean;
  error: string | null;
}

export interface BackupAllResponse {
  trigger: string;
  apps: AppBackupStatus[];
  summary: BackupSummary | null;
}

export interface BackupSummary {
  total: number;
  success: number;
  failed: number;
}

export interface AppConfig {
  refresh_interval_secs: number;
}
