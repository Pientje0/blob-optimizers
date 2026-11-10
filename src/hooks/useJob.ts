import { useCallback, useState } from "react";
import { useEngine } from "../context/EngineContext";
import { useSettings } from "../context/SettingsContext";
import { useToast } from "../context/ToastContext";
import { errorMessage, type HistoryEntry } from "../lib/api";

/**
 * Runs an engine job end to end: start, show it in Terminal, wait, toast the result.
 * `busy` holds the key of the job in progress so the triggering control can show a spinner.
 */
export function useJob() {
  const { track, waitFor } = useEngine();
  const toast = useToast();
  const { settings } = useSettings();
  const notifySuccess = settings.notifySuccess;
  const [busy, setBusy] = useState<string | null>(null);

  const run = useCallback(
    async (key: string, start: () => Promise<HistoryEntry>, messages: { success?: string; failure: string }): Promise<HistoryEntry | null> => {
      setBusy(key);
      try {
        const entry = await start();
        track(entry);
        const done = await waitFor(entry.id);
        if (done.status === "succeeded") {
          if (messages.success && notifySuccess) toast("success", messages.success);
        } else if (done.status === "declined") {
          toast("warn", "Administrator permission was declined", "Nothing was changed.");
        } else {
          toast("error", messages.failure, done.error ?? "See Terminal for details.");
        }
        return done;
      } catch (e) {
        toast("error", messages.failure, errorMessage(e));
        return null;
      } finally {
        setBusy(null);
      }
    },
    [track, waitFor, toast, notifySuccess],
  );

  return { run, busy };
}
