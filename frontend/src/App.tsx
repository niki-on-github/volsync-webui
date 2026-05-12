import { useEffect, useState, useRef } from "react";
import { api } from "@/api";
import type { App, AppConfig } from "@/types";
import { AppsTable } from "@/components/apps-table";
import { AppDetail } from "@/components/app-detail";
import { RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/button";

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
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      refreshingRef.current = false;
      setRefreshing(false);
    }
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
          <Button variant="outline" size="sm" onClick={loadApps} disabled={refreshing}>
            <RefreshCw className={`mr-1 h-4 w-4 ${refreshing ? "animate-spin" : ""}`} />
            {refreshing ? "Refreshing..." : "Refresh"}
          </Button>
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
              <AppDetail app={selectedApp} onBackupComplete={loadApps} />
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
