import { useState, useEffect, useRef } from "react";
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
import {
  Card,
  CardHeader,
  CardTitle,
  CardContent,
} from "@/components/ui/card";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { RefreshCw, Play, RotateCcw } from "lucide-react";

interface Props {
  app: App;
  onBackupComplete: () => void;
}

export function AppDetail({ app, onBackupComplete }: Props) {
  const [snapshots, setSnapshots] = useState<Snapshot[]>([]);
  const [loadingSnapshots, setLoadingSnapshots] = useState(false);
  const [backingUp, setBackingUp] = useState(false);
  const [restoring, setRestoring] = useState(false);
  const [timestamp, setTimestamp] = useState("");
  const [status, setStatus] = useState("");
  const mounted = useRef(true);

  useEffect(() => {
    return () => { mounted.current = false; };
  }, []);

  useEffect(() => {
    setTimestamp("");
    setStatus("");
    setSnapshots([]);
    let cancelled = false;
    setLoadingSnapshots(true);
    api.getSnapshots(app.name, app.namespace)
      .then((s) => { if (!cancelled) setSnapshots(s); })
      .catch((e: Error) => {
        if (!cancelled) {
          setSnapshots([]);
          setStatus(`Failed to load snapshots: ${e.message}`);
        }
      })
      .finally(() => { if (!cancelled) setLoadingSnapshots(false); });
    return () => { cancelled = true; };
  }, [app.name, app.namespace]);

  const handleBackup = async () => {
    if (backingUp) return;
    setBackingUp(true);
    setStatus("Starting backup...");
    try {
      const r = await api.triggerBackup(app.name, app.namespace);
      const ok = r.result?.toLowerCase() === "successful";
      setStatus(ok ? "Backup completed successfully" : `Backup failed: ${r.result ?? "unknown"}`);
      onBackupComplete();
    } catch (e) {
      setStatus(`Backup error: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setBackingUp(false);
    }
  };

  const handleRestore = async () => {
    if (restoring) return;
    setRestoring(true);
    setStatus("Starting restore...");
    const trigger = `restore-${Date.now()}`;
    try {
      const r = await api.triggerRestore(
        app.name,
        app.namespace,
        trigger,
        timestamp === "__latest__" ? undefined : timestamp,
      );
      const ok = r.result?.toLowerCase() === "successful";
      setStatus(ok ? "Restore completed successfully" : `Restore result: ${r.result ?? "unknown"}`);
    } catch (e) {
      setStatus(`Restore error: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setRestoring(false);
    }
  };

  const resultOk = app.last_result?.toLowerCase() === "successful";

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center justify-between">
          <span>
            {app.name}
            <span className="ml-2 text-sm font-normal text-muted-foreground">
              ({app.namespace})
            </span>
          </span>
          {app.in_progress && (
            <Badge variant="secondary" className="ml-2">
              <RefreshCw className="mr-1 h-3 w-3 animate-spin" /> Running
            </Badge>
          )}
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-4">
        {/* Status summary */}
        <div className="grid grid-cols-2 gap-2 text-sm">
          <div>
            <span className="text-muted-foreground">Last backup:</span>{" "}
            {app.last_sync_time
              ? new Date(app.last_sync_time).toLocaleString()
              : "-"}
          </div>
          <div>
            <span className="text-muted-foreground">Duration:</span>{" "}
            {app.last_sync_duration ?? "-"}
          </div>
          <div>
            <span className="text-muted-foreground">Result:</span>{" "}
            {app.last_result ? (
              <Badge variant={resultOk ? "default" : "destructive"} className="ml-1">
                {resultOk ? "Successful" : app.last_result}
              </Badge>
            ) : (
              "-"
            )}
          </div>
          <div>
            <span className="text-muted-foreground">Next backup:</span>{" "}
            {app.next_sync_time
              ? new Date(app.next_sync_time).toLocaleString()
              : "-"}
          </div>
          {app.paused && (
            <div className="col-span-2">
              <Badge variant="outline">Paused</Badge>
            </div>
          )}
        </div>

        {/* Actions */}
        <div className="flex gap-2">
          <Button onClick={handleBackup} disabled={backingUp} size="sm">
            <Play className="mr-1 h-4 w-4" />
            {backingUp ? "Backing up..." : "Backup Now"}
          </Button>
          <Button
            onClick={handleRestore}
            disabled={restoring || !timestamp || timestamp === "__latest__"}
            size="sm"
            variant="destructive"
          >
            <RotateCcw className="mr-1 h-4 w-4" />
            {restoring ? "Restoring..." : "Restore"}
          </Button>
        </div>

        {/* Restore snapshot selector */}
        <div>
          <label className="block text-sm font-medium text-muted-foreground mb-1">
            Select snapshot to restore
          </label>
          <Select value={timestamp} onValueChange={setTimestamp}>
            <SelectTrigger className="w-full">
              <SelectValue placeholder="Choose a snapshot..." />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="__latest__">Latest (no timestamp)</SelectItem>
              {snapshots.map((snap) => (
                <SelectItem key={snap.id} value={snap.time}>
                  {snap.id.substring(0, 12)} — {new Date(snap.time).toLocaleString()}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>

        {/* Status message */}
        {status && (
          <p className="text-sm text-muted-foreground">{status}</p>
        )}

        {/* Snapshots list */}
        <div>
          <h4 className="text-sm font-medium text-muted-foreground mb-2">Snapshots</h4>
          {loadingSnapshots ? (
            <p className="text-sm text-muted-foreground">Loading snapshots...</p>
          ) : snapshots.length === 0 ? (
            <p className="text-sm text-muted-foreground">No snapshots found</p>
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>ID</TableHead>
                  <TableHead>Time</TableHead>
                  <TableHead>Tags</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {snapshots.map((snap) => (
                  <TableRow key={snap.id}>
                    <TableCell className="font-mono text-xs">{snap.id.substring(0, 12)}</TableCell>
                    <TableCell>{new Date(snap.time).toLocaleString()}</TableCell>
                    <TableCell>
                      <div className="flex flex-wrap gap-1">
                        {snap.tags.map((tag) => (
                          <Badge key={tag} variant="secondary">{tag}</Badge>
                        ))}
                      </div>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </div>
      </CardContent>
    </Card>
  );
}
