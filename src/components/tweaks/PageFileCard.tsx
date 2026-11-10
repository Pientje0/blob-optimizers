import { HardDrive, Loader2 } from "lucide-react";
import { useEffect, useState } from "react";
import { useJob } from "../../hooks/useJob";
import { api, type SystemInfo } from "../../lib/api";
import { Card, InfoRow } from "../ui/Card";

const NUM = "w-28 rounded-lg border border-line bg-base/60 px-2 py-1.5 text-right text-sm tabular-nums outline-none focus:border-accent/60";

export function PageFileCard({ info, reload }: { info: SystemInfo; reload: () => void }) {
  const { run, busy } = useJob();
  const pf = info.pagefile;
  const file = pf.files[0];
  const [automatic, setAutomatic] = useState(pf.automatic);
  const [initial, setInitial] = useState(0);
  const [maximum, setMaximum] = useState(0);
  const limit = Math.min(pf.ramMb * 4, 262144);

  useEffect(() => {
    setAutomatic(pf.automatic);
    setInitial(file?.initialMb || Math.max(1024, Math.round(pf.ramMb * 0.5)));
    setMaximum(file?.maximumMb || Math.max(4096, pf.ramMb * 2));
    // Re-seed the form only when fresh data arrives.
  }, [info]);

  const error = !automatic && (initial < 1024 ? "Keep at least 1024 MB." : maximum < initial ? "Maximum must be at least the initial size." : maximum > limit ? `Maximum is limited to ${limit} MB.` : null);
  const changed = automatic !== pf.automatic || (!automatic && (initial !== file?.initialMb || maximum !== file?.maximumMb));

  const save = async () => {
    await run("pagefile", () => api.setPagefile(automatic, automatic ? null : initial, automatic ? null : maximum), {
      success: "Page file settings saved. Restart Windows to apply them.",
      failure: "Could not change the page file",
    });
    reload();
  };

  return (
    <Card title="Virtual memory" icon={HardDrive} className="h-full">
      <div className="text-[13px]">
        <InfoRow label="Managed by" value={pf.automatic ? "Windows (automatic)" : "Custom size"} />
        {file && <InfoRow label="Page file" value={`${file.path} · ${file.allocatedMb} MB allocated`} />}
        {file && <InfoRow label="Usage now / peak" value={`${file.currentUsageMb} MB / ${file.peakUsageMb} MB`} />}
        <InfoRow label="Installed RAM" value={`${pf.ramMb} MB`} />
      </div>

      <div className="mt-4 space-y-2 text-[13px]">
        <label className="flex cursor-pointer items-center gap-2.5">
          <input type="radio" checked={automatic} onChange={() => setAutomatic(true)} className="accent-[rgb(var(--c-accent))]" />
          Let Windows manage the size <span className="text-xs text-muted">(recommended)</span>
        </label>
        <label className="flex cursor-pointer items-center gap-2.5">
          <input type="radio" checked={!automatic} onChange={() => setAutomatic(false)} className="accent-[rgb(var(--c-accent))]" />
          Custom size on the system drive
        </label>
        {!automatic && (
          <div className="ml-6 flex flex-wrap items-center gap-3 pt-1">
            <label className="flex items-center gap-2 text-xs text-muted">
              Initial (MB) <input type="number" className={NUM} value={initial} onChange={(e) => setInitial(+e.target.value)} />
            </label>
            <label className="flex items-center gap-2 text-xs text-muted">
              Maximum (MB) <input type="number" className={NUM} value={maximum} onChange={(e) => setMaximum(+e.target.value)} />
            </label>
          </div>
        )}
        {error && <p className="ml-6 text-xs text-bad">{error}</p>}
      </div>
      <p className="mt-3 text-xs leading-relaxed text-muted">
        The page file lets Windows move rarely used memory to disk. Blob Optimizers never removes it completely: without one, apps can crash when memory
        runs out and crash dumps cannot be saved. Changes need a restart.
      </p>
      <button className="btn-primary mt-3 px-3 py-1.5 text-xs" disabled={!changed || !!error || !!busy} onClick={save}>
        {busy && <Loader2 size={13} className="animate-spin" />} Save
      </button>
    </Card>
  );
}
