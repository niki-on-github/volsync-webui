import { useState, useEffect } from "react";
import { api } from "@/api";
import type { App, Snapshot } from "@/types";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
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
} from "lucide-react";

interface Props {
  app: App;
  onBackupComplete: () => void;
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
        {app.in_progress ? (
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

export function AppDetail({ app, onBackupComplete }: Props) {
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

  // Lazy-fetch destination repository
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
    if (backingUp) return;
    setBackingUp(true);
    setBackupStatus("Starting backup...");
    try {
      const r = await api.triggerBackup(app.name, app.namespace);
      const ok = r.result?.toLowerCase() === "successful";
      setBackupStatus(ok ? "Backup completed successfully" : `Backup failed: ${r.result ?? "unknown"}`);
      onBackupComplete();
    } catch (e) {
      setBackupStatus(`Error: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setBackingUp(false);
    }
  };

  const handleRestore = async () => {
    if (restoring) return;
    setRestoring(true);
    setRestoreStatus("Starting restore...");
    const trigger = `restore-${Date.now()}`;
    try {
      const r = await api.triggerRestore(
        app.name,
        app.namespace,
        trigger,
        timestamp === "__latest__" ? undefined : timestamp,
      );
      const ok = r.result?.toLowerCase() === "successful";
      setRestoreStatus(ok ? "Restore completed successfully" : `Restore result: ${r.result ?? "unknown"}`);
    } catch (e) {
      setRestoreStatus(`Error: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setRestoring(false);
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
                ? new Date(app.last_sync_time).toLocaleString()
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
                ? new Date(app.next_sync_time).toLocaleString()
                : "-"}
            </div>
            <div className="text-muted-foreground">Repository:</div>
            <div className="font-mono text-xs">{app.repository ?? "-"}</div>
          </div>
          <div className="flex items-center gap-3">
            <Button onClick={handleBackup} disabled={backingUp} size="sm">
              <Play className="mr-1 h-4 w-4" />
              {backingUp ? "Backing up..." : "Backup Now"}
            </Button>
            {backupStatus && (
              <span className="text-sm text-muted-foreground">{backupStatus}</span>
            )}
          </div>
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
                    <TableHead>Data</TableHead>
                    <TableHead>Host</TableHead>
                    <TableHead>Tags</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {snapshots.map((snap) => (
                    <TableRow key={snap.id}>
                      <TableCell className="font-mono text-xs">{snap.short_id}</TableCell>
                      <TableCell className="whitespace-nowrap text-xs">
                        {new Date(snap.time).toLocaleString()}
                      </TableCell>
                      <TableCell className="text-xs whitespace-nowrap">
                        <span className="text-green-400" title="new">+{snap.files_new}</span>{" "}
                        <span className="text-yellow-400" title="changed">~{snap.files_changed}</span>{" "}
                        <span className="text-muted-foreground" title="unmodified">{snap.files_unmodified}</span>
                        <span className="text-muted-foreground ml-1">({snap.total_files_processed})</span>
                      </TableCell>
                      <TableCell className="text-xs font-mono">
                        {snap.data_added > 0
                          ? snap.data_added > 1_000_000
                            ? `${(snap.data_added / 1_000_000).toFixed(1)}MB`
                            : `${(snap.data_added / 1_000).toFixed(0)}KB`
                          : "-"}
                      </TableCell>
                      <TableCell className="text-xs text-muted-foreground">{snap.hostname || "-"}</TableCell>
                      <TableCell>
                        <div className="flex flex-wrap gap-1">
                          {snap.tags.length > 0
                            ? snap.tags.map((tag) => (
                                <Badge key={tag} variant="secondary">{tag}</Badge>
                              ))
                            : <span className="text-muted-foreground text-xs">-</span>}
                        </div>
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
                  <SelectItem key={snap.id} value={snap.id}>
                    {snap.id.substring(0, 12)} —{" "}
                    {new Date(snap.time).toLocaleString()}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <div className="flex items-center gap-3">
            <Button
              onClick={handleRestore}
              disabled={
                restoring || !timestamp
              }
              size="sm"
              variant="destructive"
            >
              <RotateCcw className="mr-1 h-4 w-4" />
              {restoring ? "Restoring..." : "Restore"}
            </Button>
            {restoreStatus && (
              <span className="text-sm text-muted-foreground">
                {restoreStatus}
              </span>
            )}
          </div>
          {destLoaded && (
            <div className="grid grid-cols-2 gap-2 text-sm mt-3">
              <div className="text-muted-foreground">Repository:</div>
              <div className="font-mono text-xs">{destRepo ?? "none"}</div>
            </div>
          )}
        </Section>

      </CardContent>
    </Card>
  );
}
