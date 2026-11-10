import { AlertTriangle, Folder, Info, Loader2, Package, RefreshCw, RotateCcw, Settings2, Trash2, XCircle } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { Badge, Skeleton } from "../components/ui/Card";
import { Chip, SearchInput, Switch } from "../components/ui/Controls";
import { Modal } from "../components/ui/Modal";
import { useJob } from "../hooks/useJob";
import { api, errorMessage, type StartupEntry, type StartupList } from "../lib/api";
import { formatTime } from "../lib/format";

type Filter = "all" | "enabled" | "disabled";

const ADVICE = {
  safeToDisable: { label: "Safe to disable", tone: "good" },
  caution: { label: "Caution", tone: "warn" },
  keep: { label: "Recommended on", tone: "accent" },
} as const;

const SOURCE_ICON = {
  registry: <Settings2 size={14} className="shrink-0 text-accent" />,
  folder: <Folder size={14} className="shrink-0 text-accent" />,
  store: <Package size={14} className="shrink-0 text-accent2" />,
};

function impact(ms: number | null) {
  if (ms == null) return null;
  const tone = ms > 3000 ? "bad" : ms > 1000 ? "warn" : "good";
  return <Badge tone={tone}>{(ms / 1000).toFixed(1)} s at boot</Badge>;
}

