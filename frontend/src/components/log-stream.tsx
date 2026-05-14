import { useEffect, useRef, useState } from "react";

interface Props {
  app: string;
  namespace: string;
  logType: "backup" | "restore";
  active: boolean;
}

type LogStatus = "idle" | "waiting" | "streaming" | "done" | "error";

interface LogLine {
  text: string;
  type: "log" | "status" | "error";
}

export function LogStream({ app, namespace, logType, active }: Props) {
  const [lines, setLines] = useState<LogLine[]>([]);
  const [status, setStatus] = useState<LogStatus>("idle");
  const containerRef = useRef<HTMLDivElement>(null);
  const [autoScroll, setAutoScroll] = useState(true);
  const sourceRef = useRef<EventSource | null>(null);
  const errorRef = useRef(false);
  const linesRef = useRef(lines);
  linesRef.current = lines;

  const clear = () => {
    setLines([]);
    setStatus("idle");
    errorRef.current = false;
  };

  useEffect(() => {
    if (!active) {
      sourceRef.current?.close();
      sourceRef.current = null;
      return;
    }

    setStatus("waiting");
    errorRef.current = false;
    setLines([]);

    const url = `/api/apps/${encodeURIComponent(app)}/${encodeURIComponent(namespace)}/mover-logs?type=${logType}`;
    const source = new EventSource(url);
    sourceRef.current = source;

    source.onmessage = (event) => {
      if (errorRef.current) {
        errorRef.current = false;
        setStatus("streaming");
      }
      try {
        const data = JSON.parse(event.data);
        if (data.status) {
          switch (data.status) {
            case "waiting":
              setStatus("waiting");
              setLines((prev) => [...prev, { text: data.message || "Waiting...", type: "status" as const }]);
              break;
            case "streaming":
              setStatus("streaming");
              break;
            case "done":
              setStatus("done");
              if (data.result === "failed") {
                setLines((prev) => [...prev, { text: data.error || "Operation failed", type: "error" as const }]);
              } else {
                setLines((prev) => [...prev, { text: "Operation completed", type: "status" as const }]);
              }
              source.close();
              sourceRef.current = null;
              break;
          }
        } else if (data.line) {
          setLines((prev) => {
            const next = [...prev, { text: data.line, type: "log" as const }];
            return next.length > 2000 ? next.slice(-2000) : next;
          });
        }
      } catch {
        setLines((prev) => [...prev, { text: event.data, type: "log" as const }]);
      }
    };

    source.onerror = () => {
      if (!errorRef.current) {
        errorRef.current = true;
        setStatus("error");
        setLines((prev) => [...prev, { text: "Connection lost — reconnecting...", type: "error" as const }]);
      }
    };

    return () => {
      source.close();
      sourceRef.current = null;
    };
  }, [app, namespace, logType, active]);

  useEffect(() => {
    clear();
  }, [app, namespace, logType]);

  useEffect(() => {
    if (autoScroll && containerRef.current) {
      containerRef.current.scrollTop = containerRef.current.scrollHeight;
    }
  }, [lines, autoScroll]);

  const handleScroll = () => {
    if (!containerRef.current) return;
    const { scrollTop, scrollHeight, clientHeight } = containerRef.current;
    setAutoScroll(scrollHeight - scrollTop - clientHeight < 50);
  };

  if (!active && lines.length === 0) {
    return null;
  }

  const statusBadge = () => {
    switch (status) {
      case "waiting":
        return <span className="text-yellow-400 animate-pulse">Connecting...</span>;
      case "streaming":
        return <><span className="h-1.5 w-1.5 rounded-full bg-green-400 animate-pulse inline-block mr-1" /> Streaming</>;
      case "done":
        return <span className="text-blue-400">Complete</span>;
      case "error":
        return <span className="text-red-400">Reconnecting...</span>;
      default:
        return <span className="text-muted-foreground">Idle</span>;
    }
  };

  return (
    <div className="rounded-md border border-border overflow-hidden mt-2">
      <div className="flex items-center justify-between bg-muted/50 px-3 py-1.5 text-xs">
        <div className="flex items-center gap-2">
          {statusBadge()}
          <span className="text-muted-foreground">
            ({lines.filter((l) => l.type === "log").length} lines)
          </span>
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={clear}
            className="text-muted-foreground hover:text-foreground transition-colors"
          >
            Clear
          </button>
          {active && (
            <button
              onClick={() => { sourceRef.current?.close(); sourceRef.current = null; setStatus("done"); }}
              className="text-muted-foreground hover:text-foreground transition-colors"
            >
              Stop
            </button>
          )}
        </div>
      </div>
      <div
        ref={containerRef}
        onScroll={handleScroll}
        className="bg-zinc-950 text-green-400 font-mono text-xs p-3 overflow-y-auto whitespace-pre-wrap break-all"
        style={{ maxHeight: "300px", minHeight: "60px" }}
      >
        {lines.length === 0 ? (
          <span className="text-muted-foreground">Waiting for log output...</span>
        ) : (
          lines.map((line, i) => (
            <div
              key={i}
              className={
                line.type === "status" ? "text-yellow-400" :
                line.type === "error" ? "text-red-400" :
                ""
              }
            >
              {line.text}
            </div>
          ))
        )}
      </div>
    </div>
  );
}
