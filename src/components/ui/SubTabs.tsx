import type { LucideIcon } from "lucide-react";

export interface SubTab<T extends string> {
  id: T;
  label: string;
  icon?: LucideIcon;
  count?: number;
}

/** Section tabs inside a page (Gaming, Network, Tweaks). Wraps on narrow windows. */
export function SubTabs<T extends string>({ value, onChange, tabs }: { value: T; onChange: (v: T) => void; tabs: SubTab<T>[] }) {
  return (
    <div role="tablist" className="flex flex-wrap gap-1 rounded-xl border border-line bg-surface p-1">
      {tabs.map((t) => {
        const Icon = t.icon;
        const active = value === t.id;
        return (
          <button
            key={t.id}
            role="tab"
            aria-selected={active}
            onClick={() => onChange(t.id)}
            className={`flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-[13px] font-medium transition-colors ${
              active ? "bg-raised text-fg shadow-sm" : "text-muted hover:bg-raised/50 hover:text-fg"
            }`}
          >
            {Icon && <Icon size={14} className={active ? "text-accent" : undefined} />}
            {t.label}
            {t.count != null && <span className="text-[11px] tabular-nums text-muted">{t.count}</span>}
          </button>
        );
      })}
    </div>
  );
}
