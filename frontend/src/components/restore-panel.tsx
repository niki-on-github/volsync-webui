import { useState } from "react";
import { api } from "@/api";
import type { Snapshot } from "@/types";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardHeader,
  CardTitle,
  CardContent,
} from "@/components/ui/card";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

interface Props {
  appName: string;
  ns: string;
  snapshots: Snapshot[];
}

export function RestorePanel({ appName, ns, snapshots }: Props) {
  const [timestamp, setTimestamp] = useState("");
  const [restoring, setRestoring] = useState(false);
  const [status, setStatus] = useState("Ready");

  const handleRestore = async () => {
    if (restoring) return;
    setRestoring(true);
    setStatus("Starting restore...");
    const trigger = `restore-${Date.now()}`;
    try {
      const r = await api.triggerRestore(
        appName,
        ns,
        trigger,
        timestamp || undefined,
      );
      const success = r.result?.toLowerCase() === "successful";
      setStatus(
        success
          ? "Restore completed successfully"
          : `Restore completed with result: ${r.result ?? "unknown"}`,
      );
    } catch (e) {
      setStatus(
        `Restore error: ${e instanceof Error ? e.message : String(e)}`,
      );
    } finally {
      setRestoring(false);
    }
  };

  return (
    <Card>
      <CardHeader>
        <CardTitle>Restore</CardTitle>
      </CardHeader>
      <CardContent>
        <div className="mb-4">
          <label className="block text-sm font-medium text-muted-foreground mb-2">
            Select Snapshot (RFC3339 timestamp)
          </label>
          <Select value={timestamp} onValueChange={setTimestamp}>
            <SelectTrigger className="w-full">
              <SelectValue placeholder="Latest (no timestamp)" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="">Latest (no timestamp)</SelectItem>
              {snapshots.map((snap) => (
                <SelectItem key={snap.id} value={snap.time}>
                  {snap.id} - {snap.time}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        <Button
          onClick={handleRestore}
          disabled={restoring || !appName}
          variant="destructive"
        >
          {restoring ? "Restoring..." : "Restore"}
        </Button>
        <p className="text-sm text-muted-foreground mt-3">
          Status: {status}
        </p>
      </CardContent>
    </Card>
  );
}
