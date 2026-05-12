import { useState } from "react";
import { api } from "@/api";
import type { BackupAllResponse } from "@/types";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardHeader,
  CardTitle,
  CardContent,
} from "@/components/ui/card";

interface Props {
  appName: string;
  ns: string;
}

export function BackupPanel({ appName, ns }: Props) {
  const [status, setStatus] = useState("Ready");
  const [backingUp, setBackingUp] = useState(false);
  const [backingUpAll, setBackingUpAll] = useState(false);
  const [backupAllResults, setBackupAllResults] =
    useState<BackupAllResponse | null>(null);

  const handleBackup = async () => {
    if (backingUp) return;
    setBackingUp(true);
    setStatus("Starting backup...");
    try {
      const r = await api.triggerBackup(appName, ns);
      const success = r.result?.toLowerCase() === "successful";
      setStatus(
        success
          ? "Backup completed successfully"
          : `Backup failed: ${r.result ?? "unknown"}`,
      );
    } catch (e) {
      setStatus(
        `Backup error: ${e instanceof Error ? e.message : String(e)}`,
      );
    } finally {
      setBackingUp(false);
    }
  };

  const handleBackupAll = async () => {
    if (backingUpAll) return;
    setBackingUpAll(true);
    setStatus("Starting backup for all apps...");
    try {
      const r = await api.triggerBackupAll();
      const failed = r.apps.filter((a) => !a.success).length;
      setStatus(
        failed === 0
          ? "All backups completed successfully"
          : `${failed} of ${r.apps.length} backups failed`,
      );
      setBackupAllResults(r);
    } catch (e) {
      setStatus(
        `Backup-all error: ${e instanceof Error ? e.message : String(e)}`,
      );
    } finally {
      setBackingUpAll(false);
    }
  };

  return (
    <Card>
      <CardHeader>
        <CardTitle>Backup</CardTitle>
      </CardHeader>
      <CardContent>
        <div className="flex gap-3 mb-4">
          <Button onClick={handleBackup} disabled={backingUp || !appName}>
            {backingUp ? "Backing up..." : "Backup"}
          </Button>
          <Button
            onClick={handleBackupAll}
            disabled={backingUpAll}
            variant="secondary"
          >
            {backingUpAll ? "Processing..." : "Backup All Apps"}
          </Button>
        </div>
        <p className="text-sm text-muted-foreground">Status: {status}</p>

        {backupAllResults && (
          <div className="mt-4">
            <h4 className="text-sm font-medium text-muted-foreground mb-2">
              Results:
            </h4>
            <div className="space-y-1">
              {backupAllResults.apps.map((r) => (
                <div
                  key={`${r.app}/${r.namespace}`}
                  className="flex items-center gap-2 text-sm"
                >
                  <span
                    className={
                      r.success ? "text-green-400" : "text-red-400"
                    }
                  >
                    {r.success ? "✓" : "✗"}
                  </span>
                  <span>{r.app}</span>
                  <span className="text-muted-foreground">
                    ({r.namespace})
                  </span>
                  {r.error && (
                    <span className="text-red-400 text-xs">{r.error}</span>
                  )}
                </div>
              ))}
            </div>
            {backupAllResults.summary && (
              <p className="text-xs text-muted-foreground mt-2">
                {backupAllResults.summary.success}/
                {backupAllResults.summary.total} succeeded
                {backupAllResults.summary.failed > 0 &&
                  `, ${backupAllResults.summary.failed} failed`}
              </p>
            )}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
