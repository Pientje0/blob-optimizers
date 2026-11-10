import { getCurrentWindow } from "@tauri-apps/api/window";
import { createContext, useCallback, useContext, useEffect, useState, type ReactNode } from "react";
import { api, type AppSettings } from "../lib/api";

export type Settings = AppSettings;

const DEFAULTS: Settings = {
  theme: "dark",
  accent: "violet",
  refreshMs: 1500,
  notifySuccess: true,
  startMinimized: false,
  restorePointDefault: true,
  historyLimit: 500,
  defaultPreset: "gaming",
  language: "en",
  checkUpdates: true,
  updateRepo: "Pientje0/blob-optimizers",
  startWithWindows: false,
};

/** Accent presets as "r g b" pairs for --c-accent / --c-accent2. */
export const ACCENTS: Record<string, { label: string; a: string; b: string }> = {
  violet: { label: "Violet", a: "124 92 255", b: "56 152 255" },
  blue: { label: "Ocean", a: "59 130 246", b: "34 211 238" },
  green: { label: "Mint", a: "16 185 129", b: "45 212 191" },
  orange: { label: "Ember", a: "249 115 22", b: "239 68 68" },
  pink: { label: "Blossom", a: "236 72 153", b: "168 85 247" },
};

function systemDark() {
  return window.matchMedia?.("(prefers-color-scheme: dark)").matches ?? true;
}

function applyAppearance(s: Settings) {
  const dark = s.theme === "dark" || (s.theme === "system" && systemDark());
  const root = document.documentElement;
  root.dataset.theme = dark ? "dark" : "light";
  root.style.colorScheme = dark ? "dark" : "light";
  const accent = ACCENTS[s.accent] ?? ACCENTS.violet;
  root.style.setProperty("--c-accent", accent.a);
  root.style.setProperty("--c-accent2", accent.b);
  // Match the native title bar to the app theme.
  getCurrentWindow()
    .setTheme(s.theme === "system" ? null : dark ? "dark" : "light")
    .catch(() => undefined);
}

interface SettingsState {
  settings: Settings;
  /** Saves to the backend; returns an error message on failure. */
  update: (patch: Partial<Settings>) => Promise<string | null>;
}

const SettingsContext = createContext<SettingsState | null>(null);

export function SettingsProvider({ children }: { children: ReactNode }) {
  const [settings, setSettings] = useState<Settings>(DEFAULTS);

  useEffect(() => {
    api.settingsGet()
      .then((s) => {
        setSettings(s);
        applyAppearance(s);
      })
      .catch(() => applyAppearance(DEFAULTS));
  }, []);

  // Follow Windows' light/dark switch when the theme is "system".
  useEffect(() => {
    if (settings.theme !== "system") return;
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const on = () => applyAppearance(settings);
    mq.addEventListener("change", on);
    return () => mq.removeEventListener("change", on);
  }, [settings]);

  const update = useCallback(
    async (patch: Partial<Settings>) => {
      const next = { ...settings, ...patch };
      try {
        await api.settingsSet(next);
        const saved = await api.settingsGet();
        setSettings(saved);
        applyAppearance(saved);
        return null;
      } catch (e) {
        return typeof e === "string" ? e : String(e);
      }
    },
    [settings],
  );

  return <SettingsContext.Provider value={{ settings, update }}>{children}</SettingsContext.Provider>;
}

export function useSettings() {
  const ctx = useContext(SettingsContext);
  if (!ctx) throw new Error("useSettings must be used inside SettingsProvider");
  return ctx;
}
