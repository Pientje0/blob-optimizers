import { listen } from "@tauri-apps/api/event";
import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { api, type HistoryEntry, type Operation, type OutputLine, type Shell } from "../lib/api";

/** Lines kept per job in the UI; the backend log keeps the full output. */
const MAX_LINES = 5000;

export interface SessionJob {
  entry: HistoryEntry;
  lines: OutputLine[];
  /** Output was loaded from the history log rather than streamed live. */
  fromHistory?: boolean;
}

interface EngineState {
  catalog: Operation[];
  /** Jobs started or opened during this app session, newest last. */
  jobs: Record<string, SessionJob>;
  order: string[];
  history: HistoryEntry[];
  running: HistoryEntry[];
  runOperation: (id: string, params?: Record<string, string>) => Promise<HistoryEntry>;
  runCommand: (shell: Shell, command: string, confirmed: boolean, asAdmin: boolean) => Promise<HistoryEntry>;
  cancel: (id: string) => Promise<void>;
  /** Shows a job started by another module (apps, tweaks...) in the Terminal tab. */
  track: (entry: HistoryEntry) => void;
  /** Resolves with the final entry once the job has finished (immediately if it already has). */
  waitFor: (id: string) => Promise<HistoryEntry>;
  openFromHistory: (entry: HistoryEntry) => Promise<void>;
  closeJob: (id: string) => void;
  clearHistory: () => Promise<void>;
}

const EngineContext = createContext<EngineState | null>(null);

function appendLines(existing: OutputLine[], incoming: OutputLine[]): OutputLine[] {
  const out = existing.slice();
  for (const l of incoming) {
    // An in-place update (\r) replaces the previous transient line, like a real console.
    if (out.length && out[out.length - 1].transient) out.pop();
    out.push(l);
  }
  return out.length > MAX_LINES ? out.slice(out.length - MAX_LINES) : out;
}

export function EngineProvider({ children }: { children: ReactNode }) {
  const [catalog, setCatalog] = useState<Operation[]>([]);
  const [jobs, setJobs] = useState<Record<string, SessionJob>>({});
  const [order, setOrder] = useState<string[]>([]);
  const [history, setHistory] = useState<HistoryEntry[]>([]);
  const finished = useRef(new Map<string, HistoryEntry>());
  const waiters = useRef(new Map<string, ((e: HistoryEntry) => void)[]>());

  const waitFor = useCallback((id: string) => {
    const done = finished.current.get(id);
    if (done) return Promise.resolve(done);
    return new Promise<HistoryEntry>((resolve) => {
      waiters.current.set(id, [...(waiters.current.get(id) ?? []), resolve]);
    });
  }, []);

  const upsert = useCallback((entry: HistoryEntry, lines?: OutputLine[], fromHistory = false) => {
    setJobs((j) => ({
      ...j,
      [entry.id]: { entry, lines: lines ?? j[entry.id]?.lines ?? [], fromHistory: fromHistory || j[entry.id]?.fromHistory },
    }));
    setOrder((o) => (o.includes(entry.id) ? o : [...o, entry.id]));
  }, []);

  // A submit call resolves after the job has started, so its events may already have arrived (a fast
  // job can even be finished); never overwrite newer state with the initial "running" snapshot.
  const track = useCallback((entry: HistoryEntry) => {
    setJobs((j) => (j[entry.id] ? j : { ...j, [entry.id]: { entry, lines: [] } }));
    setOrder((o) => (o.includes(entry.id) ? o : [...o, entry.id]));
  }, []);

  useEffect(() => {
    api.catalog().then(setCatalog).catch(() => undefined);
    api.history(200).then(setHistory).catch(() => undefined);
    // Re-attach to jobs that were already running (e.g. after a webview reload).
    api.activeJobs()
      .then((active) => active.forEach((a) => upsert(a.entry, a.lines)))
      .catch(() => undefined);

    const unlisteners = [
      listen<HistoryEntry>("engine:job", ({ payload }) => {
        upsert(payload);
        if (payload.status !== "running") {
          setHistory((h) => [payload, ...h.filter((x) => x.id !== payload.id)].slice(0, 500));
          finished.current.set(payload.id, payload);
          waiters.current.get(payload.id)?.forEach((resolve) => resolve(payload));
          waiters.current.delete(payload.id);
        }
      }),
      listen<{ jobId: string; lines: OutputLine[] }>("engine:output", ({ payload }) => {
        setJobs((j) => {
          const job = j[payload.jobId];
          if (!job) return j;
          return { ...j, [payload.jobId]: { ...job, lines: appendLines(job.lines, payload.lines) } };
        });
      }),
    ];
    return () => {
      unlisteners.forEach((p) => p.then((un) => un()));
    };
  }, [upsert]);

  const runOperation = useCallback(
    async (id: string, params?: Record<string, string>) => {
      const entry = await api.runOperation(id, params);
      track(entry);
      return entry;
    },
    [track],
  );

  const runCommand = useCallback(
    async (shell: Shell, command: string, confirmed: boolean, asAdmin: boolean) => {
      const entry = await api.runCommand(shell, command, confirmed, asAdmin);
      track(entry);
      return entry;
    },
    [track],
  );

  const openFromHistory = useCallback(
    async (entry: HistoryEntry) => {
      if (jobs[entry.id]) return;
      const lines = await api.jobOutput(entry.id);
      upsert(entry, lines, true);
    },
    [jobs, upsert],
  );

  const closeJob = useCallback((id: string) => {
    setOrder((o) => o.filter((x) => x !== id));
    setJobs((j) => {
      if (j[id]?.entry.status === "running") return j;
      const { [id]: _removed, ...rest } = j;
      return rest;
    });
  }, []);

  const clearHistory = useCallback(async () => {
    await api.clearHistory();
    setHistory([]);
  }, []);

  const running = useMemo(
    () => Object.values(jobs).map((j) => j.entry).filter((e) => e.status === "running"),
    [jobs],
  );

  return (
    <EngineContext.Provider
      value={{
        catalog,
        jobs,
        order,
        history,
        running,
        runOperation,
        runCommand,
        cancel: api.cancelJob,
        track,
        waitFor,
        openFromHistory,
        closeJob,
        clearHistory,
      }}
    >
      {children}
    </EngineContext.Provider>
  );
}

export function useEngine() {
  const ctx = useContext(EngineContext);
  if (!ctx) throw new Error("useEngine must be used inside EngineProvider");
  return ctx;
}
