import { Loader2, RefreshCw, Trash2 } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useJob } from "../../hooks/useJob";
import { api, errorMessage, type CleanupItem } from "../../lib/api";
import { formatBytes } from "../../lib/format";
import { Badge, Card, Skeleton } from "../ui/Card";
import { Checkbox } from "../ui/Controls";
import { Modal } from "../ui/Modal";

const META: Record<string, { label: string; note: string; admin?: boolean; defaultOn: boolean; permanent?: boolean }> = {
  userTemp: { label: "Your temporary files", note: "Files in your Temp folder older than 24 hours. Files in use are skipped.", defaultOn: true },
  winTemp: { label: "Windows temporary files", note: "Files in Windows\\Temp older than 24 hours.", admin: true, defaultOn: true },
  thumbnails: { label: "Thumbnail cache", note: "Picture previews in File Explorer; rebuilt automatically.", defaultOn: false },
  shaders: { label: "GPU shader caches", note: "DirectX, AMD and NVIDIA shader caches. Games may stutter briefly on first launch while they rebuild.", defaultOn: false },
  errorReports: { label: "Error reports and crash dumps", note: "Windows Error Reporting archives and app crash dumps.", admin: true, defaultOn: true },
  updateLogs: { label: "Windows servicing logs", note: "CBS log files older than 24 hours.", admin: true, defaultOn: false },
  recycleBin: { label: "Recycle Bin", note: "Permanently deletes everything in the Recycle Bin.", defaultOn: false, permanent: true },
};

export function CleanupCard() {
  const { run, busy } = useJob();
  const [items, setItems] = useState<CleanupItem[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [scanning, setScanning] = useState(false);
  const [picked, setPicked] = useState<Set<string>>(new Set(Object.entries(META).filter(([, m]) => m.defaultOn).map(([k]) => k)));
  const [confirm, setConfirm] = useState(false);

  const scan = useCallback(async () => {
    setScanning(true);
    setError(null);
    try {
      setItems(await api.cleanupScan());
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setScanning(false);
    }
  }, []);

  useEffect(() => {
    scan();
  }, [scan]);

  const total = (items ?? []).filter((i) => picked.has(i.id)).reduce((s, i) => s + i.bytes, 0);
  const toggle = (id: string, on: boolean) =>
    setPicked((p) => {
      const n = new Set(p);
      if (on) n.add(id);
      else n.delete(id);
      return n;
    });

  const clean = async () => {
    setConfirm(false);
    await run("cleanup", () => api.cleanupRun([...picked]), { success: "Cleanup finished", failure: "Cleanup did not complete" });
    scan();
  };

  return (
    <Card
      title="Storage cleanup"
      icon={Trash2}
      className="h-full"
      actions={
        <button className="flex items-center gap-1 text-xs text-muted hover:text-fg" onClick={scan} disabled={scanning}>
          <RefreshCw size={12} className={scanning ? "animate-spin" : ""} /> Rescan
        </button>
      }
    >
      {error && <p className="mb-2 text-xs text-bad">{error}</p>}
      {!items ? (
        <div className="space-y-3">
          {Array.from({ length: 5 }).map((_, i) => (
            <Skeleton key={i} className="h-9 w-full" />
          ))}
        </div>
      ) : (
        <ul className="space-y-1.5">
          {items.map((i) => {
            const m = META[i.id];
            if (!m) return null;
            return (
              <li key={i.id} className="flex cursor-pointer items-start gap-3 rounded-xl bg-raised/60 px-3 py-2.5" onClick={() => toggle(i.id, !picked.has(i.id))}>
                <span className="pt-0.5">
                  <Checkbox checked={picked.has(i.id)} onChange={(v) => toggle(i.id, v)} label={m.label} />
                </span>
                <div className="min-w-0 flex-1">
                  <div className="flex flex-wrap items-center gap-2 text-[13px] font-medium">
                    {m.label}
                    {m.admin && <Badge tone="accent">Admin</Badge>}
                    {m.permanent && <Badge tone="bad">Permanent</Badge>}
                  </div>
                  <div className="text-xs text-muted">{m.note}</div>
                </div>
                <span className="shrink-0 text-right text-xs tabular-nums">
                  {formatBytes(i.bytes)}
                  <span className="block text-muted">{i.files} files</span>
                </span>
              </li>
            );
          })}
        </ul>
      )}
      <div className="mt-3 flex items-center gap-3">
        <span className="flex-1 text-xs text-muted">Selected: about {formatBytes(total)}. Only these cache and temp locations are touched; never your documents.</span>
        <button className="btn-primary px-3 py-1.5 text-xs" disabled={!picked.size || !items || !!busy} onClick={() => setConfirm(true)}>
          {busy ? <Loader2 size={13} className="animate-spin" /> : <Trash2 size={13} />} Clean up
        </button>
      </div>

      <Modal
        open={confirm}
        title="Clean up selected items?"
        onClose={() => setConfirm(false)}
        footer={
          <>
            <button className="btn-ghost" onClick={() => setConfirm(false)}>
              Cancel
            </button>
            <button className="btn-primary bg-bad" onClick={clean}>
              <Trash2 size={14} /> Delete {formatBytes(total)}
            </button>
          </>
        }
      >
        <ul className="list-inside list-disc space-y-1 text-[13px]">
          {[...picked].map((id) => (
            <li key={id}>
              {META[id]?.label}
              {META[id]?.permanent && <span className="text-bad"> (cannot be undone)</span>}
            </li>
          ))}
        </ul>
        <p className="mt-3 text-xs text-muted">These files are deleted directly, not moved to the Recycle Bin. Caches rebuild automatically when needed.</p>
      </Modal>
    </Card>
  );
}