export function Startup() {
  const { run, busy } = useJob();
  const [data, setData] = useState<StartupList | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<Filter>("all");
  const [confirm, setConfirm] = useState<{ e: StartupEntry; action: "disable" | "remove" } | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      setData(await api.startup());
      setError(null);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const rows = useMemo(() => {
    const q = query.trim().toLowerCase();
    return (data?.entries ?? [])
      .filter((e) => filter === "all" || (filter === "enabled" ? e.enabled : !e.enabled))
      .filter((e) => !q || e.name.toLowerCase().includes(q) || (e.publisher ?? "").toLowerCase().includes(q) || e.command.toLowerCase().includes(q))
      .sort((a, b) => Number(b.enabled) - Number(a.enabled) || (b.impactMs ?? 0) - (a.impactMs ?? 0) || a.name.localeCompare(b.name));
  }, [data, query, filter]);

  const setEnabled = async (e: StartupEntry, enabled: boolean) => {
    setConfirm(null);
    await run(e.id, () => api.setStartupEnabled(e.id, enabled), {
      success: `${e.name} ${enabled ? "will start" : "won't start"} with Windows`,
      failure: `Could not change ${e.name}`,
    });
    load();
  };

  const remove = async (e: StartupEntry) => {
    setConfirm(null);
    await run(e.id, () => api.removeStartup(e.id), { success: `Removed ${e.name} from startup`, failure: `Could not remove ${e.name}` });
    load();
  };

  const enabledCount = data?.entries.filter((e) => e.enabled).length ?? 0;

  return (
    <div className="space-y-5">
      <div className="flex flex-wrap items-center gap-2">
        <SearchInput value={query} onChange={setQuery} placeholder="Search startup apps" className="w-64" />
        <Chip active={filter === "all"} onClick={() => setFilter("all")} count={data?.entries.length}>
          All
        </Chip>
        <Chip active={filter === "enabled"} onClick={() => setFilter("enabled")} count={data ? enabledCount : undefined}>
          Enabled
        </Chip>
        <Chip active={filter === "disabled"} onClick={() => setFilter("disabled")} count={data ? data.entries.length - enabledCount : undefined}>
          Disabled
        </Chip>
        <span className="flex-1" />
        <button className="btn-ghost px-3 py-1.5 text-xs" onClick={load} disabled={loading}>
          <RefreshCw size={13} className={loading ? "animate-spin" : ""} /> Refresh
        </button>
      </div>

      {error && (
        <div className="flex items-center gap-2 rounded-xl border border-bad/30 bg-bad/10 px-4 py-3 text-sm text-bad">
          <XCircle size={15} /> Could not read startup entries: {error}
        </div>
      )}
      {data && !data.impactAvailable && (
        <p className="flex items-center gap-2 text-xs text-muted">
          <Info size={13} /> Startup delays are shown when Windows has measured them; reading those measurements needs administrator rights.
        </p>
      )}

      <div className="card divide-y divide-line/40 overflow-hidden">
        {!data
          ? Array.from({ length: 6 }).map((_, i) => (
              <div key={i} className="p-4">
                <Skeleton className="h-10 w-full" />
              </div>
            ))
          : rows.map((e) => (
              <div key={e.id} className={`flex items-start gap-4 px-5 py-3.5 ${e.enabled ? "" : "opacity-70"}`}>
                <div className="min-w-0 flex-1">
                  <div className="flex flex-wrap items-center gap-2">
                    {SOURCE_ICON[e.source]}
                    <span className="font-medium">{e.name}</span>
                    <Badge tone={ADVICE[e.advice].tone}>{ADVICE[e.advice].label}</Badge>
                    {impact(e.impactMs)}
                    {e.scope === "machine" && <Badge>All users</Badge>}
                    {e.policy && <Badge>Set by policy</Badge>}
                  </div>
                  <div className="mt-0.5 text-xs text-muted">
                    {e.publisher ?? "Unknown publisher"} · {e.source === "store" ? "Store app" : e.location}
                  </div>
                  <div className="selectable mt-1 truncate font-mono text-[11.5px] text-muted" title={e.command}>
                    {e.command}
                  </div>
                  <div className="mt-1 text-xs leading-relaxed text-muted">{e.note}</div>
                </div>
                <div className="flex items-center gap-2 pt-1">
                  {e.removable && (
                    <button className="rounded-lg p-1.5 text-muted hover:bg-raised hover:text-bad disabled:opacity-30" title="Remove from startup" disabled={!!busy} onClick={() => setConfirm({ e, action: "remove" })}>
                      <Trash2 size={14} />
                    </button>
                  )}
                  {busy === e.id ? (
                    <Loader2 size={18} className="mx-2.5 animate-spin text-accent" />
                  ) : (
                    <Switch
                      label={`Start ${e.name} with Windows`}
                      checked={e.enabled}
                      disabled={!!busy || e.policy}
                      onChange={(v) => (!v && e.advice !== "safeToDisable" ? setConfirm({ e, action: "disable" }) : setEnabled(e, v))}
                    />
                  )}
                </div>
              </div>
            ))}
        {data && rows.length === 0 && <p className="p-8 text-center text-sm text-muted">No startup entries match.</p>}
      </div>

      {data && data.removed.length > 0 && (
        <section className="card p-5">
          <div className="mb-3 text-[13.5px] font-semibold">Removed startup entries</div>
          <ul className="space-y-1.5">
            {data.removed.map((r) => (
              <li key={`${r.location}|${r.name}`} className="flex items-center gap-3 rounded-xl bg-raised/60 px-3 py-2 text-[13px]">
                <span className="flex-1">
                  {r.name} <span className="text-xs text-muted">· {r.location} · removed {formatTime(r.removedAt)}</span>
                </span>
                <button
                  className="btn-ghost px-2.5 py-1 text-xs"
                  disabled={!!busy}
                  onClick={async () => {
                    await run(`restore-${r.name}`, () => api.restoreStartup(r.name, r.location), { success: `Restored ${r.name}`, failure: `Could not restore ${r.name}` });
                    load();
                  }}
                >
                  <RotateCcw size={12} /> Restore
                </button>
              </li>
            ))}
          </ul>
        </section>
      )}
      <p className="text-xs text-muted">
        Disabling works like Task Manager: the entry stays in place and can be turned back on at any time. Removed registry entries can be restored above;
        removed Startup-folder shortcuts go to the Recycle Bin. Scheduled tasks and services that start with Windows are managed under Windows Tweaks → System.
      </p>

      <Modal
        open={!!confirm}
        title={confirm?.action === "remove" ? `Remove ${confirm.e.name} from startup?` : `Disable ${confirm?.e.name}?`}
        onClose={() => setConfirm(null)}
        footer={
          <>
            <button className="btn-ghost" onClick={() => setConfirm(null)}>
              Cancel
            </button>
            <button
              className={confirm?.action === "remove" ? "btn-primary bg-bad" : "btn-primary"}
              onClick={() => confirm && (confirm.action === "remove" ? remove(confirm.e) : setEnabled(confirm.e, false))}
            >
              {confirm?.action === "remove" ? "Remove" : "Disable"}
            </button>
          </>
        }
      >
        {confirm && (
          <div className="space-y-3 text-[13px] leading-relaxed">
            <p className="flex gap-2">
              <AlertTriangle size={16} className="mt-0.5 shrink-0 text-warn" /> {confirm.e.note}
            </p>
            {confirm.action === "remove" && (
              <p className="text-muted">
                {confirm.e.source === "folder"
                  ? "The shortcut is moved to the Recycle Bin."
                  : "The entry is deleted from the registry and saved so it can be restored from this page. Disabling is usually the better choice."}
              </p>
            )}
          </div>
        )}
      </Modal>
    </div>
  );
}
