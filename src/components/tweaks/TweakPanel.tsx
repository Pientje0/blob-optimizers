import { AlertTriangle, RefreshCw, Search, Sparkles } from "lucide-react";
import { useMemo, useState } from "react";
import { useEngine } from "../../context/EngineContext";
import { useSettings } from "../../context/SettingsContext";
import { useSystem } from "../../context/SystemContext";
import { useToast } from "../../context/ToastContext";
import type { TweakInfo, TweakTarget } from "../../lib/api";
import { api } from "../../lib/api";
import { useJob } from "../../hooks/useJob";
import { Skeleton } from "../ui/Card";
import { Checkbox } from "../ui/Controls";
import { Modal } from "../ui/Modal";
import { TweakRow } from "./TweakRow";

interface Props {
  tweaks: TweakInfo[] | null;
  reload: () => Promise<void>;
  /** Called after a change that needs File Explorer to restart. */
  onExplorerChange: () => void;
  /** Lay groups out in two columns on wide windows (default true). */
  columns?: boolean;
  /** Hide the search/filter toolbar (for short lists). */
  bare?: boolean;
}

type Filter = "all" | "applied" | "notApplied" | "recommended";
const FILTERS: { id: Filter; label: string }[] = [
  { id: "all", label: "All" },
  { id: "notApplied", label: "Not applied" },
  { id: "applied", label: "Applied" },
  { id: "recommended", label: "Recommended" },
];

function matches(t: TweakInfo, q: string, f: Filter) {
  if (f === "applied" && t.state !== "on") return false;
  if (f === "notApplied" && t.state === "on") return false;
  if (f === "recommended" && t.recommend !== "apply") return false;
  if (!q) return true;
  const hay = `${t.title} ${t.description} ${t.group}`.toLowerCase();
  return q.toLowerCase().split(/\s+/).every((w) => hay.includes(w));
}

/** Restore point descriptions only allow a safe character set. */
const RP_DESCRIPTION = "Before applying recommended tweaks";

