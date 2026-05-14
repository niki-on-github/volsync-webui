import { useState, useEffect, useRef, useCallback } from "react";
import { api } from "@/api";
import type { App, Snapshot } from "@/types";
import { formatDateTime } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Collapsible, CollapsibleTrigger, CollapsibleContent } from "@/components/ui/collapsible";
import { LogStream } from "@/components/log-stream";
import {
  AlertDialog,
  AlertDialogTrigger,
  AlertDialogContent,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogCancel,
  AlertDialogAction,
} from "@/components/ui/alert-dialog";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Card, CardHeader, CardContent } from "@/components/ui/card";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  RefreshCw,
  Play,
  RotateCcw,
  Clock,
  CheckCircle2,
  XCircle,
  Unlock,
} from "lucide-react";

interface Props {
  app: App;
  onBackupComplete: () => void;
  onRestoreComplete: () => void;
}

function Section({ title, children }: { title: React.ReactNode; children: React.ReactNode }) {
  return (
    <div className="border-t border-border pt-4 first:border-t-0 first:pt-0">
      <h4 className="flex items-center gap-2 text-sm font-semibold text-foreground mb-3">
        {title}
      </h4>
      {children}
    </div>
  );
}

function AppHeader({ app }: { app: App }) {
  const resultOk = app.last_result?.toLowerCase() === "successful";
  return (
    <div className="flex items-start justify-between">
      <div>
        <h3 className="text-lg font-semibold leading-none tracking-tight">
          {app.name}
        </h3>
        <p className="text-sm text-muted-foreground mt-1">{app.namespace}</p>
      </div>
      <div className="flex items-center gap-2">
        {app.paused && <Badge variant="outline">Paused</Badge>}
        {app.restore_pending ? (
          <Badge variant="secondary">
            <RefreshCw className="mr-1 h-3 w-3 animate-spin" /> Restoring
          </Badge>
        ) : app.backup_pending ? (
          <Badge variant="secondary">
            <RefreshCw className="mr-1 h-3 w-3 animate-spin" /> Backing up
          </Badge>
        ) : app.in_progress ? (
          <Badge variant="secondary">
            <RefreshCw className="mr-1 h-3 w-3 animate-spin" /> Running
          </Badge>
        ) : app.last_result ? (
          <Badge variant={resultOk ? "success" : "destructive"}>
            {resultOk ? <CheckCircle2 className="mr-1 h-3 w-3" /> : <XCircle className="mr-1 h-3 w-3" />}
            {resultOk ? "OK" : "Failed"}
          </Badge>
        ) : null}
      </div>
    </div>
  );
}

