import { Lock, RefreshCw, RotateCcw, ShieldCheck } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useEngine } from "../../context/EngineContext";
import { useSystem } from "../../context/SystemContext";
import { useToast } from "../../context/ToastContext";
import { api, errorMessage, type WindowsFeature } from "../../lib/api";
import { Badge, Skeleton } from "../ui/Card";
import { Chip, SearchInput, Switch } from "../ui/Controls";
import { Modal } from "../ui/Modal";

const ADVICE = {
  keepOn: { label: "Recommended on", tone: "good" },
  keepOff: { label: "Recommended off", tone: "warn" },
  either: { label: "Your choice", tone: "neutral" },
  protected: { label: "Required", tone: "accent" },
} as const;

export function FeaturesPanel() {
  const { waitFor, track } = useEngine();
  const { appInfo } = useSystem();
  const toast = useToast();
  const [features, setFeatures] = useState<WindowsFeature[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [showAll, setShowAll] = useState(false);
  const [query, setQuery] = useState("");
  const [pending, setPending] = useState<{ f: WindowsFeature; enable: boolean } | null>(null);
  const [running, setRunning] = useState<string | null>(null);
  const [restartNeeded, setRestartNeeded] = useState(false);

  const load = useCallback(async () => {
    setError(null);
    try {
      setFeatures(await api.features());
    } catch (e) {
      setError(errorMessage(e));
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const rows = useMemo(() => {
    const q = query.trim().toLowerCase();
    return (features ?? []).filter((f) => (showAll || f.common) && (!q || f.title.toLowerCase().includes(q) || f.name.toLowerCase().includes(q)));
  }, [features, showAll, query]);

  const apply = async () => {
    if (!pending) return;
    const { f, enable } = pending;
    setPending(null);
    setRunning(f.name);
    try {
      const entry = await api.toggleFeature(f.name, enable);
      track(entry);
      const done = await waitFor(entry.id);
      if (done.status === "succeeded") {
        toast("success", `${f.title} turned ${enable ? "on" : "off"}`, "Restart Windows if the change does not take effect.");
        setRestartNeeded(true);
      } else {
        toast("error", `Could not change ${f.title}`, done.error ?? "See Terminal for details.");
      }
      await load();
    } catch (e) {
      toast("error", `Could not change ${f.title}`, errorMessage(e));
    } finally {
      setRunning(null);
    }
  };

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center gap-2">
        <SearchInput value={query} onChange={setQuery} placeholder="Search features" className="w-64" />
        <Chip active={!showAll} onClick={() => setShowAll(false)} count={features?.filter((f) => f.common).length}>
          Common
        </Chip>
        <Chip active={showAll} onClick={() => setShowAll(true)} count={features?.length}>
          All features
        </Chip>
        <span className="flex-1" />
        <button className="btn-ghost px-3 py-1.5 text-xs" onClick={load}>
          <RefreshCw size={13} /> Refresh
        </button>
      </div>

      {restartNeeded && (
        <div className="flex items-center gap-2.5 rounded-xl border border-warn/30 bg-warn/10 px-4 py-3 text-[13px]">
          <RotateCcw size={15} className="text-warn" /> Restart Windows to finish applying feature changes.
        </div>
      )}
      {error && <div className="rounded-xl border border-bad/30 bg-bad/10 px-4 py-3 text-sm text-bad">Could not read Windows features: {error}</div>}

      <div className="card divide-y divide-line/40">
        {!features && !error
          ? Array.from({ length: 6 }).map((_, i) => (
              <div key={i} className="p-4">
                <Skeleton className="h-4 w-1/3" />
                <Skeleton className="mt-2 h-3 w-2/3" />
              </div>
            ))
          : rows.map((f) => {
              const locked = f.advice === "protected" && f.enabled;
              return (
                <div key={f.name} className="flex items-center gap-4 px-5 py-3.5">
                  <div className="min-w-0 flex-1">
                    <div className="flex flex-wrap items-center gap-2">
                      <span className="font-medium">{f.title}</span>
                      {f.advice && <Badge tone={ADVICE[f.advice].tone}>{ADVICE[f.advice].label}</Badge>}
                      <span className="font-mono text-[11px] text-muted">{f.name}</span>
                    </div>
                    {f.description && <div className="mt-1 text-xs leading-relaxed text-muted">{f.description}</div>}
                  </div>
                  <span className={`w-8 text-right text-xs ${f.enabled ? "text-good" : "text-muted"}`}>{f.enabled ? "On" : "Off"}</span>
                  {locked ? (
                    <Lock size={16} className="mx-3 text-muted" />
                  ) : running === f.name ? (
                    <RefreshCw size={16} className="mx-3 animate-spin text-accent" />
                  ) : (
                    <Switch label={f.title} checked={f.enabled} disabled={!!running} onChange={(v) => setPending({ f, enable: v })} />
                  )}
                </div>
              );
            })}
        {features && rows.length === 0 && <p className="p-8 text-center text-sm text-muted">No features match.</p>}
      </div>

      <Modal
        open={!!pending}
        title={pending ? `${pending.enable ? "Turn on" : "Turn off"} ${pending.f.title}?` : ""}
        onClose={() => setPending(null)}
        footer={
          <>
            <button className="btn-ghost" onClick={() => setPending(null)}>
              Cancel
            </button>
            <button className="btn-primary" onClick={apply}>
              {pending?.enable ? "Turn on" : "Turn off"}
            </button>
          </>
        }
      >
        {pending && (
          <div className="space-y-3 text-[13px] leading-relaxed">
            <p>{pending.f.description ?? "Windows optional feature."}</p>
            <p className="text-muted">
              Windows applies this with DISM. It can take a few minutes, cannot be cancelled once started, and may need a restart. You can switch
              it back here at any time.
            </p>
            {!appInfo?.elevated && (
              <p className="flex items-center gap-2 text-xs text-muted">
                <ShieldCheck size={14} className="text-accent" /> Windows will ask for administrator permission.
              </p>
            )}
          </div>
        )}
      </Modal>
    </div>
  );
}
