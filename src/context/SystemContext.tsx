import { createContext, useCallback, useContext, useEffect, useRef, useState, type ReactNode } from "react";
import { api, errorMessage, type AppInfo, type DisplayInfo, type HardwareInfo, type LiveStats } from "../lib/api";
import { useSettings } from "./SettingsContext";

export interface HistoryPoint {
  t: number;
  cpu: number;
  gpu: number | null;
  ram: number;
  rx: number;
  tx: number;
}

const HISTORY_LENGTH = 60;

interface SystemState {
  appInfo: AppInfo | null;
  live: LiveStats | null;
  history: HistoryPoint[];
  liveError: string | null;
  hardware: HardwareInfo | null;
  hardwareLoading: boolean;
  hardwareError: string | null;
  displays: DisplayInfo[] | null;
  refreshHardware: () => Promise<void>;
}

const SystemContext = createContext<SystemState | null>(null);

export function SystemProvider({ children }: { children: ReactNode }) {
  const { settings } = useSettings();
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  const [live, setLive] = useState<LiveStats | null>(null);
  const [history, setHistory] = useState<HistoryPoint[]>([]);
  const [liveError, setLiveError] = useState<string | null>(null);
  const [hardware, setHardware] = useState<HardwareInfo | null>(null);
  const [hardwareLoading, setHardwareLoading] = useState(false);
  const [hardwareError, setHardwareError] = useState<string | null>(null);
  const [displays, setDisplays] = useState<DisplayInfo[] | null>(null);
  const intervalRef = useRef(settings.refreshMs);
  intervalRef.current = settings.refreshMs;

  // Live polling: schedule the next poll only after the previous one finished (no overlap),
  // and skip polling entirely while the window is hidden/minimised.
  useEffect(() => {
    let cancelled = false;
    let timer: number | undefined;

    const tick = async () => {
      if (cancelled) return;
      if (!document.hidden) {
        try {
          const s = await api.liveStats();
          if (cancelled) return;
          setLive(s);
          setLiveError(null);
          setHistory((h) => {
            const next = h.concat({
              t: Date.now(),
              cpu: s.cpuUsage,
              gpu: s.gpuUsage,
              ram: s.memTotal ? (s.memUsed / s.memTotal) * 100 : 0,
              rx: s.netRxPerSec,
              tx: s.netTxPerSec,
            });
            return next.length > HISTORY_LENGTH ? next.slice(next.length - HISTORY_LENGTH) : next;
          });
        } catch (e) {
          if (!cancelled) setLiveError(errorMessage(e));
        }
      }
      timer = window.setTimeout(tick, intervalRef.current);
    };

    const onVisible = () => {
      if (!document.hidden) {
        window.clearTimeout(timer);
        tick();
      }
    };
    document.addEventListener("visibilitychange", onVisible);
    tick();
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
      document.removeEventListener("visibilitychange", onVisible);
    };
  }, []);

  const loadHardware = useCallback(async (refresh: boolean) => {
    setHardwareLoading(true);
    setHardwareError(null);
    api.displays().then(setDisplays).catch(() => undefined);
    try {
      setHardware(await api.hardwareInfo(refresh));
    } catch (e) {
      setHardwareError(errorMessage(e));
    } finally {
      setHardwareLoading(false);
    }
  }, []);

  useEffect(() => {
    api.appInfo().then(setAppInfo).catch(() => undefined);
    loadHardware(false);
  }, [loadHardware]);

  const refreshHardware = useCallback(() => loadHardware(true), [loadHardware]);

  return (
    <SystemContext.Provider
      value={{ appInfo, live, history, liveError, hardware, hardwareLoading, hardwareError, displays, refreshHardware }}
    >
      {children}
    </SystemContext.Provider>
  );
}

export function useSystem() {
  const ctx = useContext(SystemContext);
  if (!ctx) throw new Error("useSystem must be used inside SystemProvider");
  return ctx;
}
