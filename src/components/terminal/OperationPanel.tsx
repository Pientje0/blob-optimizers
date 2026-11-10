import { ChevronLeft, Clock, Play, Search, ShieldAlert, ShieldCheck, TriangleAlert, Wrench } from "lucide-react";
import { useMemo, useState } from "react";
import { useEngine } from "../../context/EngineContext";
import { useSystem } from "../../context/SystemContext";
import { useToast } from "../../context/ToastContext";
import { errorMessage, type Operation } from "../../lib/api";
import { Badge } from "../ui/Card";
import { Modal } from "../ui/Modal";

const RISK_TONE = { safe: "good", caution: "warn", high: "bad" } as const;

function OperationBadges({ op }: { op: Operation }) {
  return (
    <div className="flex flex-wrap gap-1.5">
      <Badge tone={RISK_TONE[op.risk]}>{op.modifiesSystem ? "Changes system" : "Read-only"}</Badge>
      {op.requiresAdmin && (
        <Badge tone="accent">
          <ShieldCheck size={11} /> Admin
        </Badge>
      )}
      {!op.cancellable && <Badge tone="warn">Not cancellable</Badge>}
      <Badge>
        <Clock size={11} /> {op.durationHint}
      </Badge>
    </div>
  );
}

export function OperationPanel({ onStarted }: { onStarted: (id: string) => void }) {
  const { catalog, runOperation } = useEngine();
  const { appInfo } = useSystem();
  const toast = useToast();
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<Operation | null>(null);
  const [params, setParams] = useState<Record<string, string>>({});
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);

  const groups = useMemo(() => {
    const q = query.trim().toLowerCase();
    const filtered = catalog.filter(
      (o) => !q || o.title.toLowerCase().includes(q) || o.description.toLowerCase().includes(q) || o.script.toLowerCase().includes(q),
    );
    const map = new Map<string, Operation[]>();
    filtered.forEach((o) => map.set(o.category, [...(map.get(o.category) ?? []), o]));
    return [...map.entries()];
  }, [catalog, query]);

  const select = (op: Operation) => {
    setSelected(op);
    setParams(Object.fromEntries(op.params.map((p) => [p.name, p.default])));
  };

  const preview = selected
    ? selected.params.reduce((s, p) => s.split(`{${p.name}}`).join(params[p.name] || p.default), selected.script)
    : "";

  const start = async () => {
    if (!selected) return;
    setBusy(true);
    try {
      const entry = await runOperation(selected.id, params);
      onStarted(entry.id);
      setConfirming(false);
    } catch (e) {
      toast("error", `Could not start "${selected.title}"`, errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const needsUac = selected?.requiresAdmin && !appInfo?.elevated;

  return (
    <section className="card flex min-h-0 flex-col overflow-hidden">
      {!selected ? (
        <>
          <header className="border-b border-line/60 p-4">
            <div className="mb-3 flex items-center gap-2 text-[13.5px] font-semibold">
              <Wrench size={16} className="text-accent" /> Operations
            </div>
            <div className="relative">
              <Search size={14} className="absolute left-3 top-1/2 -translate-y-1/2 text-muted" />
              <input
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                placeholder="Search operations"
                className="w-full rounded-xl border border-line bg-base/60 py-2 pl-8 pr-3 text-sm outline-none placeholder:text-muted focus:border-accent/60"
              />
            </div>
          </header>
          <div className="min-h-0 flex-1 overflow-y-auto p-2">
            {groups.map(([cat, ops]) => (
              <div key={cat} className="mb-2">
                <div className="label px-2.5 py-1.5">{cat}</div>
                {ops.map((op) => (
                  <button
                    key={op.id}
                    onClick={() => select(op)}
                    className="flex w-full items-start gap-3 rounded-xl px-2.5 py-2 text-left transition-colors hover:bg-raised/70"
                  >
                    <div className="min-w-0 flex-1">
                      <div className="flex items-center gap-1.5 text-[13.5px] font-medium">
                        {op.title}
                        {op.requiresAdmin && <ShieldCheck size={12} className="text-accent" />}
                      </div>
                      <div className="truncate text-xs text-muted">{op.description}</div>
                    </div>
                    {op.modifiesSystem && <span className="mt-1.5 h-1.5 w-1.5 shrink-0 rounded-full bg-warn" title="Changes system" />}
                  </button>
                ))}
              </div>
            ))}
            {groups.length === 0 && <p className="p-4 text-center text-sm text-muted">No operations match.</p>}
          </div>
        </>
      ) : (
        <div className="flex min-h-0 flex-1 flex-col overflow-y-auto p-4">
          <button className="mb-3 flex items-center gap-1 self-start text-xs text-muted hover:text-fg" onClick={() => setSelected(null)}>
            <ChevronLeft size={14} /> All operations
          </button>
          <h3 className="text-[15px] font-semibold">{selected.title}</h3>
          <p className="mt-1 text-[13px] leading-relaxed text-muted">{selected.description}</p>
          <div className="mt-3">
            <OperationBadges op={selected} />
          </div>

          <div className="label mt-5">What it changes</div>
          <p className="mt-1 text-[13px] leading-relaxed">{selected.effects}</p>

          {selected.params.length > 0 && (
            <div className="mt-5 space-y-3">
              {selected.params.map((p) => (
                <label key={p.name} className="block">
                  <span className="label">{p.label}</span>
                  {p.kind.type === "choice" ? (
                    <select
                      value={params[p.name]}
                      onChange={(e) => setParams({ ...params, [p.name]: e.target.value })}
                      className="mt-1 w-full rounded-xl border border-line bg-base/60 px-3 py-2 text-sm outline-none focus:border-accent/60"
                    >
                      {p.kind.options.map((o) => (
                        <option key={o}>{o}</option>
                      ))}
                    </select>
                  ) : (
                    <input
                      value={params[p.name] ?? ""}
                      type={p.kind.type === "int" ? "number" : "text"}
                      min={p.kind.type === "int" ? p.kind.min : undefined}
                      max={p.kind.type === "int" ? p.kind.max : undefined}
                      onChange={(e) => setParams({ ...params, [p.name]: e.target.value })}
                      className="mt-1 w-full rounded-xl border border-line bg-base/60 px-3 py-2 font-mono text-sm outline-none focus:border-accent/60"
                    />
                  )}
                </label>
              ))}
            </div>
          )}

          <div className="label mt-5">Command</div>
          <pre className="selectable mt-1 whitespace-pre-wrap break-all rounded-xl border border-line bg-base/70 px-3 py-2.5 font-mono text-[12px]">
            <span className="text-muted">{selected.shell === "cmd" ? "CMD> " : "PS> "}</span>
            {preview}
          </pre>

          {needsUac && (
            <p className="mt-3 flex gap-2 text-xs text-muted">
              <ShieldAlert size={14} className="shrink-0 text-warn" /> Windows will ask for administrator permission when this starts.
            </p>
          )}

          <div className="mt-auto pt-5">
            <button
              className="btn-primary w-full"
              disabled={busy}
              onClick={() => (selected.modifiesSystem ? setConfirming(true) : start())}
            >
              <Play size={15} /> Run
            </button>
          </div>
        </div>
      )}

      <Modal
        open={confirming && !!selected}
        title={`Run "${selected?.title}"?`}
        onClose={() => setConfirming(false)}
        footer={
          <>
            <button className="btn-ghost" onClick={() => setConfirming(false)}>
              Cancel
            </button>
            <button className="btn-primary" disabled={busy} onClick={start}>
              <Play size={15} /> Run now
            </button>
          </>
        }
      >
        {selected && (
          <div className="space-y-4">
            <div className="flex gap-3 rounded-xl border border-warn/30 bg-warn/10 p-3 text-[13px] leading-relaxed">
              <TriangleAlert size={17} className="mt-0.5 shrink-0 text-warn" />
              <span>{selected.effects}</span>
            </div>
            <div>
              <div className="label mb-1">Command</div>
              <pre className="whitespace-pre-wrap break-all rounded-xl border border-line bg-base/70 px-3 py-2.5 font-mono text-[12px]">{preview}</pre>
            </div>
            <OperationBadges op={selected} />
          </div>
        )}
      </Modal>
    </section>
  );
}
