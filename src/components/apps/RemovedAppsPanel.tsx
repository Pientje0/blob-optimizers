import { Loader2, Package, RotateCcw } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useEngine } from "../../context/EngineContext";
import { useToast } from "../../context/ToastContext";
import { api, errorMessage, type RemovedApp } from "../../lib/api";
import { formatTime } from "../../lib/format";

export function RemovedAppsPanel({ onChanged }: { onChanged: () => void }) {
  const { track, waitFor } = useEngine();
  const toast = useToast();
  const [items, setItems] = useState<RemovedApp[] | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  const load = useCallback(() => {
    api.removedApps().then(setItems).catch(() => setItems([]));
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const reinstall = async (r: RemovedApp) => {
    setBusy(r.packageName);
    try {
      const entry = await api.reinstallApp(r.packageName);
      track(entry);
      const done = await waitFor(entry.id);
      if (done.status === "succeeded") {
        toast("success", `${r.name} is back`);
        onChanged();
      } else {
        toast("error", `Could not reinstall ${r.name}`, done.error ?? "See Terminal for details.");
      }
    } catch (e) {
      toast("error", `Could not reinstall ${r.name}`, errorMessage(e));
    } finally {
      setBusy(null);
      load();
    }
  };

  if (!items) return null;
  if (!items.length) {
    return (
      <div className="card flex flex-col items-center justify-center px-6 py-16 text-center">
        <Package size={28} className="text-muted" />
        <p className="mt-3 font-medium">No removed apps</p>
        <p className="mt-1 max-w-md text-sm text-muted">Store apps you remove with Blob Optimizers are listed here so you can restore them later.</p>
      </div>
    );
  }

  return (
    <div className="card divide-y divide-line/40">
      {items.map((r) => (
        <div key={r.packageName} className="flex items-center gap-4 px-5 py-3.5">
          <Package size={16} className="shrink-0 text-accent2" />
          <div className="min-w-0 flex-1">
            <div className="font-medium">{r.name}</div>
            <div className="text-xs text-muted">
              Removed {formatTime(r.removedAt)} · <span className="font-mono">{r.packageName}</span>
            </div>
          </div>
          <button className="btn-ghost px-3 py-1.5 text-xs" disabled={!!busy} onClick={() => reinstall(r)}>
            {busy === r.packageName ? <Loader2 size={13} className="animate-spin" /> : <RotateCcw size={13} />}
            Reinstall
          </button>
        </div>
      ))}
      <p className="px-5 py-3 text-xs text-muted">
        Reinstalling first restores the copy Windows keeps for preinstalled apps. If there is none, the app is downloaded from the Microsoft Store
        {" "}with winget, which accepts the Store's terms on your behalf.
      </p>
    </div>
  );
}
