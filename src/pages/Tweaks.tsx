import { Gamepad2, Gauge, LayoutList, Monitor, Network, RefreshCw, RotateCw, Shield, Timer, Wrench, XCircle, type LucideIcon } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { CleanupCard } from "../components/tweaks/CleanupCard";
import { PageFileCard } from "../components/tweaks/PageFileCard";
import { PowerCard } from "../components/tweaks/PowerCard";
import { ServicesCard } from "../components/tweaks/ServicesCard";
import { TweakPanel } from "../components/tweaks/TweakPanel";
import { Skeleton } from "../components/ui/Card";
import { SubTabs } from "../components/ui/SubTabs";
import { useEngine } from "../context/EngineContext";
import { useToast } from "../context/ToastContext";
import { api, errorMessage, type SystemInfo, type TweakCategory, type TweakInfo } from "../lib/api";

type Tab = TweakCategory | "latency" | "all";

const TABS: { id: Tab; label: string; icon: LucideIcon }[] = [
  { id: "performance", label: "Performance", icon: Gauge },
  { id: "latency", label: "Latency", icon: Timer },
  { id: "gaming", label: "Gaming", icon: Gamepad2 },
  { id: "network", label: "Network", icon: Network },
  { id: "privacy", label: "Privacy", icon: Shield },
  { id: "windows", label: "Windows", icon: Monitor },
  { id: "system", label: "System", icon: Wrench },
  { id: "all", label: "All tweaks", icon: LayoutList },
];

const inTabFilter = (tab: Tab) => (t: TweakInfo) => (tab === "all" ? true : tab === "latency" ? t.latency : t.category === tab);
let lastTab: Tab = "performance";

export function Tweaks() {
  const { runOperation, waitFor } = useEngine();
  const toast = useToast();
  const [tab, setTabState] = useState<Tab>(lastTab);
  const setTab = (t: Tab) => {
    lastTab = t;
    setTabState(t);
  };
  const [tweaks, setTweaks] = useState<TweakInfo[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [needsExplorer, setNeedsExplorer] = useState(false);
  const [restarting, setRestarting] = useState(false);
  const [sys, setSys] = useState<SystemInfo | null>(null);
  const [sysError, setSysError] = useState<string | null>(null);

  const reload = useCallback(async () => {
    try {
      setTweaks(await api.tweaks());
      setError(null);
    } catch (e) {
      setError(errorMessage(e));
    }
  }, []);

  const reloadSys = useCallback(async () => {
    try {
      setSys(await api.systemInfo());
      setSysError(null);
    } catch (e) {
      setSysError(errorMessage(e));
    }
  }, []);

  useEffect(() => {
    reload();
  }, [reload]);

  useEffect(() => {
    if (tab === "system" && !sys) reloadSys();
  }, [tab, sys, reloadSys]);

  const inTab = useMemo(() => tweaks?.filter(inTabFilter(tab)) ?? null, [tweaks, tab]);

  const restartExplorer = async () => {
    setRestarting(true);
    try {
      const e = await runOperation("maint.restart_explorer");
      const done = await waitFor(e.id);
      if (done.status === "succeeded") {
        setNeedsExplorer(false);
        toast("success", "File Explorer restarted");
      } else toast("error", "Could not restart File Explorer", done.error ?? undefined);
    } catch (e) {
      toast("error", "Could not restart File Explorer", errorMessage(e));
    } finally {
      setRestarting(false);
    }
  };

  return (
    <div className="space-y-5">
      <SubTabs<Tab> value={tab} onChange={setTab} tabs={TABS.map((t) => ({ ...t, count: tweaks?.filter(inTabFilter(t.id)).length }))} />

      {needsExplorer && (
        <div className="flex items-center gap-3 rounded-xl border border-accent/30 bg-accent/10 px-4 py-3 text-[13px]">
          <RotateCw size={16} className="text-accent" />
          <span className="flex-1">Some changes show up after File Explorer restarts (the taskbar disappears for a few seconds).</span>
          <button className="btn-primary px-3 py-1.5 text-xs" disabled={restarting} onClick={restartExplorer}>
            {restarting && <RefreshCw size={13} className="animate-spin" />} Restart Explorer
          </button>
        </div>
      )}
      {error && (
        <div className="flex items-center gap-2 rounded-xl border border-bad/30 bg-bad/10 px-4 py-3 text-sm text-bad">
          <XCircle size={15} /> {error}
        </div>
      )}

      {tab === "system" && (
        <>
          {sysError && <div className="rounded-xl border border-bad/30 bg-bad/10 px-4 py-3 text-sm text-bad">Could not read power settings: {sysError}</div>}
          <div className="grid grid-cols-1 gap-5 xl:grid-cols-2">
            {sys ? <PowerCard info={sys} reload={reloadSys} /> : <Skeleton className="h-96" />}
            {sys ? <PageFileCard info={sys} reload={reloadSys} /> : <Skeleton className="h-96" />}
          </div>
          <CleanupCard />
          <ServicesCard />
        </>
      )}

      <TweakPanel key={tab} tweaks={inTab} reload={reload} onExplorerChange={() => setNeedsExplorer(true)} />
    </div>
  );
}
