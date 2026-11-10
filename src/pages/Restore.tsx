import { AlertTriangle, ArrowRight, Download, FileClock, History, Loader2, LifeBuoy, RotateCcw, ShieldCheck, ShieldOff, Undo2, Upload } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { HistoryPanel } from "../components/terminal/HistoryPanel";
import { Badge, Card, Skeleton } from "../components/ui/Card";
import { Modal } from "../components/ui/Modal";
import { OperationButton } from "../components/ui/OperationButton";
import { useEngine } from "../context/EngineContext";
import { useToast } from "../context/ToastContext";
import { useJob } from "../hooks/useJob";
import {
  api,
  errorMessage,
  type HistoryEntry,
  type ImportChange,
  type LastPreset,
  type ProfileFile,
  type RegBackup,
  type RemovedApp,
  type ServiceInfo,
  type StartupList,
  type TweakInfo,
} from "../lib/api";
import { formatBytes, formatTime } from "../lib/format";
import type { PageId } from "../navigation";

interface RestorePoint {
  sequence: number;
  created: string;
  type: string;
  description: string;
}

const STATE = { on: "On", off: "Off", custom: "Custom" } as const;

function SystemRestoreCard() {
  const { waitFor } = useEngine();
  const { run, busy } = useJob();
  const toast = useToast();
  const [status, setStatus] = useState<{ enabled: boolean; disabledByPolicy: boolean } | null>(null);
  const [points, setPoints] = useState<RestorePoint[] | null>(null);
  const [description, setDescription] = useState("Manual restore point");
  const [rollback, setRollback] = useState<RestorePoint | null>(null);

  const loadStatus = useCallback(() => api.restoreStatus().then(setStatus).catch(() => undefined), []);
  useEffect(() => {
    loadStatus();
  }, [loadStatus]);

  const listPoints = async () => {
    const done = await run("list", () => api.restoreList(), { failure: "Could not read restore points" });
    if (done?.status !== "succeeded") return;
    const lines = await api.jobOutput(done.id);
    setPoints(
      lines
        .map((l) => l.text)
        .filter((t) => t.startsWith("##RP "))
        .map((t) => {
          const [seq, created, type, ...desc] = t.slice(5).split("|");
          return { sequence: +seq, created, type, description: desc.join("|") };
        })
        .reverse(),
    );
  };

  const doRollback = async () => {
    if (!rollback) return;
    const rp = rollback;
    setRollback(null);
    try {
      const e = await api.restoreRollback(rp.sequence);
      toast("warn", "Windows is restoring", "Save your work: the PC restarts to finish the restore.");
      await waitFor(e.id);
    } catch (e) {
      toast("error", "Could not start the restore", errorMessage(e));
    }
  };

  const safeDescription = description.replace(/[^\p{L}\p{N} .,_()\-:]/gu, " ").trim().slice(0, 60) || "Manual restore point";

  return (
    <Card title="System Restore" icon={LifeBuoy} className="h-full">
      {status && (
        <div className={`mb-4 flex items-center gap-3 rounded-xl px-3 py-2.5 text-[13px] ${status.enabled ? "bg-good/10" : "bg-warn/10"}`}>
          {status.enabled ? <ShieldCheck size={16} className="text-good" /> : <ShieldOff size={16} className="text-warn" />}
          <span className="flex-1">
            {status.enabled
              ? "System Protection is on: restore points can be created."
              : status.disabledByPolicy
                ? "System Restore is turned off by a group policy on this PC."
                : "System Protection is off, so no restore points can be created."}
          </span>
          {!status.enabled && !status.disabledByPolicy && (
            <OperationButton op="restore.enable_protection" className="btn-primary px-3 py-1.5 text-xs" onDone={() => loadStatus()}>
              Turn on
            </OperationButton>
          )}
        </div>
      )}
      <div className="flex flex-wrap items-center gap-2">
        <input
          value={description}
          maxLength={60}
          onChange={(e) => setDescription(e.target.value)}
          aria-label="Restore point description"
          className="min-w-0 flex-1 rounded-xl border border-line bg-base/60 px-3 py-1.5 text-[13px] outline-none focus:border-accent/60"
        />
        <OperationButton op="restore.create_point" params={{ description: safeDescription }} className="btn-primary px-3 py-1.5 text-xs" confirm onDone={(ok) => ok && points && listPoints()}>
          Create restore point
        </OperationButton>
      </div>
      <div className="mt-4 flex items-center justify-between">
        <span className="label">Restore points</span>
        <button className="flex items-center gap-1 text-xs text-muted hover:text-fg" disabled={!!busy} onClick={listPoints}>
          {busy === "list" ? <Loader2 size={12} className="animate-spin" /> : <FileClock size={12} />} {points ? "Refresh" : "Show restore points"}
        </button>
      </div>
      {points && (
        <ul className="mt-2 max-h-64 space-y-1.5 overflow-y-auto pr-1">
          {points.length === 0 && <li className="text-[13px] text-muted">No restore points yet.</li>}
          {points.map((p) => (
            <li key={p.sequence} className="flex items-center gap-3 rounded-xl bg-raised/60 px-3 py-2 text-[13px]">
              <div className="min-w-0 flex-1">
                <div className="truncate font-medium">{p.description}</div>
                <div className="text-xs text-muted">
                  #{p.sequence} · {new Date(p.created).toLocaleString()}
                </div>
              </div>
              <button className="btn-ghost px-2.5 py-1 text-xs" onClick={() => setRollback(p)}>
                <RotateCcw size={12} /> Roll back
              </button>
            </li>
          ))}
        </ul>
      )}
      <p className="mt-3 text-xs text-muted">Reading restore points needs administrator permission. Restore points cover system files, settings and installed programs, not your documents.</p>

      <Modal
        open={!!rollback}
        title="Roll Windows back?"
        onClose={() => setRollback(null)}
        footer={
          <>
            <button className="btn-ghost" onClick={() => setRollback(null)}>
              Cancel
            </button>
            <button className="btn-primary bg-bad" onClick={doRollback}>
              Restart and restore
            </button>
          </>
        }
      >
        {rollback && (
          <div className="space-y-3 text-[13px] leading-relaxed">
            <p>
              Windows will restore system files, settings, drivers and installed programs to <b>{rollback.description}</b> ({new Date(rollback.created).toLocaleString()}).
            </p>
            <p className="flex gap-2 rounded-xl border border-bad/30 bg-bad/10 p-3">
              <AlertTriangle size={16} className="mt-0.5 shrink-0 text-bad" />
              The PC restarts immediately. Save and close your work first. Programs installed after this point are removed; your documents, photos and other personal
              files are not changed. Windows creates a restore point first, so this can be undone.
            </p>
          </div>
        )}
      </Modal>
    </Card>
  );
}

