import type { LucideIcon } from "lucide-react";
import type { StatusSummary } from "../../lib/insights";
import { Skeleton } from "../ui/Card";

const TONE = {
  good: { text: "text-good", ring: "from-good/30", dot: "bg-good" },
  warn: { text: "text-warn", ring: "from-warn/30", dot: "bg-warn" },
  bad: { text: "text-bad", ring: "from-bad/30", dot: "bg-bad" },
};

export function StatusCard({ icon: Icon, title, status }: { icon: LucideIcon; title: string; status: StatusSummary | null }) {
  const tone = status ? TONE[status.level] : null;
  return (
    <section className="card animate-fade-up relative overflow-hidden p-5">
      {tone && <div className={`pointer-events-none absolute -right-10 -top-10 h-32 w-32 rounded-full bg-gradient-to-br ${tone.ring} to-transparent blur-2xl`} />}
      <div className="flex items-center gap-2 text-muted">
        <Icon size={15} className="text-accent" />
        <span className="label">{title}</span>
      </div>
      {status && tone ? (
        <div className="mt-3 flex items-end justify-between">
          <div>
            <div className={`flex items-center gap-2 text-[17px] font-semibold ${tone.text}`}>
              <span className={`h-2 w-2 rounded-full ${tone.dot}`} />
              {status.headline}
            </div>
            <div className="mt-1 text-xs text-muted">Based on measured system state</div>
          </div>
          <div className="text-right">
            <div className="text-3xl font-semibold tabular-nums">{status.score}</div>
            <div className="text-[11px] text-muted">/ 100</div>
          </div>
        </div>
      ) : (
        <div className="mt-3 space-y-2">
          <Skeleton className="h-5 w-40" />
          <Skeleton className="h-3 w-28" />
        </div>
      )}
    </section>
  );
}
