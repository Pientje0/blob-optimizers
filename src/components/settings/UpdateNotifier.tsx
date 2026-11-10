import { useEffect, useRef } from "react";
import { useSettings } from "../../context/SettingsContext";
import { useToast } from "../../context/ToastContext";
import { api } from "../../lib/api";

/** Checks for a newer release once per launch (when enabled) and mentions it with a toast. */
export function UpdateNotifier() {
  const { settings } = useSettings();
  const toast = useToast();
  const done = useRef(false);

  useEffect(() => {
    if (done.current || !settings.checkUpdates || !settings.updateRepo) return;
    // Wait until start-up work (hardware detection, first polls) has settled.
    const t = window.setTimeout(() => {
      done.current = true;
      api
        .checkUpdates()
        .then((u) => {
          if (u.available) toast("info", `Blob Optimizers ${u.latest} is available`, "Open Settings › Updates to see what's new and get the installer.");
        })
        .catch(() => undefined); // Offline or not published yet: stay quiet at startup; Settings shows errors.
    }, 8000);
    return () => window.clearTimeout(t);
  }, [settings.checkUpdates, settings.updateRepo, toast]);

  return null;
}
