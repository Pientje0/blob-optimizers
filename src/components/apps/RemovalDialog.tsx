import { AlertTriangle, CheckCircle2, Circle, FolderX, Info, Loader2, MinusCircle, SquareTerminal, Trash2, XCircle } from "lucide-react";
import { useMemo, useState } from "react";
import { useEngine } from "../../context/EngineContext";
import { useSettings } from "../../context/SettingsContext";
import { useSystem } from "../../context/SystemContext";
import { api, errorMessage, type HistoryEntry, type InstalledApp, type LeftoverScan } from "../../lib/api";
import { formatBytes } from "../../lib/format";
import { Badge } from "../ui/Card";
import { Checkbox } from "../ui/Controls";
import { Modal } from "../ui/Modal";
import { RecommendationBadge } from "./AppBadges";

type StepStatus = "pending" | "running" | "ok" | "failed" | "skipped";

interface Step {
  key: string;
  label: string;
  status: StepStatus;
  detail?: string;
  jobId?: string;
}

type Phase = "confirm" | "running" | "leftovers" | "cleaning" | "done";

interface Props {
  apps: InstalledApp[];
  onClose: () => void;
  /** Called after changes were made so the caller can refresh its list. */
  onChanged: () => void;
  onOpenTerminal: () => void;
}

const STEP_ICON: Record<StepStatus, JSX.Element> = {
  pending: <Circle size={15} className="text-muted" />,
  running: <Loader2 size={15} className="animate-spin text-accent" />,
  ok: <CheckCircle2 size={15} className="text-good" />,
  failed: <XCircle size={15} className="text-bad" />,
  skipped: <MinusCircle size={15} className="text-muted" />,
};

/** Restore point descriptions only allow a safe character set. */
function safeDescription(apps: InstalledApp[]) {
  const text = apps.length === 1 ? `Before removing ${apps[0].name}` : `Before removing ${apps.length} apps`;
  return text.replace(/[^\p{L}\p{N} .,_()\-:]/gu, " ").replace(/\s+/g, " ").trim().slice(0, 60);
}

