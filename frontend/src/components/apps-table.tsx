import { RefreshCw } from "lucide-react";
import type { App } from "@/types";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";

interface Props {
  apps: App[];
  selectedApp: App | null;
  onSelect: (app: App | null) => void;
  refreshing: boolean;
  onRefresh: () => void;
}

function formatTime(iso: string | null): string {
  if (!iso) return "-";
  try {
    const d = new Date(iso);
    return d.toLocaleString();
  } catch {
    return iso;
  }
}

function formatDuration(secs: string | null): string {
  return secs ?? "-";
}

export function AppsTable({ apps, selectedApp, onSelect, refreshing, onRefresh }: Props) {
  return (
    <div className="rounded-lg border bg-card text-card-foreground">
      <div className="flex items-center justify-between p-6 pb-4">
        <h2 className="text-lg font-semibold leading-none tracking-tight">
          Applications ({apps.length})
        </h2>
        <Button variant="outline" size="sm" onClick={onRefresh} disabled={refreshing}>
          <RefreshCw className={`mr-1 h-4 w-4 ${refreshing ? "animate-spin" : ""}`} />
          {refreshing ? "Refreshing..." : "Refresh"}
        </Button>
      </div>
      <div className="p-6 pt-0">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>App</TableHead>
              <TableHead>Namespace</TableHead>
              <TableHead>Last Backup</TableHead>
              <TableHead>Duration</TableHead>
              <TableHead>Result</TableHead>
              <TableHead>Next Backup</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {apps.map((app) => {
              const isSelected =
                selectedApp?.name === app.name && selectedApp?.namespace === app.namespace;
              const resultOk =
                app.last_result?.toLowerCase() === "successful";
              return (
                <TableRow
                  key={`${app.name}/${app.namespace}`}
                  className={`cursor-pointer ${isSelected ? "bg-muted" : ""}`}
                  onClick={() => onSelect(isSelected ? null : app)}
                >
                  <TableCell className="font-medium">{app.name}</TableCell>
                  <TableCell className="text-muted-foreground">{app.namespace}</TableCell>
                  <TableCell>
                    {app.in_progress ? (
                      <span className="flex items-center gap-1 text-yellow-400">
                        <RefreshCw className="h-3 w-3 animate-spin" /> In progress
                      </span>
                    ) : (
                      formatTime(app.last_sync_time)
                    )}
                  </TableCell>
                  <TableCell>{formatDuration(app.last_sync_duration)}</TableCell>
                  <TableCell>
                    {app.last_result ? (
                      <Badge variant={resultOk ? "default" : "destructive"}>
                        {resultOk ? "✓ OK" : "✗ Failed"}
                      </Badge>
                    ) : (
                      <span className="text-muted-foreground">-</span>
                    )}
                  </TableCell>
                  <TableCell className="text-muted-foreground">
                    {formatTime(app.next_sync_time)}
                  </TableCell>
                </TableRow>
              );
            })}
            {apps.length === 0 && (
              <TableRow>
                <TableCell colSpan={6} className="text-center text-muted-foreground py-8">
                  No applications found
                </TableCell>
              </TableRow>
            )}
          </TableBody>
        </Table>
      </div>
    </div>
  );
}
