import { Search, Trash2 } from "lucide-react";
import { useMemo, useState } from "react";
import { useEngine } from "../../context/EngineContext";
import { useToast } from "../../context/ToastContext";
import { errorMessage, type HistoryEntry } from "../../lib/api";
import { formatDuration, formatTime } from "../../lib/format";
import { irreversibleReason } from "../../lib/reversibility";
import { Badge } from "../ui/Card";
import { Modal } from "../ui/Modal";
import { JobStatusBadge, jobDuration } from "./JobStatus";

type Filter = "all" | "changes" | "failed" | "typed";

const FILTERS: { id: Filter; label: string; test: (e: HistoryEntry) => boolean }[] = [
  { id: "all", label: "All", test: () => true },
  { id: "changes", label: "System changes", test: (e) => e.modifiesSystem },
  { id: "failed", label: "Problems", test: (e) => e.status !== "succeeded" },
  { id: "typed", label: "Typed commands", test: (e) => e.source === "user" },
];

export function HistoryPanel({ onOpen }: { onOpen: (e: HistoryEntry) => void }) {
  const { history, clearHistory } = useEngine();
  const toast = useToast();
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<Filter>("all");
  const [confirmClear, setConfirmClear] = useState(false);

  const rows = useMemo(() => {
    const q = query.trim().toLowerCase();
    const f = FILTERS.find((x) => x.id === filter)!;
    return history.filter((e) => f.test(e) && (!q || e.title.toLowerCase().includes(q) || e.command.toLowerCase().includes(q)));
  }, [history, query, filter]);

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex flex-wrap items-center gap-2 border-b border-line/60 px-4 py-3">
        <div className="relative">
          <Search size={14} className="absolute left-3 top-1/2 -translate-y-1/2 text-muted" />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search history"
            className="w-56 rounded-xl border border-line bg-base/60 py-1.5 pl-8 pr-3 text-[13px] outline-none placeholder:text-muted focus:border-accent/60"
          />
        </div>
        {FILTERS.map((f) => (
          <button
            key={f.id}
            onClick={() => setFilter(f.id)}
            className={`rounded-full border px-3 py-1 text-xs transition-colors ${
              filter === f.id ? "border-accent/50 bg-accent/10 text-fg" : "border-line text-muted hover:text-fg"
            }`}
          >
            {f.label}
          </button>
        ))}
        <span className="flex-1" />
        <button className="btn-ghost px-2.5 py-1.5 text-xs" disabled={!history.length} onClick={() => setConfirmClear(true)}>
          <Trash2 size={13} /> Clear history
        </button>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto">
        {rows.length === 0 ? (
          <p className="p-8 text-center text-sm text-muted">{history.length ? "Nothing matches this filter." : "No operations have been run yet."}</p>
        ) : (
          <table className="w-full text-[13px]">
            <thead className="sticky top-0 bg-surface">
              <tr className="text-left text-[11px] uppercase tracking-wider text-muted">
                <th className="px-4 py-2 font-semibold">Operation</th>
                <th className="px-2 py-2 font-semibold">Started</th>
                <th className="px-2 py-2 font-semibold">Duration</th>
                <th className="px-4 py-2 text-right font-semibold">Result</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((e) => (
                <tr key={e.id} onClick={() => onOpen(e)} className="cursor-pointer border-t border-line/40 hover:bg-raised/50">
                  <td className="max-w-0 px-4 py-2.5">
                    <div className="flex items-center gap-2">
                      <span className="truncate font-medium">{e.title}</span>
                      {e.source === "user" && <Badge>typed</Badge>}
                      {e.modifiesSystem && <Badge tone="warn">change</Badge>}
                      {irreversibleReason(e.operationId) && e.status === "succeeded" && (
                        <span title={irreversibleReason(e.operationId)!}>
                          <Badge tone="bad">not reversible</Badge>
                        </span>
                      )}
                      {e.elevated && <Badge tone="accent">admin</Badge>}
                    </div>
                    <div className="truncate font-mono text-[11.5px] text-muted">{e.command}</div>
                  </td>
                  <td className="whitespace-nowrap px-2 py-2.5 text-muted">{formatTime(e.startedAt)}</td>
                  <td className="whitespace-nowrap px-2 py-2.5 tabular-nums text-muted">{formatDuration(jobDuration(e))}</td>
                  <td className="px-4 py-2.5 text-right">
                    <JobStatusBadge status={e.status} />
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

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
                try {
                  await clearHistory();
                  setConfirmClear(false);
                } catch (e) {
                  toast("error", "Could not clear history", errorMessage(e));
                }
              }}
            >
              <Trash2 size={14} /> Clear
            </button>
          </>
        }
      >
        This deletes the list of past operations and their saved output logs. It does not undo or change any operation that was
        run.
      </Modal>
    </div>
  );
}
