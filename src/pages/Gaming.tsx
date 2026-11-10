import { ChevronRight, Layers } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { DisplaysCard } from "../components/gaming/DisplaysCard";
import { FpsMonitor } from "../components/gaming/FpsMonitor";
import { GameProfiles } from "../components/gaming/GameProfiles";
import { GpuCard } from "../components/gaming/GpuCard";
import { NetworkCard } from "../components/gaming/NetworkCard";
import { PresetsSection } from "../components/gaming/PresetsSection";
import { TweakPanel } from "../components/tweaks/TweakPanel";
import { Card } from "../components/ui/Card";
import { useSystem } from "../context/SystemContext";
import { api, errorMessage, type DisplayInfo, type LastPreset, type PresetInfo, type TweakInfo } from "../lib/api";
import { formatBytes } from "../lib/format";
import type { PageId } from "../navigation";

const SYSTEM_PROCESSES = new Set([
  "svchost.exe", "system", "registry", "memory compression", "dwm.exe", "msmpeng.exe", "msedgewebview2.exe", "blob-optimizers.exe",
  "explorer.exe", "csrss.exe", "lsass.exe", "searchhost.exe", "startmenuexperiencehost.exe", "mpdefendercoreservice.exe",
]);

export function Gaming({ onNavigate }: { onNavigate: (id: PageId) => void }) {
  const { live } = useSystem();
  const [presets, setPresets] = useState<PresetInfo[] | null>(null);
  const [last, setLast] = useState<LastPreset | null>(null);
  const [tweaks, setTweaks] = useState<TweakInfo[] | null>(null);
  const [displays, setDisplays] = useState<DisplayInfo[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const loadTweaks = useCallback(async () => {
    try {
      setTweaks((await api.tweaks()).filter((t) => t.category === "gaming"));
    } catch (e) {
      setError(errorMessage(e));
    }
  }, []);

  const loadPresets = useCallback(async () => {
    try {
      const [p, l] = await api.presets();
      setPresets(p);
      setLast(l);
    } catch (e) {
      setError(errorMessage(e));
    }
  }, []);

  const loadDisplays = useCallback(async () => {
    try {
      setDisplays(await api.displays());
    } catch (e) {
      setError(errorMessage(e));
    }
  }, []);

  useEffect(() => {
    loadPresets();
    loadTweaks();
    loadDisplays();
  }, [loadPresets, loadTweaks, loadDisplays]);

  // Presets and individual tweaks change the same settings, so refresh both together.
  const reloadAll = useCallback(async () => {
    await Promise.all([loadPresets(), loadTweaks()]);
  }, [loadPresets, loadTweaks]);

  // Windows itself and Blob Optimizers' own WebView aren't apps the user can close before playing.
  const heavy = useMemo(
    () =>
      (live?.topProcesses ?? [])
        .filter((p) => !SYSTEM_PROCESSES.has(p.name.toLowerCase()))
        .filter((p) => p.cpu > 3 || p.memory > 800 * 1024 * 1024)
        .slice(0, 4),
    [live],
  );

  return (
    <div className="space-y-6">
      {error && <div className="rounded-xl border border-bad/30 bg-bad/10 px-4 py-3 text-sm text-bad">{error}</div>}

      <PresetsSection presets={presets} last={last} reload={reloadAll} />

      <FpsMonitor />

      <div className="grid grid-cols-1 gap-5 xl:grid-cols-2">
        <DisplaysCard displays={displays} reload={loadDisplays} />
        <GpuCard />
      </div>

      <section className="space-y-3">
        <h2 className="text-[15px] font-semibold">Windows gaming features</h2>
        <TweakPanel tweaks={tweaks} reload={reloadAll} onExplorerChange={() => undefined} />
      </section>

      <GameProfiles />

      <div className="grid grid-cols-1 gap-5 xl:grid-cols-2">
        <NetworkCard />
        <Card title="Background apps" icon={Layers} className="h-full">
          <p className="text-xs text-muted">Apps using noticeable CPU or memory right now. Close the ones you don't need before playing.</p>
          <ul className="mt-3 space-y-1.5">
            {heavy.length === 0 ? (
              <li className="text-[13px] text-muted">Nothing heavy is running in the background.</li>
            ) : (
              heavy.map((p) => (
                <li key={p.name} className="flex items-center gap-3 rounded-lg bg-raised/60 px-3 py-2 text-[13px]">
                  <span className="flex-1 truncate">
                    {p.name.replace(/\.exe$/i, "")}
                    {p.instances > 1 && <span className="ml-1 text-xs text-muted">×{p.instances}</span>}
                  </span>
                  <span className="text-xs tabular-nums text-muted">
                    {p.cpu.toFixed(1)}% · {formatBytes(p.memory)}
                  </span>
                </li>
              ))
            )}
          </ul>
          <div className="mt-4 flex flex-wrap gap-2">
            <button className="btn-ghost px-3 py-1.5 text-xs" onClick={() => onNavigate("processes")}>
              Processes <ChevronRight size={13} />
            </button>
            <button className="btn-ghost px-3 py-1.5 text-xs" onClick={() => onNavigate("startup")}>
              Startup apps <ChevronRight size={13} />
            </button>
          </div>
        </Card>
      </div>
    </div>
  );
}
