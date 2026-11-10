// Derives status summaries and recommendations from measured data only.
// Recommendations describe what was observed; they never promise a specific FPS gain.
import type { DisplayInfo, HardwareInfo, LiveStats } from "./api";
import type { PageId } from "../navigation";

export type Level = "good" | "warn" | "bad";

export interface Recommendation {
  id: string;
  level: Level;
  title: string;
  detail: string;
  /** Section of the app where the user can act on it. */
  target?: PageId;
}

export interface StatusSummary {
  level: Level;
  score: number;
  headline: string;
}

const HIGH_PERF_PLANS = [
  "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c", // High performance
  "e9a42b02-d5df-448d-aa00-03f14749eb61", // Ultimate Performance
];

function levelFromScore(score: number): Level {
  return score >= 80 ? "good" : score >= 55 ? "warn" : "bad";
}

/** Displays running below the highest refresh rate they support at their current resolution. */
function slowDisplays(displays: DisplayInfo[] | null) {
  return (displays ?? []).filter((d) => d.rates.length && d.hz < Math.max(...d.rates));
}

export function recommendations(hw: HardwareInfo | null, live: LiveStats | null, displays: DisplayInfo[] | null = null): Recommendation[] {
  const out: Recommendation[] = [];

  if (live) {
    const ramPct = (live.memUsed / live.memTotal) * 100;
    if (ramPct >= 85) {
      out.push({
        id: "ram-high",
        level: "bad",
        title: `Memory usage is high (${ramPct.toFixed(0)}%)`,
        detail: "Close memory-heavy apps or review background processes before gaming to avoid stutter from paging.",
        target: "processes",
      });
    } else if (ramPct >= 70) {
      out.push({
        id: "ram-mid",
        level: "warn",
        title: `Memory usage is elevated (${ramPct.toFixed(0)}%)`,
        detail: "Several apps are holding memory. Review background processes if games feel less smooth.",
        target: "processes",
      });
    }
    if (live.processCount > 250) {
      out.push({
        id: "procs",
        level: "warn",
        title: `${live.processCount} processes running`,
        detail: "A large number of background processes. Reviewing startup apps usually reduces this after a restart.",
        target: "startup",
      });
    }
    if (live.uptimeSecs > 7 * 86400) {
      out.push({
        id: "uptime",
        level: "warn",
        title: "PC has not restarted in over a week",
        detail: "A restart clears leaked memory and completes pending updates.",
      });
    }
  }

  if (hw) {
    for (const d of hw.disks) {
      if (d.health && d.health !== "Healthy") {
        out.push({
          id: `disk-health-${d.number}`,
          level: "bad",
          title: `${d.model} reports "${d.health}"`,
          detail: "Back up important data. Windows reports this drive as not healthy.",
          target: "hardware",
        });
      }
      for (const v of d.volumes) {
        const freePct = v.sizeBytes ? (v.freeBytes / v.sizeBytes) * 100 : 100;
        if (freePct < 10) {
          out.push({
            id: `disk-free-${v.letter}`,
            level: freePct < 5 ? "bad" : "warn",
            title: `Drive ${v.letter} is almost full (${freePct.toFixed(0)}% free)`,
            detail: "Low free space slows Windows updates and game patching. Uninstall unused apps or clean temporary files.",
            target: "uninstaller",
          });
        }
      }
    }

    const plan = hw.config.powerPlan;
    if (plan && !HIGH_PERF_PLANS.includes(plan.guid.toLowerCase())) {
      out.push({
        id: "power-plan",
        level: "warn",
        title: `Power plan: ${plan.name}`,
        detail: "For gaming sessions a High performance plan keeps CPU clocks responsive. It increases power use and heat.",
        target: "gaming",
      });
    }
    if (!hw.config.gameModeEnabled) {
      out.push({
        id: "game-mode",
        level: "warn",
        title: "Game Mode is turned off",
        detail: "Game Mode lets Windows prioritise the active game and defer some background work.",
        target: "gaming",
      });
    }

    for (const d of slowDisplays(displays)) {
      out.push({
        id: `refresh-${d.device}`,
        level: d.primary ? "warn" : "good",
        title: `${d.primary ? "Main display" : "A secondary display"} runs at ${d.hz} Hz but supports ${Math.max(...d.rates)} Hz`,
        detail: "A higher refresh rate makes motion visibly smoother. You can change it on the Gaming page.",
        target: "gaming",
      });
    }

    const mods = hw.memory.modules;
    if (mods.length === 1) {
      out.push({
        id: "single-channel",
        level: "warn",
        title: "Single memory module detected",
        detail: "One stick of RAM runs in single-channel mode, which can limit performance on CPU-bound games.",
        target: "hardware",
      });
    }
  }

  const order: Record<Level, number> = { bad: 0, warn: 1, good: 2 };
  return out.sort((a, b) => order[a.level] - order[b.level]);
}

export function pcStatus(hw: HardwareInfo | null, live: LiveStats | null): StatusSummary {
  let score = 100;
  if (live) {
    const ramPct = (live.memUsed / live.memTotal) * 100;
    if (ramPct > 70) score -= Math.min(30, (ramPct - 70) * 1.5);
    if (live.cpuUsage > 80) score -= 10;
  }
  if (hw) {
    for (const d of hw.disks) {
      if (d.health && d.health !== "Healthy") score -= 35;
      for (const v of d.volumes) {
        const freePct = v.sizeBytes ? (v.freeBytes / v.sizeBytes) * 100 : 100;
        if (freePct < 10) score -= freePct < 5 ? 20 : 10;
      }
    }
  }
  score = Math.max(0, Math.round(score));
  const level = levelFromScore(score);
  return {
    level,
    score,
    headline: level === "good" ? "System healthy" : level === "warn" ? "Needs attention" : "Action recommended",
  };
}

export function gamingStatus(hw: HardwareInfo | null, displays: DisplayInfo[] | null = null): StatusSummary | null {
  if (!hw) return null;
  let score = 100;
  const plan = hw.config.powerPlan;
  if (plan && !HIGH_PERF_PLANS.includes(plan.guid.toLowerCase())) score -= 15;
  if (!hw.config.gameModeEnabled) score -= 15;
  if (slowDisplays(displays).some((d) => d.primary)) score -= 20;
  if (hw.memory.modules.length === 1) score -= 15;
  const level = levelFromScore(score);
  return {
    level,
    score,
    headline: level === "good" ? "Ready for gaming" : level === "warn" ? "Room for improvement" : "Not optimized",
  };
}
