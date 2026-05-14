export interface App {
  name: string;
  namespace: string;
  last_sync_time: string | null;
  last_sync_duration: string | null;
  last_result: string | null;
  next_sync_time: string | null;
  in_progress: boolean;
  paused: boolean;
  repository: string | null;
  repo_locked: boolean;
  backup_pending: boolean;
  restore_pending: boolean;
}

export interface TaskStatus {
  task_type: "backup" | "restore";
  app: string;
  namespace: string;
  status: "pending" | "running" | "completed" | "failed";
  result: string | null;
  error: string | null;
  started_at: string;
}

export interface UnlockResponse {
  message: string;
}

export interface Snapshot {
  id: string;
  short_id: string;
  time: string;
  tags: string[];
  paths: string[];
  hostname: string;
  files_new: number;
  files_changed: number;
  files_unmodified: number;
  data_added: number;
  total_files_processed: number;
  total_bytes_processed: number;
}