function UndoCard() {
  const { run, busy } = useJob();
  const [tweaks, setTweaks] = useState<TweakInfo[] | null>(null);
  const [last, setLast] = useState<LastPreset | null>(null);
  const [services, setServices] = useState<ServiceInfo[]>([]);
  const [startup, setStartup] = useState<StartupList | null>(null);
  const [removed, setRemoved] = useState<RemovedApp[]>([]);
  const [regs, setRegs] = useState<RegBackup[]>([]);
  const [resetAll, setResetAll] = useState(false);

  const load = useCallback(async () => {
    const [t, p, s, st, r, rg] = await Promise.allSettled([api.tweaks(), api.presets(), api.services(), api.startup(), api.removedApps(), api.registryBackups()]);
    if (t.status === "fulfilled") setTweaks(t.value);
    if (p.status === "fulfilled") setLast(p.value[1]);
    if (s.status === "fulfilled") setServices(s.value.filter((x) => x.original && x.startMode.toLowerCase() !== x.original));
    if (st.status === "fulfilled") setStartup(st.value);
    if (r.status === "fulfilled") setRemoved(r.value);
    if (rg.status === "fulfilled") setRegs(rg.value);
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const act = async (key: string, start: () => Promise<HistoryEntry>, label: string) => {
    await run(key, start, { success: `${label} done`, failure: `${label} failed` });
    load();
  };

  const undoable = (tweaks ?? []).filter((t) => t.canUndo).sort((a, b) => (b.lastChangeAt ?? 0) - (a.lastChangeAt ?? 0));
  const notDefault = (tweaks ?? []).filter((t) => !t.atDefault);
  const empty = tweaks && !undoable.length && !last && !services.length && !startup?.removed.length && !removed.length && !regs.length;

  const Row = ({ k, title, sub, action, onClick }: { k: string; title: string; sub: string; action: string; onClick: () => void }) => (
    <li className="flex items-center gap-3 rounded-xl bg-raised/60 px-3 py-2 text-[13px]">
      <div className="min-w-0 flex-1">
        <div className="truncate font-medium">{title}</div>
        <div className="truncate text-xs text-muted">{sub}</div>
      </div>
      <button className="btn-ghost px-2.5 py-1 text-xs" disabled={!!busy} onClick={onClick}>
        {busy === k ? <Loader2 size={12} className="animate-spin" /> : <Undo2 size={12} />} {action}
      </button>
    </li>
  );

  return (
    <Card
      title="Changes you can undo"
      icon={Undo2}
      actions={
        <button className="btn-ghost px-3 py-1.5 text-xs" disabled={!notDefault.length || !!busy} onClick={() => setResetAll(true)}>
          <RotateCcw size={13} /> Reset all tweaks to Windows defaults
        </button>
      }
    >
      {!tweaks ? (
        <Skeleton className="h-32 w-full" />
      ) : empty ? (
        <p className="py-6 text-center text-sm text-muted">Nothing to undo yet. Changes made through Blob Optimizers will show up here.</p>
      ) : (
        <div className="space-y-4">
          {last && (
            <section>
              <div className="label mb-1.5">Gaming preset</div>
              <ul>
                <Row
                  k="preset"
                  title={`${last.preset[0].toUpperCase()}${last.preset.slice(1)} preset`}
                  sub={`Applied ${formatTime(last.at)} · ${last.tweaks.length} settings${last.previousPlanName ? ` · was on ${last.previousPlanName}` : ""}`}
                  action="Revert"
                  onClick={() => act("preset", () => api.revertPreset(), "Revert preset")}
                />
              </ul>
            </section>
          )}
          {undoable.length > 0 && (
            <section>
              <div className="label mb-1.5">Tweaks ({undoable.length})</div>
              <ul className="max-h-72 space-y-1.5 overflow-y-auto pr-1">
                {undoable.map((t) => (
                  <Row
                    key={t.id}
                    k={t.id}
                    title={t.title}
                    sub={`Changed ${t.lastChangeAt ? formatTime(t.lastChangeAt) : ""} · now ${t.atDefault ? "Windows default" : STATE[t.state]}`}
                    action="Undo"
                    onClick={() => act(t.id, () => api.applyTweaks([{ id: t.id, target: "undo" }]), `Undo ${t.title}`)}
                  />
                ))}
              </ul>
            </section>
          )}
          {services.length > 0 && (
            <section>
              <div className="label mb-1.5">Services</div>
              <ul className="space-y-1.5">
                {services.map((s) => (
                  <Row
                    key={s.name}
                    k={s.name}
                    title={s.displayName}
                    sub={`Now ${s.startMode}, originally ${s.original}`}
                    action="Restore"
                    onClick={() => act(s.name, () => api.setServiceStart(s.name, s.original!), `Restore ${s.displayName}`)}
                  />
                ))}
              </ul>
            </section>
          )}
          {startup && startup.removed.length > 0 && (
            <section>
              <div className="label mb-1.5">Removed startup entries</div>
              <ul className="space-y-1.5">
                {startup.removed.map((r) => (
                  <Row
                    key={`${r.location}|${r.name}`}
                    k={`st-${r.name}`}
                    title={r.name}
                    sub={`${r.location} · removed ${formatTime(r.removedAt)}`}
                    action="Restore"
                    onClick={() => act(`st-${r.name}`, () => api.restoreStartup(r.name, r.location), `Restore ${r.name}`)}
                  />
                ))}
              </ul>
            </section>
          )}
          {removed.length > 0 && (
            <section>
              <div className="label mb-1.5">Removed Store apps</div>
              <ul className="space-y-1.5">
                {removed.map((r) => (
                  <Row
                    key={r.packageName}
                    k={`app-${r.packageName}`}
                    title={r.name}
                    sub={`Removed ${formatTime(r.removedAt)}`}
                    action="Reinstall"
                    onClick={() => act(`app-${r.packageName}`, () => api.reinstallApp(r.packageName), `Reinstall ${r.name}`)}
                  />
                ))}
              </ul>
            </section>
          )}
          {regs.length > 0 && (
            <section>
              <div className="label mb-1.5">Registry backups (from leftover cleanup)</div>
              <ul className="space-y-1.5">
                {regs.map((r) => (
                  <Row
                    key={r.file}
                    k={`reg-${r.file}`}
                    title={r.keys[0] ?? r.file}
                    sub={`${formatTime(r.created)} · ${formatBytes(r.sizeBytes)}${r.keys.length > 1 ? ` · ${r.keys.length} keys` : ""}${r.machineWide ? " · needs admin" : ""}`}
                    action="Restore"
                    onClick={() => act(`reg-${r.file}`, () => api.importRegistryBackup(r.file), "Restore registry backup")}
                  />
                ))}
              </ul>
            </section>
          )}
        </div>
      )}
      <p className="mt-4 text-xs text-muted">
        Uninstalled desktop programs, deleted temporary files and network resets cannot be undone here; they are marked "not reversible" in the history below. Leftover
        folders went to the Recycle Bin. A restore point covers the rest.
      </p>

      <Modal
        open={resetAll}
        title={`Reset ${notDefault.length} tweaks to Windows defaults?`}
        onClose={() => setResetAll(false)}
        width={580}
        footer={
          <>
            <button className="btn-ghost" onClick={() => setResetAll(false)}>
              Cancel
            </button>
            <button
              className="btn-primary"
              onClick={async () => {
                setResetAll(false);
                await act("reset-all", () => api.applyTweaks(notDefault.map((t) => ({ id: t.id, target: "default" }))), "Reset to Windows defaults");
              }}
            >
              Reset all
            </button>
          </>
        }
      >
        <ul className="max-h-72 space-y-1 overflow-y-auto pr-1 text-[13px]">
          {notDefault.map((t) => (
            <li key={t.id} className="flex items-center gap-2">
              <span className="flex-1">{t.title}</span>
              <span className="text-xs text-muted">{STATE[t.state]}</span>
              <ArrowRight size={11} className="text-muted" />
              <span className="text-xs">Default</span>
            </li>
          ))}
        </ul>
        <p className="mt-3 text-xs text-muted">Each change is snapshotted first, so "Undo" still brings back your current values.</p>
      </Modal>
    </Card>
  );
}

function ProfilesCard() {
  const toast = useToast();
  const { run, busy } = useJob();
  const [folder, setFolder] = useState("");
  const [files, setFiles] = useState<ProfileFile[] | null>(null);
  const [preview, setPreview] = useState<{ file: string; changes: ImportChange[] } | null>(null);

  const load = useCallback(() => {
    api.profiles()
      .then(([f, list]) => {
        setFolder(f);
        setFiles(list);
      })
      .catch(() => setFiles([]));
  }, []);
  useEffect(() => {
    load();
  }, [load]);

  const exportNow = async () => {
    try {
      const path = await api.exportProfile();
      toast("success", "Tweak profile saved", path);
      load();
    } catch (e) {
      toast("error", "Could not save the profile", errorMessage(e));
    }
  };

  const open = async (file: string) => {
    try {
      setPreview({ file, changes: await api.previewProfile(file) });
    } catch (e) {
      toast("error", "Could not read the profile", errorMessage(e));
    }
  };

  const apply = async () => {
    if (!preview) return;
    const changes = preview.changes;
    setPreview(null);
    await run("profile", () => api.applyTweaks(changes.map((c) => ({ id: c.id, target: c.target }))), {
      success: `Applied ${changes.length} settings from the profile`,
      failure: "Some settings could not be applied",
    });
  };

  return (
    <Card
      title="Saved tweak profiles"
      icon={Download}
      className="h-full"
      actions={
        <button className="btn-primary px-3 py-1.5 text-xs" onClick={exportNow}>
          <Download size={13} /> Save current settings
        </button>
      }
    >
      <p className="text-xs text-muted">
        Save the state of every tweak to a file, then apply it again later or on another PC. Files are kept in <span className="selectable font-mono">{folder}</span>.
      </p>
      {!files ? (
        <Skeleton className="mt-3 h-16 w-full" />
      ) : files.length === 0 ? (
        <p className="mt-4 text-[13px] text-muted">No saved profiles yet.</p>
      ) : (
        <ul className="mt-3 space-y-1.5">
          {files.map((f) => (
            <li key={f.file} className="flex items-center gap-3 rounded-xl bg-raised/60 px-3 py-2 text-[13px]">
              <div className="min-w-0 flex-1">
                <div className="truncate font-medium">{f.file}</div>
                <div className="text-xs text-muted">
                  {formatTime(f.created)} · {f.computer} · {f.count} settings
                </div>
              </div>
              <button className="btn-ghost px-2.5 py-1 text-xs" disabled={!!busy} onClick={() => open(f.file)}>
                <Upload size={12} /> Apply…
              </button>
            </li>
          ))}
        </ul>
      )}
      <Modal
        open={!!preview}
        title="Apply saved profile?"
        onClose={() => setPreview(null)}
        footer={
          <>
            <button className="btn-ghost" onClick={() => setPreview(null)}>
              Cancel
            </button>
            <button className="btn-primary" disabled={!preview?.changes.length} onClick={apply}>
              Apply {preview?.changes.length ?? 0} changes
            </button>
          </>
        }
      >
        {preview && preview.changes.length === 0 ? (
          <p className="text-[13px]">This PC already matches the profile.</p>
        ) : (
          <ul className="max-h-72 space-y-1 overflow-y-auto pr-1 text-[13px]">
            {preview?.changes.map((c) => (
              <li key={c.id} className="flex items-center gap-2">
                <span className="flex-1">{c.title}</span>
                <span className="text-xs text-muted">{STATE[c.from]}</span>
                <ArrowRight size={11} className="text-muted" />
                <span className="text-xs">{c.target === "on" ? "On" : "Off"}</span>
              </li>
            ))}
          </ul>
        )}
      </Modal>
    </Card>
  );
}

export function Restore({ onNavigate }: { onNavigate: (id: PageId) => void }) {
  const { openFromHistory } = useEngine();
  return (
    <div className="space-y-5">
      <div className="grid grid-cols-1 gap-5 xl:grid-cols-2">
        <SystemRestoreCard />
        <ProfilesCard />
      </div>
      <UndoCard />
      <section className="card flex h-[520px] flex-col overflow-hidden">
        <header className="flex items-center gap-2.5 border-b border-line/60 px-5 py-3.5">
          <History size={16} className="text-accent" />
          <h2 className="flex-1 text-[13.5px] font-semibold">Optimization history</h2>
          <Badge>Click an entry to see its output</Badge>
        </header>
        <HistoryPanel
          onOpen={async (e) => {
            await openFromHistory(e);
            onNavigate("terminal");
          }}
        />
      </section>
    </div>
  );
}
