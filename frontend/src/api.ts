import type { App, TaskStatus, Snapshot, UnlockResponse } from "./types";

const BASE = typeof window !== "undefined" ? window.location.origin : "http://localhost:8080";

async function fetchJson<T>(url: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`${BASE}${url}`, {
    headers: { "Content-Type": "application/json" },
    cache: "no-cache",
    ...init,
  });
  if (!res.ok) {
    const body = await res.text().catch(() => "");
    throw new Error(`HTTP ${res.status}${body ? `: ${body}` : ""}`);
  }
  return res.json();
}

function logError(context: string, e: unknown) {
  const msg = e instanceof Error ? e.message : String(e);
  console.error(`${context}: ${msg}`);
}

export const api = {
  async listApps(): Promise<App[]> {
    try {
      return await fetchJson<App[]>("/api/apps");
    } catch (e) {
      logError("listApps", e);
      throw e;
    }
  },

  async getSnapshots(app: string, ns: string): Promise<Snapshot[]> {
    try {
      return await fetchJson<Snapshot[]>(
        `/api/apps/${encodeURIComponent(app)}/${encodeURIComponent(ns)}/snapshots`,
      );
    } catch (e) {
      logError("getSnapshots", e);
      throw e;
    }
  },

  async triggerBackup(app: string, ns: string): Promise<TaskStatus> {
    try {
      return await fetchJson<TaskStatus>(
        `/api/apps/${encodeURIComponent(app)}/${encodeURIComponent(ns)}/backup`,
        { method: "POST" },
      );
    } catch (e) {
      logError("triggerBackup", e);
      throw e;
    }
  },

  async getBackupStatus(app: string, ns: string): Promise<TaskStatus | null> {
    try {
      return await fetchJson<TaskStatus | null>(
        `/api/apps/${encodeURIComponent(app)}/${encodeURIComponent(ns)}/backup/status`,
      );
    } catch {
      return null;
    }
  },

  async getRestoreStatus(app: string, ns: string): Promise<TaskStatus | null> {
    try {
      return await fetchJson<TaskStatus | null>(
        `/api/apps/${encodeURIComponent(app)}/${encodeURIComponent(ns)}/restore/status`,
      );
    } catch {
      return null;
    }
  },

  async getDestRepository(app: string, ns: string): Promise<string | null> {
    try {
      return await fetchJson<string | null>(
        `/api/apps/${encodeURIComponent(app)}/${encodeURIComponent(ns)}/destination/repository`,
      );
    } catch {
      return null;
    }
  },

  async triggerUnlock(app: string, ns: string): Promise<UnlockResponse> {
    try {
      return await fetchJson<UnlockResponse>(
        `/api/apps/${encodeURIComponent(app)}/${encodeURIComponent(ns)}/unlock`,
        { method: "POST" },
      );
    } catch (e) {
      logError("triggerUnlock", e);
      throw e;
    }
  },

  async triggerRestore(
    app: string,
    ns: string,
    timestamp?: string,
  ): Promise<TaskStatus> {
    try {
      return await fetchJson<TaskStatus>(
        `/api/apps/${encodeURIComponent(app)}/${encodeURIComponent(ns)}/restore`,
        {
          method: "POST",
          body: JSON.stringify({ timestamp: timestamp ?? null }),
        },
      );
    } catch (e) {
      logError("triggerRestore", e);
      throw e;
    }
  },
};
