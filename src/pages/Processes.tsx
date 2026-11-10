import { AlertTriangle, ArrowDown, Lock, Pause, Play, ShieldCheck, XCircle } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { Skeleton } from "../components/ui/Card";
import { Checkbox, Chip, SearchInput } from "../components/ui/Controls";
import { Modal } from "../components/ui/Modal";
import { useSettings } from "../context/SettingsContext";
import { useSystem } from "../context/SystemContext";
import { useJob } from "../hooks/useJob";
import { api, errorMessage, type ProcessInfo } from "../lib/api";
import { formatBytes } from "../lib/format";

type Sort = "cpu" | "memory" | "name";

export function Processes() {
  const { settings } = useSettings();
  const { appInfo } = useSystem();
  const { run, busy } = useJob();
  const [procs, setProcs] = useState<ProcessInfo[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState<Sort>("cpu");
  const [paused, setPaused] = useState(false);
  const [hideProtected, setHideProtected] = useState(false);
  const [target, setTarget] = useState<ProcessInfo | null>(null);
  const [tree, setTree] = useState(false);
  const [asAdmin, setAsAdmin] = useState(false);
  const pausedRef = useRef(paused);
  pausedRef.current = paused;

  // Poll only while this page is open and visible; never overlap requests.
  useEffect(() => {
    let cancelled = false;
    let timer: number | undefined;
    const tick = async () => {
      if (!document.hidden && !pausedRef.current) {
        try {
          const list = await api.processes();
          if (!cancelled) {
            setProcs(list);
            setError(null);
          }
        } catch (e) {
          if (!cancelled) setError(errorMessage(e));
        }
      }
      if (!cancelled) timer = window.setTimeout(tick, Math.max(settings.refreshMs, 1500));
    };
    tick();
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [settings.refreshMs]);

  const rows = useMemo(() => {
    const q = query.trim().toLowerCase();
    const list = (procs ?? []).filter(
      (p) =>
        (!hideProtected || p.protection !== "protected") &&
        (!q || p.name.toLowerCase().includes(q) || (p.publisher ?? "").toLowerCase().includes(q) || String(p.pid) === q),
    );
    const cmp: Record<Sort, (a: ProcessInfo, b: ProcessInfo) => number> = {
      cpu: (a, b) => b.cpu - a.cpu || b.memory - a.memory,
      memory: (a, b) => b.memory - a.memory,
      name: (a, b) => a.name.localeCompare(b.name),
    };
    return list.sort(cmp[sort]);
  }, [procs, query, sort, hideProtected]);

  const totals = useMemo(() => ({ cpu: (procs ?? []).reduce((s, p) => s + p.cpu, 0), mem: (procs ?? []).reduce((s, p) => s + p.memory, 0) }), [procs]);

  const end = async () => {
    if (!target) return;
    const p = target;
    setTarget(null);
    await run(`end-${p.pid}`, () => api.endProcess(p, tree, asAdmin && !appInfo?.elevated), {
      success: `Ended ${p.name}`,
      failure: `Could not end ${p.name}`,
    });
    setTree(false);
    setAsAdmin(false);
  };

  const Th = ({ id, label, right }: { id: Sort; label: string; right?: boolean }) => (
    <th className={`px-3 py-2 font-semibold ${right ? "text-right" : ""}`}>
      <button className={`inline-flex items-center gap-1 uppercase ${sort === id ? "text-fg" : ""}`} onClick={() => setSort(id)}>
        {label}
        {sort === id && <ArrowDown size={11} />}
      </button>
    </th>
  );

  return (
    <div className="space-y-5">
      <div className="flex flex-wrap items-center gap-2">
        <SearchInput value={query} onChange={setQuery} placeholder="Search name, publisher or PID" className="w-72" />
        <Chip active={hideProtected} onClick={() => setHideProtected(!hideProtected)}>
          Hide Windows core processes
        </Chip>
        <span className="flex-1" />
        <span className="text-xs tabular-nums text-muted">
          {procs?.length ?? "…"} processes · CPU {totals.cpu.toFixed(0)}% · {formatBytes(totals.mem)}
        </span>
        <button className="btn-ghost px-3 py-1.5 text-xs" onClick={() => setPaused(!paused)}>
          {paused ? <Play size={13} /> : <Pause size={13} />} {paused ? "Resume" : "Pause"} updates
        </button>
      </div>

      {error && (
        <div className="flex items-center gap-2 rounded-xl border border-bad/30 bg-bad/10 px-4 py-3 text-sm text-bad">
          <XCircle size={15} /> {error}
        </div>
      )}

      <div className="card overflow-hidden">
        {!procs ? (
          <div className="space-y-3 p-5">
            {Array.from({ length: 10 }).map((_, i) => (
              <Skeleton key={i} className="h-7 w-full" />
            ))}
          </div>
        ) : (
          <table className="w-full table-fixed text-[13px]">
            <thead className="bg-surface">
              <tr className="text-left text-[11px] uppercase tracking-wider text-muted">
                <Th id="name" label="Process" />
                <th className="w-[220px] px-3 py-2 font-semibold">Publisher</th>
                <th className="w-[80px] px-3 py-2 text-right font-semibold">PID</th>
                <Th id="cpu" label="CPU" right />
                <Th id="memory" label="Memory" right />
                <th className="w-[100px] px-3 py-2" />
              </tr>
            </thead>
            <tbody>
              {rows.map((p) => (
                <tr key={p.pid} className="border-t border-line/40 hover:bg-raised/40">
                  <td className="px-3 py-2">
                    <div className="flex items-center gap-2">
                      {p.protection === "protected" && <Lock size={12} className="shrink-0 text-muted" />}
                      {p.protection === "caution" && <AlertTriangle size={12} className="shrink-0 text-warn" />}
                      <span className="truncate font-medium" title={p.path ?? p.name}>
                        {p.name}
                      </span>
                    </div>
                  </td>
                  <td className="truncate px-3 py-2 text-muted" title={p.publisher ?? ""}>
                    {p.publisher ?? "—"}
                  </td>
                  <td className="px-3 py-2 text-right tabular-nums text-muted">{p.pid}</td>
                  <td className={`w-[90px] px-3 py-2 text-right tabular-nums ${p.cpu >= 20 ? "text-warn" : ""}`}>{p.cpu.toFixed(1)}%</td>
                  <td className="w-[110px] px-3 py-2 text-right tabular-nums">{formatBytes(p.memory)}</td>
                  <td className="px-3 py-1.5 text-right">
                    {p.protection !== "protected" && (
                      <button className="btn-ghost px-2.5 py-1 text-xs" disabled={!!busy} onClick={() => setTarget(p)}>
                        End
                      </button>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
      <p className="text-xs text-muted">
        CPU is shown as a share of the whole processor (like Task Manager). Windows core and security processes are locked to prevent crashes. Updates pause
        while this page is hidden.
      </p>

      <Modal
        open={!!target}
        title={`End ${target?.name}?`}
        onClose={() => setTarget(null)}
        footer={
          <>
            <button className="btn-ghost" onClick={() => setTarget(null)}>
              Cancel
            </button>
            <button className="btn-primary bg-bad" onClick={end}>
              End process
            </button>
          </>
        }
      >
        {target && (
          <div className="space-y-3 text-[13px] leading-relaxed">
            <p className="text-muted">
              PID {target.pid}
              {target.path ? ` · ${target.path}` : ""}
            </p>
            {target.note && (
              <p className="flex gap-2 rounded-xl border border-warn/30 bg-warn/10 p-3">
                <AlertTriangle size={16} className="mt-0.5 shrink-0 text-warn" /> {target.note}
              </p>
            )}
            <p>Unsaved work in this program will be lost.</p>
            <label className="flex cursor-pointer items-center gap-2.5 text-[13px]">
              <Checkbox checked={tree} onChange={setTree} label="End child processes too" /> Also end processes it started
            </label>
            {!appInfo?.elevated && (
              <label className="flex cursor-pointer items-center gap-2.5 text-[13px]">
                <Checkbox checked={asAdmin} onChange={setAsAdmin} label="Run as administrator" />
                <ShieldCheck size={14} className="text-accent" /> Use administrator rights (needed for processes running as admin or SYSTEM)
              </label>
            )}
          </div>
        )}
      </Modal>
    </div>
  );
}
