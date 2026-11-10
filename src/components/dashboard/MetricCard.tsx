import type { LucideIcon } from "lucide-react";
import type { ReactNode } from "react";
import { Gauge } from "../ui/Gauge";
import { Skeleton } from "../ui/Card";

interface MetricCardProps {
  icon: LucideIcon;
  title: string;
  subtitle?: string | null;
  value: number | null;
  gaugeLabel?: string;
  stats: { label: string; value: ReactNode }[];
  chart?: ReactNode;
  loading?: boolean;
}

export function MetricCard({ icon: Icon, title, subtitle, value, gaugeLabel, stats, chart, loading }: MetricCardProps) {
  return (
    <section className="card animate-fade-up flex flex-col overflow-hidden">
      <div className="flex items-start gap-4 p-5 pb-3">
        <Gauge value={loading ? null : value} label={gaugeLabel} />
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2 text-muted">
            <Icon size={15} className="text-accent" />
            <span className="label">{title}</span>
          </div>
          {subtitle === undefined ? null : subtitle ? (
            <div className="mt-1 truncate text-[13.5px] font-medium" title={subtitle}>
              {subtitle}
            </div>
          ) : (
            <Skeleton className="mt-2 h-4 w-32" />
          )}
          <dl className="mt-2.5 grid grid-cols-2 gap-x-3 gap-y-1.5 text-xs">
            {stats.map((s) => (
              <div key={s.label} className="min-w-0">
                <dt className="text-muted">{s.label}</dt>
                <dd className="truncate font-medium tabular-nums">{s.value}</dd>
              </div>
            ))}
          </dl>
        </div>
      </div>
      {chart && <div className="mt-auto">{chart}</div>}
    </section>
  );
}
