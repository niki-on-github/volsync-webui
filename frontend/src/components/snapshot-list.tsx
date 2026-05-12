import { RefreshCw } from "lucide-react";
import type { Snapshot } from "@/types";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

interface Props {
  snapshots: Snapshot[];
  loading: boolean;
  onRefresh: () => void;
}

export function SnapshotList({ snapshots, loading, onRefresh }: Props) {
  return (
    <div className="rounded-lg border bg-card text-card-foreground">
      <div className="flex items-center justify-between p-6 pb-4">
        <h3 className="text-lg font-semibold leading-none tracking-tight">
          Snapshots
        </h3>
        <Button
          variant="outline"
          size="sm"
          onClick={onRefresh}
          disabled={loading}
        >
          <RefreshCw
            className={`mr-1 h-4 w-4 ${loading ? "animate-spin" : ""}`}
          />
          {loading ? "Loading..." : "Refresh"}
        </Button>
      </div>
      {snapshots.length === 0 && !loading ? (
        <div className="p-6 pt-0">
          <p className="text-sm text-muted-foreground">
            No snapshots found
          </p>
        </div>
      ) : (
        <div className="p-6 pt-0">
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
                  <TableCell className="font-mono text-xs">
                    {snap.id}
                  </TableCell>
                  <TableCell>{snap.time}</TableCell>
                  <TableCell>
                    <div className="flex flex-wrap gap-1">
                      {snap.tags.map((tag) => (
                        <Badge key={tag} variant="secondary">
                          {tag}
                        </Badge>
                      ))}
                    </div>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </div>
      )}
    </div>
  );
}
