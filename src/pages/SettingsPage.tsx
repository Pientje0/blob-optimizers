import { Bell, Check, Database, Gamepad2, Globe, Info, Monitor, Palette, Power, ShieldCheck, Trash2 } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { Card } from "../components/ui/Card";
import { Switch } from "../components/ui/Controls";
import { Modal } from "../components/ui/Modal";
import { useEngine } from "../context/EngineContext";
import { ACCENTS, useSettings, type Settings } from "../context/SettingsContext";
import { useSystem } from "../context/SystemContext";
import { useToast } from "../context/ToastContext";
import { api, errorMessage } from "../lib/api";
import { UpdatesCard } from "../components/settings/UpdatesCard";

function Row({ title, description, children }: { title: string; description?: string; children: ReactNode }) {
  return (
    <div className="flex items-center gap-4 border-b border-line/40 py-3.5 last:border-0">
      <div className="min-w-0 flex-1">
        <div className="text-[13.5px] font-medium">{title}</div>
        {description && <div className="mt-0.5 text-xs leading-relaxed text-muted">{description}</div>}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

const SELECT = "rounded-lg border border-line bg-base/60 px-2.5 py-1.5 text-[13px] outline-none focus:border-accent/60";

export function SettingsPage() {
  const { settings, update } = useSettings();
  const { appInfo } = useSystem();
  const { clearHistory, track, waitFor } = useEngine();
  const toast = useToast();
  const [paths, setPaths] = useState<[string, string] | null>(null);
  const [confirmClear, setConfirmClear] = useState(false);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    api.appPaths().then(setPaths).catch(() => undefined);
  }, []);

  const set = async (patch: Partial<Settings>) => {
    setSaving(true);
    const err = await update(patch);
    setSaving(false);
    if (err) toast("error", "Setting not saved", err);
  };

  // Start-with-Windows writes a registry value through the command engine.
  const setAutostart = async (on: boolean) => {
    setSaving(true);
    try {
      const entry = await api.settingsSet({ ...settings, startWithWindows: on });
      if (entry) {
        track(entry);
        const done = await waitFor(entry.id);
        if (done.status !== "succeeded") toast("error", "Could not change startup setting", done.error ?? undefined);
      }
      await update({});
    } catch (e) {
      toast("error", "Could not change startup setting", errorMessage(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="mx-auto max-w-4xl space-y-5">
      <Card title="Appearance" icon={Palette}>
        <Row title="Theme" description="System follows Windows' light/dark setting.">
          <div className="flex rounded-xl border border-line bg-base/60 p-0.5 text-xs">
            {(["dark", "light", "system"] as const).map((t) => (
              <button
                key={t}
                disabled={saving}
                onClick={() => set({ theme: t })}
                className={`rounded-[10px] px-3 py-1.5 font-medium capitalize ${settings.theme === t ? "bg-raised text-fg" : "text-muted hover:text-fg"}`}
              >
                {t}
              </button>
            ))}
          </div>
        </Row>
        <Row title="Accent colour">
          <div className="flex gap-2">
            {Object.entries(ACCENTS).map(([id, a]) => (
              <button
                key={id}
                title={a.label}
                aria-label={a.label}
                disabled={saving}
                onClick={() => set({ accent: id })}
                className="flex h-8 w-8 items-center justify-center rounded-full ring-offset-2 ring-offset-surface transition-transform hover:scale-110"
                style={{ background: `linear-gradient(135deg, rgb(${a.a}), rgb(${a.b}))`, boxShadow: settings.accent === id ? `0 0 0 2px rgb(${a.a})` : undefined }}
              >
                {settings.accent === id && <Check size={14} className="text-white" />}
              </button>
            ))}
          </div>
        </Row>
      </Card>

      <Card title="Monitoring" icon={Monitor}>
        <Row title="Refresh rate" description="How often live CPU, GPU, memory and network figures update. Slower uses slightly less CPU. Monitoring pauses while the window is minimised.">
          <select className={SELECT} value={settings.refreshMs} disabled={saving} onChange={(e) => set({ refreshMs: +e.target.value })}>
            {[750, 1000, 1500, 2000, 3000, 5000].map((ms) => (
              <option key={ms} value={ms}>
                {ms < 1000 ? `${ms} ms` : `${ms / 1000} s`}
              </option>
            ))}
          </select>
        </Row>
      </Card>

      <Card title="Startup" icon={Power}>
        <Row title="Start with Windows" description="Adds Blob Optimizers to your startup apps (for your account only).">
          <Switch label="Start with Windows" checked={settings.startWithWindows} disabled={saving} onChange={setAutostart} />
        </Row>
        <Row title="Start minimised" description="When started with Windows, open minimised to the taskbar.">
          <Switch label="Start minimised" checked={settings.startMinimized} disabled={saving || !settings.startWithWindows} onChange={(v) => set({ startMinimized: v })} />
        </Row>
        <Row
          title="Administrator rights"
          description={
            appInfo?.elevated
              ? "Running as administrator: operations that need it run without extra prompts."
              : "Running as a standard user: Windows asks for permission only when an operation needs it. Use 'Elevate' in the sidebar to restart as administrator."
          }
        >
          <ShieldCheck size={18} className={appInfo?.elevated ? "text-good" : "text-muted"} />
        </Row>
      </Card>

      <Card title="Optimization" icon={Gamepad2}>
        <Row title="Default gaming preset" description="Used by the Dashboard's 'Apply Gaming preset' quick action.">
          <select className={SELECT} value={settings.defaultPreset} disabled={saving} onChange={(e) => set({ defaultPreset: e.target.value as Settings["defaultPreset"] })}>
            <option value="balanced">Balanced</option>
            <option value="gaming">Gaming</option>
            <option value="maximum">Maximum Performance</option>
          </select>
        </Row>
        <Row title="Create a restore point before batch changes" description="Pre-selects the restore point option when removing apps, applying presets or applying recommended tweaks.">
          <Switch label="Restore point by default" checked={settings.restorePointDefault} disabled={saving} onChange={(v) => set({ restorePointDefault: v })} />
        </Row>
      </Card>

      <Card title="Notifications" icon={Bell}>
        <Row title="Show success messages" description="Show a message when an operation finishes successfully. Errors and warnings are always shown.">
          <Switch label="Show success messages" checked={settings.notifySuccess} disabled={saving} onChange={(v) => set({ notifySuccess: v })} />
        </Row>
      </Card>

      <Card title="Logging and backups" icon={Database}>
        <Row title="Operation history" description="How many operations (with their full output) are kept. Older entries and their logs are deleted automatically.">
          <select className={SELECT} value={settings.historyLimit} disabled={saving} onChange={(e) => set({ historyLimit: +e.target.value })}>
            {[100, 250, 500, 1000, 2500].map((n) => (
              <option key={n} value={n}>
                {n} entries
              </option>
            ))}
          </select>
        </Row>
        <Row title="Clear operation history" description="Deletes the list of past operations and their output. Backups and undo snapshots are kept.">
          <button className="btn-ghost px-3 py-1.5 text-xs" onClick={() => setConfirmClear(true)}>
            <Trash2 size={13} /> Clear
          </button>
        </Row>
        {paths && (
          <Row title="Data folder" description="History, logs, undo snapshots and registry backups.">
            <span className="selectable font-mono text-[11.5px] text-muted">{paths[0]}</span>
          </Row>
        )}
      </Card>

      <Card title="Language" icon={Globe}>
        <Row title="Language" description="Blob Optimizers is currently available in English.">
          <select className={SELECT} value={settings.language} disabled>
            <option value="en">English</option>
          </select>
        </Row>
      </Card>

      <UpdatesCard version={appInfo?.version} saving={saving} set={set} />

      <Card title="About" icon={Info}>
        <Row title="Blob Optimizers" description="Windows PC & gaming optimization suite. Every change is made through reviewed commands, shown before it runs and logged in the Terminal tab.">
          <span className="text-[13px] tabular-nums text-muted">Version {appInfo?.version ?? "…"}</span>
        </Row>
      </Card>

      <Modal
        open={confirmClear}
        title="Clear operation history?"
        onClose={() => setConfirmClear(false)}
        footer={
          <>
            <button className="btn-ghost" onClick={() => setConfirmClear(false)}>
              Cancel
            </button>
            <button
              className="btn-primary bg-bad"
              onClick={async () => {
                setConfirmClear(false);
                try {
                  await clearHistory();
                  toast("success", "History cleared");
                } catch (e) {
                  toast("error", "Could not clear history", errorMessage(e));
                }
              }}
            >
              Clear history
            </button>
          </>
        }
      >
        Past operations and their output logs are deleted. Nothing that was changed on your PC is undone.
      </Modal>
    </div>
  );
}
