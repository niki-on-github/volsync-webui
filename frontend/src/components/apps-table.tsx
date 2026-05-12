import { useState } from "react";
import { RefreshCw } from "lucide-react";
import type { App } from "@/types";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";

type SortKey = keyof App;

interface Props {
  apps: App[];
  selectedApp: App | null;
  onSelect: (app: App | null) => void;
  refreshing: boolean;
  onRefresh: () => void;
}

const SORT_LABELS: Record<SortKey, string> = {
  name: "App",
  namespace: "Namespace",
  last_sync_time: "Last Backup",
  last_sync_duration: "Duration",
  last_result: "Result",
  next_sync_time: "Next Backup",
  in_progress: "",
  paused: "",
  repository: "",
};

const SORT_COLUMNS: SortKey[] = [
  "name",
  "namespace",
  "last_sync_time",
  "last_sync_duration",
  "last_result",
  "next_sync_time",
];

function formatTime(iso: string | null): string {
  if (!iso) return "-";
  const d = new Date(iso);
  return isNaN(d.getTime()) ? iso : d.toLocaleString();
}

function formatDuration(secs: string | null): string {
  return secs ?? "-";
}

function sortValue(app: App, key: SortKey): string | number {
  if (key === "last_sync_duration") {
    const raw = app.last_sync_duration ?? "";
    const num = parseFloat(raw.replace(/[^0-9.]/g, ""));
    return isNaN(num) ? 0 : num;
  }
  const val = app[key];
  if (val == null) return "";
  if (typeof val === "boolean") return val ? 1 : 0;
  return String(val);
}

export function AppsTable({ apps, selectedApp, onSelect, refreshing, onRefresh }: Props) {
  const [sortKey, setSortKey] = useState<SortKey>("name");
  const [sortDir, setSortDir] = useState<"asc" | "desc">("asc");

  const handleSort = (key: SortKey) => {
    if (sortKey === key) {
      setSortDir(sortDir === "asc" ? "desc" : "asc");
    } else {
      setSortKey(key);
      setSortDir("asc");
    }
  };

  const sorted = [...apps].sort((a, b) => {
    const aVal = sortValue(a, sortKey);
    const bVal = sortValue(b, sortKey);
    if (typeof aVal === "number" && typeof bVal === "number") {
      return sortDir === "asc" ? aVal - bVal : bVal - aVal;
    }
    const cmp = String(aVal).localeCompare(String(bVal));
    return sortDir === "asc" ? cmp : -cmp;
  });

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
              {SORT_COLUMNS.map((key) => (
                <TableHead
                  key={key}
                  className="cursor-pointer select-none"
                  onClick={() => handleSort(key)}
                >
                  {SORT_LABELS[key]}
                  {sortKey === key && (
                    <span className="ml-1 text-xs">{sortDir === "asc" ? "▲" : "▼"}</span>
                  )}
                </TableHead>
              ))}
            </TableRow>
          </TableHeader>
          <TableBody>
            {sorted.map((app) => {
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
                      <Badge variant={resultOk ? "success" : "destructive"}>
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
            {sorted.length === 0 && (
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
