import { AlertTriangle, ArrowRight, Check, Flame, Gamepad2, Leaf, Loader2, Undo2 } from "lucide-react";
import { useEffect, useState } from "react";
import { useEngine } from "../../context/EngineContext";
import { useSettings } from "../../context/SettingsContext";
import { useSystem } from "../../context/SystemContext";
import { useToast } from "../../context/ToastContext";
import { useJob } from "../../hooks/useJob";
import { api, type LastPreset, type PresetInfo, type TweakState } from "../../lib/api";
import { formatTime } from "../../lib/format";
import { takeIntent } from "../../lib/intent";
import { Badge, Skeleton } from "../ui/Card";
import { Checkbox } from "../ui/Controls";
import { Modal } from "../ui/Modal";

const ICON = { balanced: Leaf, gaming: Gamepad2, maximum: Flame };
const STATE: Record<TweakState, string> = { on: "On", off: "Off", custom: "Custom" };

interface Props {
  presets: PresetInfo[] | null;
  last: LastPreset | null;
  reload: () => Promise<void>;
}

export function PresetsSection({ presets, last, reload }: Props) {
  const { runOperation, waitFor } = useEngine();
  const { appInfo } = useSystem();
  const toast = useToast();
  const { run, busy } = useJob();
  const [preview, setPreview] = useState<PresetInfo | null>(null);
  const { settings } = useSettings();
  const [restorePoint, setRestorePoint] = useState(settings.restorePointDefault);
  const [working, setWorking] = useState(false);

  // Opened from the Dashboard's quick action: show that preset's preview straight away.
  useEffect(() => {
    if (!presets) return;
    const intent = takeIntent("preview-preset");
    const p = intent && presets.find((x) => x.id === intent.preset);
    if (p) setPreview(p);
  }, [presets]);

  const apply = async () => {
    if (!preview) return;
    const p = preview;
    setWorking(true);
    try {
      if (restorePoint) {
        const rp = await runOperation("restore.create_point", { description: `Before ${p.name} gaming preset` });
        const done = await waitFor(rp.id);
        if (done.status !== "succeeded") {
          setRestorePoint(false);
          toast("error", "No restore point, so nothing was applied", `${done.error ?? ""} Untick the restore point option to continue without one.`);
          return;
        }
      }
      setPreview(null);
      await run(`preset-${p.id}`, () => api.applyPreset(p.id), {
        success: `${p.name} preset applied. Some changes take effect after signing out or restarting.`,
        failure: `The ${p.name} preset was only partly applied`,
      });
      await reload();
    } finally {
      setWorking(false);
    }
  };

  const revert = async () => {
    await run("revert", () => api.revertPreset(), { success: "Last preset reverted", failure: "Could not revert the preset" });
    await reload();
  };

  return (
    <section className="space-y-3">
      <div className="flex items-center justify-between">
        <h2 className="text-[15px] font-semibold">Gaming presets</h2>
        {last && (
          <button className="btn-ghost px-3 py-1.5 text-xs" disabled={!!busy} onClick={revert} title="Undo every change the last preset made and switch back to the previous power plan">
            {busy === "revert" ? <Loader2 size={13} className="animate-spin" /> : <Undo2 size={13} />} Revert last preset ({last.preset}, {formatTime(last.at)})
          </button>
        )}
      </div>
      <div className="grid grid-cols-1 gap-4 lg:grid-cols-3">
        {!presets
          ? Array.from({ length: 3 }).map((_, i) => <Skeleton key={i} className="h-56" />)
          : presets.map((p) => {
              const Icon = ICON[p.id];
              const count = p.changes.length + (p.planChange ? 1 : 0);
              return (
                <div key={p.id} className={`card animate-fade-up flex flex-col p-5 ${p.active ? "border-accent/50 glow-accent" : ""}`}>
                  <div className="flex items-center gap-2.5">
                    <span className={`flex h-9 w-9 items-center justify-center rounded-xl ${p.id === "maximum" ? "bg-warn/15 text-warn" : "bg-accent/15 text-accent"}`}>
                      <Icon size={18} />
                    </span>
                    <span className="flex-1 text-[15px] font-semibold">{p.name}</span>
                    {p.active && (
                      <Badge tone="good">
                        <Check size={11} /> Active
                      </Badge>
                    )}
                  </div>
                  <p className="mt-3 flex-1 text-[13px] leading-relaxed text-muted">{p.description}</p>
                  {p.warning && (
                    <p className="mt-3 flex gap-2 text-xs leading-relaxed text-warn">
                      <AlertTriangle size={14} className="mt-px shrink-0" /> {p.warning}
                    </p>
                  )}
                  <button className={`${p.active ? "btn-ghost" : "btn-primary"} mt-4`} disabled={p.active || !!busy} onClick={() => setPreview(p)}>
                    {busy === `preset-${p.id}` ? <Loader2 size={15} className="animate-spin" /> : null}
                    {p.active ? "Already active" : `Review ${count} change${count === 1 ? "" : "s"}`}
                  </button>
                </div>
              );
            })}
      </div>
      <p className="text-xs text-muted">
        Presets change Windows settings and the power plan only. No preset promises a specific FPS gain: measure with the FPS monitor below before and after.
      </p>

      <Modal
        open={!!preview}
        title={`Apply the ${preview?.name} preset?`}
        onClose={() => !working && setPreview(null)}
        width={620}
        footer={
          <>
            <button className="btn-ghost" disabled={working} onClick={() => setPreview(null)}>
              Cancel
            </button>
            <button className="btn-primary" disabled={working} onClick={apply}>
              {working && <Loader2 size={15} className="animate-spin" />} Apply preset
            </button>
          </>
        }
      >
        {preview && (
          <div className="space-y-4">
            {preview.warning && (
              <p className="flex gap-2 rounded-xl border border-warn/30 bg-warn/10 p-3 text-[13px] leading-relaxed">
                <AlertTriangle size={16} className="mt-0.5 shrink-0 text-warn" /> {preview.warning}
              </p>
            )}
            <ul className="max-h-72 space-y-1.5 overflow-y-auto pr-1">
              {preview.planChange && (
                <li className="flex items-center gap-3 rounded-xl bg-raised/60 px-3 py-2 text-[13px]">
                  <span className="flex-1 font-medium">Power plan</span>
                  <span className="text-xs text-muted">{preview.planChange}</span>
                </li>
              )}
              {preview.changes.map((c) => (
                <li key={c.id} className="rounded-xl bg-raised/60 px-3 py-2">
                  <div className="flex items-center gap-3 text-[13px]">
                    <span className="flex-1 font-medium">{c.title}</span>
                    <span className="flex items-center gap-1.5 text-xs text-muted">
                      {STATE[c.from]} <ArrowRight size={11} /> <span className="text-fg">{STATE[c.to]}</span>
                    </span>
                  </div>
                  {c.caveat && c.to === "on" && <div className="mt-1 text-xs text-warn/90">{c.caveat}</div>}
                </li>
              ))}
            </ul>
            <label className="flex cursor-pointer items-start gap-3 rounded-xl bg-raised/60 p-3">
              <Checkbox checked={restorePoint} onChange={setRestorePoint} label="Create restore point" />
              <span className="text-[13px]">
                <span className="font-medium">Create a restore point first</span>
                <span className="block text-xs text-muted">If it fails, nothing is applied.{!appInfo?.elevated && " Windows will ask for administrator permission."}</span>
              </span>
            </label>
            <p className="text-xs text-muted">
              Every change is snapshotted and can be undone with "Revert last preset".{preview.requiresAdmin && !appInfo?.elevated ? " Some changes are system-wide and need administrator permission." : ""}
            </p>
          </div>
        )}
      </Modal>
    </section>
  );
}