export function RemovalDialog({ apps, onClose, onChanged, onOpenTerminal }: Props) {
  const { runOperation, track, waitFor } = useEngine();
  const { appInfo } = useSystem();
  const desktop = apps.filter((a) => a.kind === "win32");
  const store = apps.filter((a) => a.kind === "appx");
  const risky = apps.some((a) => a.recommendation === "caution");
  const { settings } = useSettings();
  const [restorePoint, setRestorePoint] = useState(settings.restorePointDefault && (desktop.length > 0 || risky));
  const [phase, setPhase] = useState<Phase>("confirm");
  const [steps, setSteps] = useState<Step[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [scans, setScans] = useState<LeftoverScan[]>([]);
  const [picked, setPicked] = useState<Record<string, Set<string>>>({});

  const busy = phase === "running" || phase === "cleaning";
  const setStep = (key: string, patch: Partial<Step>) => setSteps((s) => s.map((x) => (x.key === key ? { ...x, ...patch } : x)));

  const runJob = async (key: string, start: () => Promise<HistoryEntry>): Promise<HistoryEntry | null> => {
    setStep(key, { status: "running" });
    try {
      const entry = await start();
      track(entry);
      setStep(key, { jobId: entry.id });
      const done = await waitFor(entry.id);
      setStep(key, { status: done.status === "succeeded" ? "ok" : "failed", detail: done.status === "succeeded" ? undefined : done.error ?? done.status });
      return done;
    } catch (e) {
      setStep(key, { status: "failed", detail: errorMessage(e) });
      return null;
    }
  };

  const start = async () => {
    setError(null);
    const plan: Step[] = [];
    if (restorePoint) plan.push({ key: "rp", label: "Create restore point", status: "pending" });
    if (store.length) plan.push({ key: "store", label: store.length === 1 ? `Remove ${store[0].name}` : `Remove ${store.length} Store apps`, status: "pending" });
    desktop.forEach((a) => plan.push({ key: a.id, label: `Uninstall ${a.name}`, status: "pending" }));
    setSteps(plan);
    setPhase("running");

    if (restorePoint) {
      const rp = await runJob("rp", () => runOperation("restore.create_point", { description: safeDescription(apps) }));
      if (rp?.status !== "succeeded") {
        setSteps((s) => s.map((x) => (x.status === "pending" ? { ...x, status: "skipped" } : x)));
        setRestorePoint(false);
        setPhase("confirm");
        setError(
          `No restore point was created (${rp?.error ?? "see Terminal for details"}), so nothing was removed. If System Protection is off, turn it on with the "Turn on System Protection" operation in Terminal, or untick the restore point option to continue without one.`,
        );
        return;
      }
    }

    let changed = false;
    if (store.length) {
      const r = await runJob("store", () => api.removeStoreApps(store.map((a) => a.id)));
      changed = true;
      if (r?.status !== "succeeded") setStep("store", { detail: "Some apps could not be removed. Open Terminal for details." });
    }

    const uninstalled: InstalledApp[] = [];
    for (const app of desktop) {
      const r = await runJob(app.id, () => api.uninstallApp(app.id));
      changed = true;
      if (r?.status === "succeeded") uninstalled.push(app);
    }
    if (changed) onChanged();

    // Look for leftovers of programs that are now gone.
    const found: LeftoverScan[] = [];
    for (const app of uninstalled) {
      try {
        const scan = await api.scanLeftovers(app.id);
        if (scan.items.length) found.push(scan);
      } catch {
        /* still installed or nothing to scan */
      }
    }
    if (found.length) {
      setScans(found);
      setPicked(Object.fromEntries(found.map((s) => [s.scanId, new Set(s.items.filter((i) => i.defaultSelected).map((i) => i.id))])));
      setPhase("leftovers");
    } else {
      setPhase("done");
    }
  };

  const cleanup = async () => {
    setPhase("cleaning");
    const extra: Step[] = scans
      .filter((s) => picked[s.scanId]?.size)
      .map((s) => ({ key: `clean-${s.scanId}`, label: `Clean up ${s.appName} leftovers`, status: "pending" as StepStatus }));
    setSteps((s) => [...s, ...extra]);
    for (const s of scans) {
      const ids = [...(picked[s.scanId] ?? [])];
      if (ids.length) await runJob(`clean-${s.scanId}`, () => api.cleanupLeftovers(s.scanId, ids));
    }
    setPhase("done");
  };

  const selectedLeftovers = useMemo(() => scans.flatMap((s) => s.items.filter((i) => picked[s.scanId]?.has(i.id))), [scans, picked]);
  const needsUac = !appInfo?.elevated && (restorePoint || selectedLeftovers.some((i) => i.requiresAdmin));

  const title =
    phase === "confirm" ? `Remove ${apps.length} app${apps.length === 1 ? "" : "s"}?` : phase === "leftovers" ? "Leftovers found" : phase === "done" ? "Finished" : "Removing apps…";

  return (
    <Modal
      open
      title={title}
      onClose={() => !busy && onClose()}
      width={640}
      footer={
        phase === "confirm" ? (
          <>
            <button className="btn-ghost" onClick={onClose}>
              Cancel
            </button>
            <button className="btn-primary bg-bad" onClick={start}>
              <Trash2 size={15} /> Remove {apps.length}
            </button>
          </>
        ) : phase === "leftovers" ? (
          <>
            <button className="btn-ghost" onClick={() => setPhase("done")}>
              Keep everything
            </button>
            <button className="btn-primary" disabled={!selectedLeftovers.length} onClick={cleanup}>
              <FolderX size={15} /> Clean up {selectedLeftovers.length} selected
            </button>
          </>
        ) : (
          <>
            <button className="btn-ghost" onClick={onOpenTerminal}>
              <SquareTerminal size={15} /> View output
            </button>
            <button className="btn-primary" disabled={busy} onClick={onClose}>
              {busy ? "Working…" : "Close"}
            </button>
          </>
        )
      }
    >
      {phase === "confirm" && (
        <div className="space-y-4">
          {error && (
            <div className="flex gap-2.5 rounded-xl border border-bad/30 bg-bad/10 p-3 text-[13px] leading-relaxed">
              <XCircle size={16} className="mt-0.5 shrink-0 text-bad" /> {error}
            </div>
          )}
          <ul className="max-h-64 space-y-1.5 overflow-y-auto pr-1">
            {apps.map((a) => (
              <li key={a.id} className="rounded-xl bg-raised/60 px-3 py-2.5">
                <div className="flex items-center gap-2">
                  <span className="flex-1 font-medium">{a.name}</span>
                  <RecommendationBadge rec={a.recommendation} />
                </div>
                <div className="mt-1 text-xs leading-relaxed text-muted">{a.note}</div>
              </li>
            ))}
          </ul>

          <div className="space-y-2 rounded-xl border border-line p-3 text-[13px] leading-relaxed">
            {store.length > 0 && (
              <p className="flex gap-2">
                <Info size={15} className="mt-0.5 shrink-0 text-accent" />
                <span>
                  Store apps are removed for your account only. You can bring them back later from <b>Removed apps</b>
                  {store.some((a) => !a.reinstallable) ? " (apps without a Store listing must be reinstalled manually)" : ""}.
                </span>
              </p>
            )}
            {desktop.length > 0 && (
              <p className="flex gap-2">
                <Info size={15} className="mt-0.5 shrink-0 text-accent" />
                <span>
                  Desktop programs are removed with their own uninstaller, one at a time. Some show their own window or ask for administrator
                  permission; answer those to continue. Afterwards, Blob Optimizers looks for leftover files and lets you choose what to clean up.
                </span>
              </p>
            )}
            {risky && (
              <p className="flex gap-2">
                <AlertTriangle size={15} className="mt-0.5 shrink-0 text-warn" />
                <span>Some selected apps are marked Caution. Read their notes above before continuing.</span>
              </p>
            )}
          </div>

          <label className="flex cursor-pointer items-start gap-3 rounded-xl bg-raised/60 p-3">
            <Checkbox checked={restorePoint} onChange={setRestorePoint} label="Create restore point" />
            <span className="text-[13px] leading-relaxed">
              <span className="font-medium">Create a restore point first</span>
              <span className="block text-xs text-muted">
                Lets you roll Windows back if something stops working.{!appInfo?.elevated && " Windows will ask for administrator permission."}
              </span>
            </span>
          </label>
        </div>
      )}

      {(phase === "running" || phase === "cleaning" || phase === "done") && (
        <ul className="space-y-1.5">
          {steps.map((s) => (
            <li key={s.key} className="flex items-start gap-3 rounded-xl bg-raised/50 px-3 py-2.5">
              <span className="mt-0.5">{STEP_ICON[s.status]}</span>
              <div className="min-w-0 flex-1">
                <div className="text-[13px] font-medium">{s.label}</div>
                {s.detail && <div className="mt-0.5 text-xs text-muted">{s.detail}</div>}
              </div>
            </li>
          ))}
          {phase === "done" && (
            <li className="pt-2 text-xs text-muted">
              Every step is recorded in Terminal → History. Removed Store apps can be restored from the Removed apps tab of the Debloater.
            </li>
          )}
        </ul>
      )}

      {phase === "leftovers" && (
        <div className="space-y-4">
          <p className="text-[13px] leading-relaxed text-muted">
            These were left behind by the uninstalled programs. Folders are moved to the Recycle Bin and registry keys are backed up before
            deletion, so you can undo this. Items holding your settings are not selected by default.
          </p>
          {scans.map((s) => (
            <div key={s.scanId}>
              <div className="label mb-1.5">{s.appName}</div>
              <ul className="space-y-1.5">
                {s.items.map((i) => {
                  const on = picked[s.scanId]?.has(i.id) ?? false;
                  const toggle = (v: boolean) =>
                    setPicked((p) => {
                      const next = new Set(p[s.scanId]);
                      if (v) next.add(i.id);
                      else next.delete(i.id);
                      return { ...p, [s.scanId]: next };
                    });
                  return (
                    <li key={i.id} className="flex cursor-pointer items-start gap-3 rounded-xl bg-raised/60 px-3 py-2.5" onClick={() => toggle(!on)}>
                      <span className="pt-0.5">
                        <Checkbox checked={on} onChange={toggle} label={i.path} />
                      </span>
                      <div className="min-w-0 flex-1">
                        <div className="selectable break-all font-mono text-[12px]">{i.path}</div>
                        <div className="mt-1 flex flex-wrap items-center gap-1.5 text-xs text-muted">
                          <Badge>{i.kind === "folder" ? `${formatBytes(i.sizeBytes)} · ${i.items} files` : "Registry key"}</Badge>
                          {i.userData && <Badge tone="warn">Your settings/data</Badge>}
                          {i.requiresAdmin && <Badge tone="accent">Admin</Badge>}
                          <span>{i.reason}</span>
                        </div>
                      </div>
                    </li>
                  );
                })}
              </ul>
            </div>
          ))}
          {needsUac && <p className="text-xs text-muted">Some items are in system folders; Windows will ask for administrator permission.</p>}
        </div>
      )}
    </Modal>
  );
}