export function AppDetail({ app, onBackupComplete, onRestoreComplete }: Props) {
  const [snapshots, setSnapshots] = useState<Snapshot[]>([]);
  const [loadingSnapshots, setLoadingSnapshots] = useState(false);
  const [backingUp, setBackingUp] = useState(false);
  const [restoring, setRestoring] = useState(false);
  const [timestamp, setTimestamp] = useState("");
  const [snapshotError, setSnapshotError] = useState<string | null>(null);
  const [backupStatus, setBackupStatus] = useState<string | null>(null);
  const [restoreStatus, setRestoreStatus] = useState<string | null>(null);
  const [destRepo, setDestRepo] = useState<string | null>(null);
  const [destLoaded, setDestLoaded] = useState(false);
  const [backupLogsOpen, setBackupLogsOpen] = useState(false);
  const [restoreLogsOpen, setRestoreLogsOpen] = useState(false);
  const [unlocking, setUnlocking] = useState(false);
  const [unlockStatus, setUnlockStatus] = useState<string | null>(null);

  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const mountedRef = useRef(true);
  const onBackupCompleteRef = useRef(onBackupComplete);
  onBackupCompleteRef.current = onBackupComplete;
  const onRestoreCompleteRef = useRef(onRestoreComplete);
  onRestoreCompleteRef.current = onRestoreComplete;

  const selectedSnap = timestamp && timestamp !== "__latest__"
    ? snapshots.find(s => s.time === timestamp)
    : null;

  const stopPolling = useCallback(() => {
    if (pollRef.current !== null) {
      clearInterval(pollRef.current);
      pollRef.current = null;
    }
  }, []);

  const startPolling = useCallback((taskType: "backup" | "restore") => {
    stopPolling();
    const poll = async () => {
      if (!mountedRef.current) return;
      const fn = taskType === "backup" ? api.getBackupStatus : api.getRestoreStatus;
      const status = await fn(app.name, app.namespace);
      if (!mountedRef.current) return;
      if (!status) {
        if (taskType === "backup") { setBackingUp(false); setBackupStatus(null); }
        else { setRestoring(false); setRestoreStatus(null); }
        stopPolling();
        return;
      }
      if (status.status === "completed") {
        stopPolling();
        if (taskType === "backup") {
          setBackingUp(false);
          setBackupStatus(status.result ? `Backup completed: ${status.result}` : "Backup completed");
          onBackupCompleteRef.current();
        } else {
          setRestoring(false);
          setRestoreStatus(status.result ? `Restore completed: ${status.result}` : "Restore completed");
          onRestoreCompleteRef.current();
        }
        return;
      }
      if (status.status === "failed") {
        stopPolling();
        const msg = status.error ?? "unknown error";
        if (taskType === "backup") {
          setBackingUp(false);
          setBackupStatus(`Backup failed: ${msg}`);
        } else {
          setRestoring(false);
          setRestoreStatus(`Restore failed: ${msg}`);
        }
        return;
      }
      const started = new Date(status.started_at).toLocaleTimeString();
      if (taskType === "backup") {
        setBackupStatus(`Backing up... (since ${started})`);
      } else {
        setRestoreStatus(`Restoring... (since ${started})`);
      }
    };
    poll();
    pollRef.current = setInterval(poll, 2000);
  }, [app.name, app.namespace, stopPolling]);

  // Resume polling if re-mounting with active tasks
  useEffect(() => {
    if (app.backup_pending) {
      setBackingUp(true);
      startPolling("backup");
    } else if (app.restore_pending) {
      setRestoring(true);
      startPolling("restore");
    }
    return stopPolling;
  }, [app.name, app.namespace, app.backup_pending, app.restore_pending, startPolling, stopPolling]);

  // Cleanup polling on unmount
  useEffect(() => {
    return stopPolling;
  }, [stopPolling]);

  // Track mount state to avoid setState on unmounted component
  useEffect(() => {
    mountedRef.current = true;
    return () => { mountedRef.current = false; };
  }, []);

  useEffect(() => {
    setTimestamp("");
    setBackupStatus(null);
    setRestoreStatus(null);
    setDestRepo(null);
    setDestLoaded(false);
    setSnapshots([]);
    setSnapshotError(null);
    let cancelled = false;
    setLoadingSnapshots(true);
    api.getSnapshots(app.name, app.namespace)
      .then((s) => { if (!cancelled) setSnapshots(s); })
      .catch((e: unknown) => {
        if (!cancelled) {
          setSnapshots([]);
          setSnapshotError(e instanceof Error ? e.message : String(e));
        }
      })
      .finally(() => { if (!cancelled) setLoadingSnapshots(false); });
    return () => { cancelled = true; };
  }, [app.name, app.namespace]);

  useEffect(() => {
    let cancelled = false;
    setDestRepo(null);
    setDestLoaded(false);
    api.getDestRepository(app.name, app.namespace).then((r) => {
      if (!cancelled) {
        setDestRepo(r);
        setDestLoaded(true);
      }
    });
    return () => { cancelled = true; };
  }, [app.name, app.namespace]);

  const handleBackup = async () => {
    if (backingUp || app.backup_pending) return;
    setBackingUp(true);
    setBackupStatus("Starting backup...");
    try {
      await api.triggerBackup(app.name, app.namespace);
      startPolling("backup");
    } catch (e) {
      setBackingUp(false);
      setBackupStatus(`Error: ${e instanceof Error ? e.message : String(e)}`);
    }
  };

  const handleRestore = async () => {
    if (restoring || app.restore_pending) return;
    setRestoring(true);
    setRestoreStatus("Starting restore...");
    try {
      await api.triggerRestore(
        app.name,
        app.namespace,
        timestamp === "__latest__" ? undefined : timestamp,
      );
      startPolling("restore");
    } catch (e) {
      setRestoring(false);
      setRestoreStatus(`Error: ${e instanceof Error ? e.message : String(e)}`);
    }
  };

  useEffect(() => {
    if (backingUp) setBackupLogsOpen(true);
  }, [backingUp]);

  useEffect(() => {
    if (restoring) setRestoreLogsOpen(true);
  }, [restoring]);

  const backupLocked = backingUp || app.backup_pending || restoring || app.restore_pending;
  const restoreLocked = restoring || app.restore_pending || app.backup_pending;

  const handleUnlock = async () => {
    if (unlocking) return;
    setUnlocking(true);
    setUnlockStatus("Starting unlock...");
    try {
      const resp = await api.triggerUnlock(app.name, app.namespace);
      setUnlockStatus(resp.message);
    } catch (e) {
      setUnlockStatus(`Error: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setUnlocking(false);
    }
  };

  return (
    <Card>
      <CardHeader>
        <AppHeader app={app} />
      </CardHeader>
      <CardContent className="space-y-4">

        {/* ── Backup Section ── */}
        <Section title={
          <><Play className="h-4 w-4" /> Backup</>
        }>
          <div className="grid grid-cols-2 gap-2 text-sm mb-3">
            <div className="text-muted-foreground">Last backup:</div>
            <div>
              {app.last_sync_time
                ? formatDateTime(app.last_sync_time)
                : "-"}
            </div>
            <div className="text-muted-foreground">Duration:</div>
            <div>{app.last_sync_duration ?? "-"}</div>
            <div className="text-muted-foreground">Result:</div>
            <div>
              {app.last_result ? (
                <Badge
                  variant={
                    app.last_result.toLowerCase() === "successful"
                      ? "success"
                      : "destructive"
                  }
                >
                  {app.last_result}
                </Badge>
              ) : (
                "-"
              )}
            </div>
            <div className="text-muted-foreground">Next backup:</div>
            <div>
              {app.next_sync_time
                ? formatDateTime(app.next_sync_time)
                : "-"}
            </div>
            <div className="text-muted-foreground">Repository:</div>
            <div className="font-mono text-xs">{app.repository ?? "-"}</div>
          </div>
          {app.repo_locked && (
            <div className="rounded-md bg-amber-500/10 border border-amber-500/30 p-3 text-sm">
              <strong className="text-amber-500">Repository is locked</strong>
              <p className="text-muted-foreground mt-1">
                The last backup failed because the repository is locked.
                Use the <strong className="text-amber-500">Unlock Repository</strong> button below to remove stale locks.
              </p>
            </div>
          )}
          <div className="flex items-center gap-3">
            <Button onClick={handleBackup} disabled={backupLocked} size="sm">
              <Play className="mr-1 h-4 w-4" />
              {backupLocked ? "Backing up..." : "Backup Now"}
            </Button>
            {backupStatus && (
              <span className="text-sm text-muted-foreground">{backupStatus}</span>
            )}
          </div>
          <Collapsible open={backupLogsOpen} onOpenChange={setBackupLogsOpen}>
            <CollapsibleTrigger asChild>
              <Button variant="ghost" size="sm" className="mt-1 h-7 px-2">
                {backupLogsOpen ? "▲" : "▼"} Mover Logs
              </Button>
            </CollapsibleTrigger>
            <CollapsibleContent>
              <LogStream
                app={app.name}
                namespace={app.namespace}
                logType="backup"
                active={backingUp}
              />
            </CollapsibleContent>
          </Collapsible>
        </Section>

        {/* ── Snapshots Section ── */}
        <Section title={
          <><Clock className="h-4 w-4" /> Snapshots ({snapshots.length})</>
        }>
          {loadingSnapshots ? (
            <p className="text-sm text-muted-foreground">Loading snapshots...</p>
          ) : snapshotError ? (
            <p className="text-sm text-destructive">Failed: {snapshotError}</p>
          ) : snapshots.length === 0 ? (
            <p className="text-sm text-muted-foreground">No snapshots found</p>
          ) : (
            <div className="overflow-x-auto">
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>ID</TableHead>
                    <TableHead>Time</TableHead>
                    <TableHead>Files</TableHead>
                    <TableHead>Size</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {snapshots.map((snap) => (
                    <TableRow key={snap.id}>
                      <TableCell className="font-mono text-xs">{snap.short_id}</TableCell>
                      <TableCell className="whitespace-nowrap text-xs">
                        {formatDateTime(snap.time)}
                      </TableCell>
                      <TableCell className="text-xs whitespace-nowrap">
                        <span className="text-green-400" title="new">+{snap.files_new}</span>{" "}
                        <span className="text-yellow-400" title="changed">~{snap.files_changed}</span>{" "}
                        <span className="text-muted-foreground" title="unmodified">{snap.files_unmodified}</span>
                        <span className="text-muted-foreground ml-1">({snap.total_files_processed})</span>
                      </TableCell>
                      <TableCell className="text-xs font-mono">
                        {snap.total_bytes_processed > 0
                          ? snap.total_bytes_processed > 1_000_000_000
                            ? `${(snap.total_bytes_processed / 1_000_000_000).toFixed(2)}GB`
                            : snap.total_bytes_processed > 1_000_000
                              ? `${(snap.total_bytes_processed / 1_000_000).toFixed(1)}MB`
                              : `${(snap.total_bytes_processed / 1_000).toFixed(0)}KB`
                          : "-"}
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </div>
          )}
        </Section>

        {/* ── Restore Section ── */}
        <Section title={
          <><RotateCcw className="h-4 w-4" /> Restore</>
        }>
          <div className="mb-3">
            <label className="block text-sm text-muted-foreground mb-1">
              Select snapshot to restore from
            </label>
            <Select value={timestamp} onValueChange={setTimestamp}>
              <SelectTrigger className="w-full">
                <SelectValue placeholder="Choose a snapshot..." />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="__latest__">Latest (no timestamp)</SelectItem>
                {snapshots.map((snap) => (
                  <SelectItem key={snap.id} value={snap.time}>
                    {snap.id.substring(0, 12)} —{" "}
                    {formatDateTime(snap.time)}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <div className="flex items-center gap-3">
            <AlertDialog>
              <AlertDialogTrigger asChild>
                <Button
                  disabled={restoreLocked || !timestamp}
                  size="sm"
                  variant="destructive"
                >
                  <RotateCcw className="mr-1 h-4 w-4" />
                  {restoreLocked ? "Restoring..." : "Restore"}
                </Button>
              </AlertDialogTrigger>
              <AlertDialogContent>
                <AlertDialogHeader>
                  <AlertDialogTitle>Confirm Restore</AlertDialogTitle>
                  <AlertDialogDescription>
                    Do you really want to restore{" "}
                    {selectedSnap
                      ? `${selectedSnap.short_id} — ${formatDateTime(selectedSnap.time)}`
                      : "the selected snapshot"}
                    ?
                  </AlertDialogDescription>
                </AlertDialogHeader>
                <AlertDialogFooter>
                  <AlertDialogCancel>No</AlertDialogCancel>
                  <AlertDialogAction onClick={handleRestore} className="bg-destructive text-destructive-foreground hover:bg-destructive/90">
                    Yes
                  </AlertDialogAction>
                </AlertDialogFooter>
              </AlertDialogContent>
            </AlertDialog>
            {restoreStatus && (
              <span className="text-sm text-muted-foreground">
                {restoreStatus}
              </span>
            )}
          </div>
          <Collapsible open={restoreLogsOpen} onOpenChange={setRestoreLogsOpen}>
            <CollapsibleTrigger asChild>
              <Button variant="ghost" size="sm" className="mt-1 h-7 px-2">
                {restoreLogsOpen ? "▲" : "▼"} Mover Logs
              </Button>
            </CollapsibleTrigger>
            <CollapsibleContent>
              <LogStream
                app={app.name}
                namespace={app.namespace}
                logType="restore"
                active={restoring}
              />
            </CollapsibleContent>
          </Collapsible>
          {destLoaded && (
            <div className="grid grid-cols-2 gap-2 text-sm mt-3">
              <div className="text-muted-foreground">Repository:</div>
              <div className="font-mono text-xs">{destRepo ?? "none"}</div>
            </div>
          )}
        </Section>

        {/* ── Unlock Section ── */}
        <div className="border-t border-border pt-3">
          <div className="flex items-center gap-3">
            <AlertDialog>
              <AlertDialogTrigger asChild>
                <Button disabled={unlocking} size="sm" variant="outline">
                  <Unlock className="mr-1 h-4 w-4" />
                  {unlocking ? "Unlocking..." : "Unlock Repository"}
                </Button>
              </AlertDialogTrigger>
              <AlertDialogContent>
                <AlertDialogHeader>
                  <AlertDialogTitle>Confirm Unlock</AlertDialogTitle>
                  <AlertDialogDescription>
                    Do you really want to unlock the repository <strong>{app.name}</strong>?
                  </AlertDialogDescription>
                </AlertDialogHeader>
                <AlertDialogFooter>
                  <AlertDialogCancel>No</AlertDialogCancel>
                  <AlertDialogAction onClick={handleUnlock}>Yes</AlertDialogAction>
                </AlertDialogFooter>
              </AlertDialogContent>
            </AlertDialog>
            {unlockStatus && (
              <span className="text-sm text-muted-foreground">{unlockStatus}</span>
            )}
          </div>
        </div>

      </CardContent>
    </Card>
  );
}
