import { Download, ExternalLink, Loader2, RefreshCw } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { Card } from "../ui/Card";
import { Switch } from "../ui/Controls";
import { useSettings, type Settings } from "../../context/SettingsContext";
import { useToast } from "../../context/ToastContext";
import { api, errorMessage, type UpdateInfo } from "../../lib/api";

const REPO_RE = /^[A-Za-z0-9-]{1,100}\/[A-Za-z0-9._-]{1,100}$/;

function Row({ title, description, children }: { title: string; description?: ReactNode; children: ReactNode }) {
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

/** Update preferences plus a manual "check now" with the result and release notes. */
export function UpdatesCard({ version, saving, set }: { version?: string; saving: boolean; set: (p: Partial<Settings>) => Promise<void> }) {
  const { settings } = useSettings();
  const toast = useToast();
  const [repo, setRepo] = useState(settings.updateRepo);
  const [checking, setChecking] = useState(false);
  const [result, setResult] = useState<UpdateInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => setRepo(settings.updateRepo), [settings.updateRepo]);

  const repoValid = repo === "" || REPO_RE.test(repo);
  const repoDirty = repo !== settings.updateRepo;

  const check = async () => {
    setChecking(true);
    setError(null);
    setResult(null);
    try {
      setResult(await api.checkUpdates());
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setChecking(false);
    }
  };

  const open = async (url: string) => {
    try {
      await api.openRelease(url);
    } catch (e) {
      toast("error", "Could not open the release page", errorMessage(e));
    }
  };

  return (
    <Card title="Updates" icon={Download}>
      <Row title="Installed version">
        <span className="text-[13px] tabular-nums text-muted">{version ?? "…"}</span>
      </Row>
      <Row
        title="Release repository"
        description="GitHub repository that publishes Blob Optimizers releases, as owner/name. Only the public 'latest release' information is read; nothing is downloaded or installed automatically."
      >
        <div className="flex items-center gap-2">
          <input
            value={repo}
            onChange={(e) => setRepo(e.target.value.trim())}
            spellCheck={false}
            placeholder="owner/blob-optimizers"
            aria-label="Release repository"
            aria-invalid={!repoValid}
            className={`w-56 rounded-lg border bg-base/60 px-2.5 py-1.5 font-mono text-[12.5px] outline-none placeholder:font-sans placeholder:text-muted focus:border-accent/60 ${repoValid ? "border-line" : "border-bad"}`}
          />
          {repoDirty && (
            <button className="btn-primary px-3 py-1.5 text-xs" disabled={!repoValid || saving} onClick={() => set({ updateRepo: repo })}>
              Save
            </button>
          )}
        </div>
      </Row>
      <Row title="Check for updates at startup" description="Checks once, a few seconds after Blob Optimizers opens, and shows a message when a newer version exists.">
        <Switch label="Check for updates at startup" checked={settings.checkUpdates} disabled={saving} onChange={(v) => set({ checkUpdates: v })} />
      </Row>
      <Row
        title="Check now"
        description={
          error ? (
            <span className="text-bad">{error}</span>
          ) : result ? (
            result.available ? (
              <span className="text-good">Version {result.latest} is available (you have {result.current}).</span>
            ) : (
              <>You have the latest version ({result.current}).</>
            )
          ) : !settings.updateRepo ? (
            "Set the release repository above to enable update checks."
          ) : undefined
        }
      >
        <div className="flex gap-2">
          {result?.available && (
            <button className="btn-primary px-3 py-1.5 text-xs" onClick={() => open(result.url)}>
              <ExternalLink size={13} /> Open release page
            </button>
          )}
          <button className="btn-ghost px-3 py-1.5 text-xs" disabled={checking || !settings.updateRepo || repoDirty} onClick={check}>
            {checking ? <Loader2 size={13} className="animate-spin" /> : <RefreshCw size={13} />} Check
          </button>
        </div>
      </Row>
      {result?.available && result.notes && (
        <div className="mt-2 rounded-xl border border-line bg-base/50 p-3">
          <div className="mb-1 text-xs font-medium">{result.name || `Version ${result.latest}`}</div>
          <pre className="selectable max-h-48 overflow-y-auto whitespace-pre-wrap font-sans text-xs leading-relaxed text-muted">{result.notes}</pre>
        </div>
      )}
      <p className="pt-3 text-xs leading-relaxed text-muted">
        To update, run the new installer over this one, or run the one-line install command again if you installed with it. Settings, history, undo snapshots and backups are kept.
      </p>
    </Card>
  );
}