export function TweakPanel({ tweaks, reload, onExplorerChange, columns = true, bare = false }: Props) {
  const { runOperation, waitFor } = useEngine();
  const { appInfo } = useSystem();
  const toast = useToast();
  const { run, busy } = useJob();
  const [confirm, setConfirm] = useState<{ t: TweakInfo; target: TweakTarget } | null>(null);
  const [batch, setBatch] = useState<TweakInfo[] | null>(null);
  const { settings } = useSettings();
  const [restorePoint, setRestorePoint] = useState(settings.restorePointDefault);
  const [batchBusy, setBatchBusy] = useState(false);

  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<Filter>("all");

  const groups = useMemo(() => {
    const m = new Map<string, TweakInfo[]>();
    (tweaks ?? []).filter((t) => matches(t, query.trim(), filter)).forEach((t) => m.set(t.group, [...(m.get(t.group) ?? []), t]));
    return [...m.entries()];
  }, [tweaks, query, filter]);

  const recommended = (tweaks ?? []).filter((t) => t.recommend === "apply" && t.state !== "on");

  const apply = async (t: TweakInfo, target: TweakTarget) => {
    setConfirm(null);
    const verb = target === "on" ? "Applied" : target === "off" ? "Turned off" : target === "default" ? "Restored default for" : "Undid";
    const done = await run(t.id, () => api.applyTweaks([{ id: t.id, target }]), {
      success: `${verb} "${t.title}"`,
      failure: `Could not change "${t.title}"`,
    });
    await reload();
    if (done?.status === "succeeded" && t.after === "explorer") onExplorerChange();
  };

  const request = (t: TweakInfo, target: TweakTarget) => {
    // Tweaks with trade-offs are confirmed first; the rest apply immediately (every change can be undone).
    if (target === "on" && t.caveat) setConfirm({ t, target });
    else apply(t, target);
  };

  const applyBatch = async () => {
    if (!batch) return;
    setBatchBusy(true);
    try {
      if (restorePoint) {
        const rp = await runOperation("restore.create_point", { description: RP_DESCRIPTION });
        const rpDone = await waitFor(rp.id);
        if (rpDone.status !== "succeeded") {
          setRestorePoint(false);
          toast(
            "error",
            "No restore point, so nothing was applied",
            `${rpDone.error ?? "See Terminal for details."} Turn on System Protection (System tab) or untick the restore point option to continue.`,
          );
          return;
        }
      }
      const done = await run("batch", () => api.applyTweaks(batch.map((t) => ({ id: t.id, target: "on" }))), {
        success: `Applied ${batch.length} recommended tweaks`,
        failure: "Some tweaks could not be applied",
      });
      if (done && batch.some((t) => t.after === "explorer")) onExplorerChange();
      setBatch(null);
      await reload();
    } finally {
      setBatchBusy(false);
    }
  };

  if (!tweaks) {
    return (
      <div className="card space-y-4 p-5">
        {Array.from({ length: 5 }).map((_, i) => (
          <Skeleton key={i} className="h-12 w-full" />
        ))}
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center gap-2">
        {!bare && (
          <>
            <div className="relative">
              <Search size={14} className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-muted" />
              <input
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                placeholder="Search tweaks"
                aria-label="Search tweaks"
                className="w-52 rounded-xl border border-line bg-base/60 py-1.5 pl-8 pr-3 text-[13px] outline-none placeholder:text-muted focus:border-accent/60"
              />
            </div>
            <div className="flex rounded-xl border border-line bg-base/60 p-0.5 text-xs">
              {FILTERS.map((f) => (
                <button
                  key={f.id}
                  onClick={() => setFilter(f.id)}
                  className={`rounded-[10px] px-2.5 py-1 font-medium ${filter === f.id ? "bg-raised text-fg" : "text-muted hover:text-fg"}`}
                >
                  {f.label}
                </button>
              ))}
            </div>
          </>
        )}
        <p className="flex-1 text-xs text-muted">
          {tweaks.filter((t) => t.state === "on").length} of {tweaks.length} applied · every change can be reset or undone
        </p>
        <button className="btn-ghost px-3 py-1.5 text-xs" onClick={reload}>
          <RefreshCw size={13} /> Refresh
        </button>
        <button className="btn-primary px-3 py-1.5 text-xs" disabled={!recommended.length || !!busy} onClick={() => setBatch(recommended)}>
          <Sparkles size={14} /> Apply recommended ({recommended.length})
        </button>
      </div>

      {groups.length === 0 && <p className="card px-5 py-8 text-center text-[13px] text-muted">No tweaks match{query ? ` "${query}"` : " this filter"}.</p>}

      <div className={columns ? "gap-4 xl:columns-2 [&>section]:mb-4 [&>section]:break-inside-avoid" : "space-y-4"}>
        {groups.map(([group, items]) => (
          <section key={group} className="card animate-fade-up overflow-hidden">
            <header className="flex items-center justify-between border-b border-line/50 px-4 py-2.5">
              <span className="text-[13px] font-semibold">{group}</span>
              <span className="text-[11px] tabular-nums text-muted">
                {items.filter((t) => t.state === "on").length}/{items.length}
              </span>
            </header>
            <div className="divide-y divide-line/40">
              {items.map((t) => (
                <TweakRow key={t.id} tweak={t} busy={busy === t.id} disabled={!!busy || batchBusy} onChange={request} />
              ))}
            </div>
          </section>
        ))}
      </div>

      <Modal
        open={!!confirm}
        title={`Apply "${confirm?.t.title}"?`}
        onClose={() => setConfirm(null)}
        footer={
          <>
            <button className="btn-ghost" onClick={() => setConfirm(null)}>
              Cancel
            </button>
            <button className="btn-primary" onClick={() => confirm && apply(confirm.t, confirm.target)}>
              Apply
            </button>
          </>
        }
      >
        {confirm && (
          <div className="space-y-3 text-[13px] leading-relaxed">
            <p>{confirm.t.description}</p>
            <p className="flex gap-2 rounded-xl border border-warn/30 bg-warn/10 p-3">
              <AlertTriangle size={16} className="mt-0.5 shrink-0 text-warn" /> {confirm.t.caveat}
            </p>
            <p className="text-xs text-muted">You can reset it to the Windows default or undo it here at any time.</p>
          </div>
        )}
      </Modal>

      <Modal
        open={!!batch}
        title={`Apply ${batch?.length ?? 0} recommended tweaks?`}
        onClose={() => !batchBusy && setBatch(null)}
        width={600}
        footer={
          <>
            <button className="btn-ghost" disabled={batchBusy} onClick={() => setBatch(null)}>
              Cancel
            </button>
            <button className="btn-primary" disabled={batchBusy} onClick={applyBatch}>
              {batchBusy ? <RefreshCw size={15} className="animate-spin" /> : <Sparkles size={15} />} Apply all
            </button>
          </>
        }
      >
        <ul className="max-h-72 space-y-1.5 overflow-y-auto pr-1">
          {batch?.map((t) => (
            <li key={t.id} className="rounded-xl bg-raised/60 px-3 py-2">
              <div className="text-[13px] font-medium">{t.title}</div>
              <div className="text-xs text-muted">{t.description}</div>
              {t.caveat && <div className="mt-1 text-xs text-warn/90">{t.caveat}</div>}
            </li>
          ))}
        </ul>
        <label className="mt-4 flex cursor-pointer items-start gap-3 rounded-xl bg-raised/60 p-3">
          <Checkbox checked={restorePoint} onChange={setRestorePoint} label="Create restore point" />
          <span className="text-[13px]">
            <span className="font-medium">Create a restore point first</span>
            <span className="block text-xs text-muted">
              Recommended for batch changes.{!appInfo?.elevated && " Windows will ask for administrator permission."} If it fails, nothing is
              applied.
            </span>
          </span>
        </label>
        {batch?.some((t) => t.requiresAdmin) && !appInfo?.elevated && (
          <p className="mt-3 text-xs text-muted">Some tweaks change system-wide settings, so Windows will ask for administrator permission.</p>
        )}
      </Modal>
    </div>
  );
}
