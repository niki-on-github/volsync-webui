import { useEffect, useState, useRef } from "react";
import { api } from "@/api";
import type { App, AppConfig } from "@/types";
import { AppsTable } from "@/components/apps-table";
import { AppDetail } from "@/components/app-detail";

export default function App() {
  const [apps, setApps] = useState<App[]>([]);
  const [selectedApp, setSelectedApp] = useState<App | null>(null);
  const [config, setConfig] = useState<AppConfig>({ refresh_interval_secs: 3600 });
  const [refreshing, setRefreshing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const refreshingRef = useRef(false);

  const loadApps = async () => {
    if (refreshingRef.current) return;
    refreshingRef.current = true;
    setRefreshing(true);
    try {
      const a = await api.listApps();
      setApps(a);
      setSelectedApp(prev => {
        if (!prev) return null;
        const updated = a.find(x => x.name === prev.name && x.namespace === prev.namespace);
        return updated ?? prev;
      });
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      refreshingRef.current = false;
      setRefreshing(false);
    }
  };

  const handleBackupComplete = async () => {
    await loadApps();
  };

  // Initial fetch: config + apps
  useEffect(() => {
    api.getConfig()
      .then(setConfig)
      .catch(() => {});
    loadApps();
  }, []);

  // Periodic refresh
  useEffect(() => {
    const interval = setInterval(loadApps, config.refresh_interval_secs * 1000);
    return () => clearInterval(interval);
  }, [config.refresh_interval_secs]);

  return (
    <div className="min-h-screen bg-background">
      <header className="border-b px-6 py-4">
        <div className="flex items-center justify-between">
          <h1 className="text-xl font-bold text-primary">VolSync WebUI</h1>
        </div>
      </header>

      {error && (
        <div className="px-6 pt-4">
          <div className="rounded-md bg-destructive/10 border border-destructive/30 p-3 text-sm text-destructive">
            {error}
          </div>
        </div>
      )}

      <main className="p-6">
        <div className="grid grid-cols-1 xl:grid-cols-3 gap-6">
          <div className="xl:col-span-2">
            <AppsTable
              apps={apps}
              selectedApp={selectedApp}
              onSelect={setSelectedApp}
              refreshing={refreshing}
              onRefresh={loadApps}
            />
          </div>
          <div>
            {selectedApp ? (
              <AppDetail app={selectedApp} onBackupComplete={handleBackupComplete} />
            ) : (
              <div className="rounded-lg border bg-card text-card-foreground p-6">
                <p className="text-sm text-muted-foreground">
                  Select an application from the table to view details
                </p>
              </div>
            )}
          </div>
        </div>
      </main>
    </div>
  );
}
