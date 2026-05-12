export interface App {
  name: string;
  namespace: string;
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
