import { Ban, Copy, History, SquareTerminal, X } from "lucide-react";
import { useEffect, useState } from "react";
import { CommandInput } from "../components/terminal/CommandInput";
import { HistoryPanel } from "../components/terminal/HistoryPanel";
import { JobStatusBadge, jobDuration, statusIcon } from "../components/terminal/JobStatus";
import { OperationPanel } from "../components/terminal/OperationPanel";
import { OutputView } from "../components/terminal/OutputView";
import { Badge } from "../components/ui/Card";
import { useEngine } from "../context/EngineContext";
import { useToast } from "../context/ToastContext";
import { errorMessage, type HistoryEntry } from "../lib/api";
import { formatDuration, formatTime } from "../lib/format";

const HISTORY_TAB = "__history";

function useNow(active: boolean) {
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    if (!active) return;
    const t = window.setInterval(() => setNow(Date.now()), 500);
    return () => window.clearInterval(t);
  }, [active]);
  return now;
}

export function Terminal() {
  const { jobs, order, cancel, openFromHistory, closeJob } = useEngine();
  const toast = useToast();
  const [tab, setTab] = useState<string>(() => order[order.length - 1] ?? HISTORY_TAB);
  const job = tab !== HISTORY_TAB ? jobs[tab] : undefined;
  const now = useNow(job?.entry.status === "running");

  // If the selected tab disappears (closed), fall back to the newest job or history.
  useEffect(() => {
    if (tab !== HISTORY_TAB && !jobs[tab]) setTab(order[order.length - 1] ?? HISTORY_TAB);
  }, [tab, jobs, order]);

  const openHistory = async (e: HistoryEntry) => {
    try {
      await openFromHistory(e);
      setTab(e.id);
    } catch (err) {
      toast("error", "Could not open output", errorMessage(err));
    }
  };

  const doCancel = async (id: string) => {
    try {
      await cancel(id);
    } catch (e) {
      toast("warn", "Cannot cancel", errorMessage(e));
    }
  };

  const copyOutput = () => {
    if (!job) return;
    const text = [`> ${job.entry.command}`, ...job.lines.map((l) => l.text)].join("\n");
    navigator.clipboard.writeText(text).then(
      () => toast("success", "Output copied"),
      () => toast("error", "Clipboard unavailable"),
    );
  };

  const e = job?.entry;

  return (
    <div className="grid h-[calc(100vh-150px)] min-h-[520px] grid-cols-[320px_1fr] gap-5">
      <OperationPanel onStarted={setTab} />

      <section className="card flex min-h-0 flex-col overflow-hidden">
        <div className="flex items-center gap-1 overflow-x-auto border-b border-line/60 px-2 pt-2">
          <button
            onClick={() => setTab(HISTORY_TAB)}
            className={`flex shrink-0 items-center gap-1.5 rounded-t-xl px-3 py-2 text-[13px] ${
              tab === HISTORY_TAB ? "bg-raised text-fg" : "text-muted hover:text-fg"
            }`}
          >
            <History size={14} /> History
          </button>
          {order.map((id) => {
            const j = jobs[id];
            if (!j) return null;
            return (
              <div
                key={id}
                className={`group flex max-w-[220px] shrink-0 items-center gap-1.5 rounded-t-xl py-2 pl-3 pr-1.5 text-[13px] ${
                  tab === id ? "bg-raised text-fg" : "text-muted hover:text-fg"
                }`}
              >
                <button className="flex min-w-0 items-center gap-1.5" onClick={() => setTab(id)} title={j.entry.command}>
                  <span className={j.entry.status === "succeeded" ? "text-good" : j.entry.status === "running" ? "text-accent" : "text-bad"}>
                    {statusIcon(j.entry.status)}
                  </span>
                  <span className="truncate">{j.entry.title}</span>
                </button>
                {j.entry.status !== "running" && (
                  <button className="rounded p-0.5 opacity-60 hover:bg-line hover:opacity-100" onClick={() => closeJob(id)} aria-label="Close tab">
                    <X size={12} />
                  </button>
                )}
              </div>
            );
          })}
        </div>

        {tab === HISTORY_TAB || !job || !e ? (
          <HistoryPanel onOpen={openHistory} />
        ) : (
          <>
            <div className="border-b border-line/60 px-4 py-3">
              <div className="flex items-center gap-3">
                <SquareTerminal size={16} className="shrink-0 text-accent" />
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="truncate font-medium">{e.title}</span>
                    <JobStatusBadge status={e.status} />
                    {e.elevated && <Badge tone="accent">admin</Badge>}
                    {job.fromHistory && <Badge>from history</Badge>}
                  </div>
                  <div className="mt-0.5 text-xs text-muted">
                    {formatTime(e.startedAt)} · {formatDuration(jobDuration(e, now))}
                    {e.exitCode != null && ` · exit code ${e.exitCode}`}
                    {e.error && e.status !== "succeeded" && ` · ${e.error}`}
                  </div>
                </div>
                <button className="btn-ghost px-2.5 py-1.5 text-xs" onClick={copyOutput} disabled={!job.lines.length}>
                  <Copy size={13} /> Copy
                </button>
                {e.status === "running" && (
                  <button
                    className="btn-ghost px-2.5 py-1.5 text-xs"
                    disabled={!e.cancellable}
                    title={e.cancellable ? "Stop this operation" : "Stopping this operation midway is not safe"}
                    onClick={() => doCancel(e.id)}
                  >
                    <Ban size={13} /> Cancel
                  </button>
                )}
              </div>
              {e.status === "running" && (
                <div className="mt-3">
                  <div className="h-1.5 overflow-hidden rounded-full bg-raised">
                    {e.progress ? (
                      <div
                        className="h-full rounded-full bg-gradient-to-r from-accent to-accent2 transition-[width] duration-500"
                        style={{ width: `${e.progress.percent}%` }}
                      />
                    ) : (
                      <div className="h-full w-1/3 animate-[indeterminate_1.4s_ease-in-out_infinite] rounded-full bg-gradient-to-r from-accent to-accent2" />
                    )}
                  </div>
                  {e.progress && (
                    <div className="mt-1 flex justify-between text-[11px] text-muted">
                      <span>{e.progress.message ?? "Working…"}</span>
                      <span className="tabular-nums">{e.progress.percent.toFixed(0)}%</span>
                    </div>
                  )}
                </div>
              )}
              <pre className="selectable mt-2 truncate font-mono text-[11.5px] text-muted" title={e.command}>
                {e.shell === "cmd" ? "> " : "PS> "}
                {e.command}
              </pre>
            </div>
            <OutputView lines={job.lines} empty={e.status === "running" ? "Waiting for output…" : "This command produced no output."} />
          </>
        )}

        <CommandInput onStarted={setTab} />
      </section>
    </div>
  );
}
