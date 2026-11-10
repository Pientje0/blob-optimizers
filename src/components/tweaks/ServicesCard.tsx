import { Lock, Loader2, Play, RefreshCw, Server, Square, Undo2 } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useJob } from "../../hooks/useJob";
import { api, errorMessage, type ServiceInfo } from "../../lib/api";
import { Badge, Card, Skeleton } from "../ui/Card";
import { Chip, SearchInput } from "../ui/Controls";
import { Modal } from "../ui/Modal";

type Filter = "advice" | "running" | "all";
type Start = "automatic" | "manual" | "disabled";
const LABEL: Record<Start, string> = { automatic: "Automatic", manual: "Manual", disabled: "Disabled" };

export function ServicesCard() {
  const { run, busy } = useJob();
  const [services, setServices] = useState<ServiceInfo[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<Filter>("advice");
  const [pending, setPending] = useState<{ s: ServiceInfo; start: Start } | null>(null);

  const load = useCallback(async () => {
    try {
      setServices(await api.services());
      setError(null);
    } catch (e) {
      setError(errorMessage(e));
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const rows = useMemo(() => {
    const q = query.trim().toLowerCase();
    return (services ?? [])
      .filter((s) => (filter === "all" ? true : filter === "running" ? s.running : !!s.note || !!s.original))
      .filter((s) => !q || s.displayName.toLowerCase().includes(q) || s.name.toLowerCase().includes(q));
  }, [services, query, filter]);

  const current = (s: ServiceInfo): Start | null => (s.startMode.toLowerCase() as Start) in LABEL ? (s.startMode.toLowerCase() as Start) : null;

  const setStart = async (s: ServiceInfo, start: Start) => {
    setPending(null);
    await run(s.name, () => api.setServiceStart(s.name, start), { success: `${s.displayName}: ${LABEL[start]}`, failure: `Could not change ${s.displayName}` });
    load();
  };

  const control = async (s: ServiceInfo, start: boolean) => {
    await run(s.name, () => api.controlService(s.name, start), { success: `${s.displayName} ${start ? "started" : "stopped"}`, failure: `Could not ${start ? "start" : "stop"} ${s.displayName}` });
    load();
  };

  return (
    <Card
      title="Services"
      icon={Server}
      actions={
        <button className="flex items-center gap-1 text-xs text-muted hover:text-fg" onClick={load}>
          <RefreshCw size={12} /> Refresh
        </button>
      }
    >
      <div className="mb-3 flex flex-wrap items-center gap-2">
        <SearchInput value={query} onChange={setQuery} placeholder="Search services" className="w-60" />
        <Chip active={filter === "advice"} onClick={() => setFilter("advice")}>
          With guidance
        </Chip>
        <Chip active={filter === "running"} onClick={() => setFilter("running")}>
          Running
        </Chip>
        <Chip active={filter === "all"} onClick={() => setFilter("all")} count={services?.length}>
          All
        </Chip>
      </div>
      {error && <p className="text-xs text-bad">{error}</p>}
      {!services ? (
        <div className="space-y-3">
          {Array.from({ length: 6 }).map((_, i) => (
            <Skeleton key={i} className="h-10 w-full" />
          ))}
        </div>
      ) : (
        <div className="max-h-[560px] divide-y divide-line/40 overflow-y-auto rounded-xl border border-line/60">
          {rows.map((s) => {
            const cur = current(s);
            const suggestionDiffers = s.suggested && cur && s.suggested !== cur;
            return (
              <div key={s.name} className="flex items-start gap-3 px-4 py-3">
                <span className={`mt-1.5 h-2 w-2 shrink-0 rounded-full ${s.running ? "bg-good" : "bg-line"}`} title={s.running ? "Running" : "Stopped"} />
                <div className="min-w-0 flex-1">
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="text-[13px] font-medium">{s.displayName}</span>
                    <span className="font-mono text-[11px] text-muted">{s.name}</span>
                    {s.protected && (
                      <Badge tone="accent">
                        <Lock size={10} /> Protected
                      </Badge>
                    )}
                    {suggestionDiffers && <Badge tone="good">Suggested: {LABEL[s.suggested!]}</Badge>}
                  </div>
                  <div className="mt-0.5 line-clamp-2 text-xs text-muted">{s.note ?? s.description ?? ""}</div>
                  {s.original && cur !== s.original && (
                    <button className="mt-1 flex items-center gap-1 text-xs text-accent hover:underline" disabled={!!busy} onClick={() => setStart(s, s.original!)}>
                      <Undo2 size={11} /> Restore original ({LABEL[s.original]})
                    </button>
                  )}
                </div>
                {busy === s.name ? (
                  <Loader2 size={16} className="mt-1 animate-spin text-accent" />
                ) : s.protected || !cur ? (
                  <span className="mt-1 text-xs text-muted">{s.startMode}{s.delayed ? " (delayed)" : ""}</span>
                ) : (
                  <div className="flex items-center gap-1.5">
                    <button
                      className="rounded-lg p-1.5 text-muted hover:bg-raised hover:text-fg disabled:opacity-30"
                      title={s.running ? "Stop" : "Start"}
                      disabled={!!busy || (!s.running && cur === "disabled")}
                      onClick={() => control(s, !s.running)}
                    >
                      {s.running ? <Square size={13} /> : <Play size={13} />}
                    </button>
                    <select
                      value={cur}
                      disabled={!!busy}
                      aria-label={`Startup type for ${s.displayName}`}
                      onChange={(e) => setPending({ s, start: e.target.value as Start })}
                      className="rounded-lg border border-line bg-base/60 px-2 py-1 text-xs outline-none focus:border-accent/60"
                    >
                      {(Object.keys(LABEL) as Start[]).map((k) => (
                        <option key={k} value={k}>
                          {LABEL[k]}
                          {s.delayed && k === "automatic" ? " (delayed)" : ""}
                        </option>
                      ))}
                    </select>
                  </div>
                )}
              </div>
            );
          })}
          {rows.length === 0 && <p className="p-6 text-center text-sm text-muted">No services match.</p>}
        </div>
      )}
      <p className="mt-3 text-xs text-muted">
        Security, Windows Update, networking and sign-in services are protected. The original startup type of every service you change is remembered so
        you can restore it.
      </p>

      <Modal
        open={!!pending}
        title={`Set ${pending?.s.displayName} to ${pending ? LABEL[pending.start] : ""}?`}
        onClose={() => setPending(null)}
        footer={
          <>
            <button className="btn-ghost" onClick={() => setPending(null)}>
              Cancel
            </button>
            <button className="btn-primary" onClick={() => pending && setStart(pending.s, pending.start)}>
              Change
            </button>
          </>
        }
      >
        {pending && (
          <div className="space-y-2 text-[13px] leading-relaxed">
            <p>{pending.s.note ?? pending.s.description ?? "No description available."}</p>
            {pending.start === "disabled" && <p className="text-warn">A disabled service cannot be started by apps or Windows until you change it back. Features that depend on it stop working.</p>}
            {pending.start === "manual" && <p className="text-muted">Manual services start only when something needs them.</p>}
          </div>
        )}
      </Modal>
    </Card>
  );
}
