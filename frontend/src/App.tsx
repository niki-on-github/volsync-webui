import { useEffect, useState } from "react";
import { api } from "@/api";
import type { App, Snapshot } from "@/types";
import { NamespaceSelector } from "@/components/namespace-selector";
import { AppSelector } from "@/components/app-selector";
import { SnapshotList } from "@/components/snapshot-list";
import { BackupPanel } from "@/components/backup-panel";
import { RestorePanel } from "@/components/restore-panel";

export default function App() {
  const [namespaces, setNamespaces] = useState<string[]>([]);
  const [apps, setApps] = useState<App[]>([]);
  const [selectedNamespace, setSelectedNamespace] = useState("");
  const [selectedApp, setSelectedApp] = useState<App | null>(null);
  const [snapshots, setSnapshots] = useState<Snapshot[]>([]);
  const [loadingSnapshots, setLoadingSnapshots] = useState(false);

  useEffect(() => {
    api
      .listNamespaces()
      .then(setNamespaces)
      .catch((e: Error) => console.error("Failed to load namespaces:", e.message));
  }, []);

  useEffect(() => {
    setSelectedApp(null);
    setSnapshots([]);
    api
      .listApps(selectedNamespace || undefined)
      .then(setApps)
      .catch((e: Error) => console.error("Failed to load apps:", e.message));
  }, [selectedNamespace]);

  useEffect(() => {
    if (!selectedApp) return;
    let cancelled = false;
    setLoadingSnapshots(true);
    api
      .getSnapshots(selectedApp.name, selectedApp.namespace)
      .then((s) => {
        if (!cancelled) setSnapshots(s);
      })
      .catch((e: Error) => {
        if (!cancelled) {
          setSnapshots([]);
          console.warn("Failed to load snapshots:", e.message);
        }
      })
      .finally(() => {
        if (!cancelled) setLoadingSnapshots(false);
      });
    return () => {
      cancelled = true;
    };
  }, [selectedApp]);

  const handleRefresh = () => {
    if (!selectedApp) return;
    let cancelled = false;
    setLoadingSnapshots(true);
    api
      .getSnapshots(selectedApp.name, selectedApp.namespace)
      .then((s) => {
        if (!cancelled) setSnapshots(s);
      })
      .catch((e: Error) => {
        if (!cancelled) {
          setSnapshots([]);
          console.warn("Failed to refresh snapshots:", e.message);
        }
      })
      .finally(() => {
        if (!cancelled) setLoadingSnapshots(false);
      });
  };

  return (
    <div className="min-h-screen bg-background">
      <header className="border-b px-6 py-4">
        <div className="flex items-center justify-between">
          <h1 className="text-xl font-bold text-primary">VolSync WebUI</h1>
          <div className="flex items-center gap-4">
            <NamespaceSelector
              selected={selectedNamespace}
              namespaces={namespaces}
              onSelect={(val) => setSelectedNamespace(val === "__all__" ? "" : val)}
            />
            <AppSelector
              selected={selectedApp}
              apps={apps}
              onSelect={setSelectedApp}
            />
          </div>
        </div>
      </header>

      <main className="p-6">
        {selectedApp ? (
          <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
            <SnapshotList
              snapshots={snapshots}
              loading={loadingSnapshots}
              onRefresh={handleRefresh}
            />
            <BackupPanel
              appName={selectedApp.name}
              ns={selectedApp.namespace}
            />
            <RestorePanel
              appName={selectedApp.name}
              ns={selectedApp.namespace}
              snapshots={snapshots}
            />
          </div>
        ) : (
          <div className="flex items-center justify-center h-64">
            <p className="text-muted-foreground text-lg">
              {apps.length === 0
                ? "No applications found"
                : "Select an application to manage backups"}
            </p>
          </div>
        )}
      </main>
    </div>
  );
}
