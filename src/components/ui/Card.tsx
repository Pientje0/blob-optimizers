import type { LucideIcon } from "lucide-react";
import type { ReactNode } from "react";

interface CardProps {
  title?: ReactNode;
  icon?: LucideIcon;
  actions?: ReactNode;
  className?: string;
  bodyClassName?: string;
  children: ReactNode;
}

export function Card({ title, icon: Icon, actions, className = "", bodyClassName = "", children }: CardProps) {
  return (
    <section className={`card animate-fade-up flex flex-col ${className}`}>
      {(title || actions) && (
        <header className="flex items-center gap-2.5 px-5 pb-1 pt-4">
          {Icon && <Icon size={16} className="text-accent" />}
          <h2 className="flex-1 text-[13.5px] font-semibold tracking-tight">{title}</h2>
          {actions}
        </header>
      )}
      <div className={`flex-1 px-5 pb-5 pt-3 ${bodyClassName}`}>{children}</div>
    </section>
  );
}

export function InfoRow({ label, value, mono }: { label: string; value: ReactNode; mono?: boolean }) {
  return (
    <div className="flex items-baseline justify-between gap-4 border-b border-line/40 py-2 last:border-0">
      <span className="shrink-0 text-muted">{label}</span>
      <span className={`selectable truncate text-right ${mono ? "font-mono text-[12.5px]" : ""}`} title={typeof value === "string" ? value : undefined}>
        {value ?? "—"}
      </span>
    </div>
  );
}

export function Skeleton({ className = "" }: { className?: string }) {
  return <div className={`animate-pulse rounded-lg bg-raised ${className}`} />;
}

type Tone = "good" | "warn" | "bad" | "neutral" | "accent";

const TONES: Record<Tone, string> = {
  good: "bg-good/10 text-good border-good/25",
  warn: "bg-warn/10 text-warn border-warn/25",
  bad: "bg-bad/10 text-bad border-bad/25",
  neutral: "bg-raised text-muted border-line",
  accent: "bg-accent/10 text-accent border-accent/30",
};

export function Badge({ tone = "neutral", children }: { tone?: Tone; children: ReactNode }) {
  return (
    <span className={`inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-[11px] font-medium ${TONES[tone]}`}>
      {children}
    </span>
  );
}
