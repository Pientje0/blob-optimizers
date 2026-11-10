import { Info, Timer } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { api, errorMessage, type TweakInfo } from "../../lib/api";
import { NetworkCard } from "../gaming/NetworkCard";
import { TweakPanel } from "./TweakPanel";

/** Display order of latency groups: input first, network last. */
const ORDER = ["Input", "Scheduling", "Power", "Display & frames", "Windows gaming features", "Background activity", "TCP", "Adapter"];

/**
 * Every latency-related setting in one place (input, CPU scheduling, power, network). The same panel
 * appears on the Gaming and Network pages, so it's easy to find from either.
 */
export function LatencyPanel({ focus }: { focus: "gaming" | "network" }) {
  const [tweaks, setTweaks] = useState<TweakInfo[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(async () => {
    try {
      setTweaks((await api.tweaks()).filter((t) => t.latency));
      setError(null);
    } catch (e) {
      setError(errorMessage(e));
    }
  }, []);

  useEffect(() => {
    reload();
  }, [reload]);

  // Network page: network groups first; Gaming page: input and scheduling first.
  const sorted = useMemo(() => {
    if (!tweaks) return null;
    const rank = (t: TweakInfo) => {
      const i = ORDER.indexOf(t.group);
      const base = i < 0 ? ORDER.length : i;
      return focus === "network" && t.category === "network" ? base - 100 : base;
    };
    return [...tweaks].sort((a, b) => rank(a) - rank(b));
  }, [tweaks, focus]);

  return (
    <div className="space-y-4">
      <div className="card flex gap-3 px-4 py-3 text-[12.5px] leading-relaxed text-muted">
        <Timer size={18} className="mt-0.5 shrink-0 text-accent" />
        <div>
          <span className="font-medium text-fg">Latency settings</span> — input, CPU scheduling, power and network settings that affect how quickly your
          actions reach the game and the server. The same list appears under Gaming › Latency and Network › Latency &amp; tweaks (and Windows Tweaks › Latency). Effects depend on your hardware and game,
          so test one change at a time with the ping test here and the FPS monitor (Gaming › FPS & performance).
          <span className="mt-1 flex items-center gap-1.5 text-[11.5px]">
            <Info size={12} /> Settings that don't exist on this PC (for example adapter options your network card doesn't support) are hidden.
          </span>
        </div>
      </div>

      {error && <div className="rounded-xl border border-bad/30 bg-bad/10 px-4 py-3 text-sm text-bad">{error}</div>}

      <div className="grid grid-cols-1 gap-4 2xl:grid-cols-[1fr_380px]">
        <div className="2xl:order-2">
          <NetworkCard />
        </div>
        <div className="min-w-0 2xl:order-1">
          <TweakPanel tweaks={sorted} reload={reload} onExplorerChange={() => undefined} />
        </div>
      </div>
    </div>
  );
}
