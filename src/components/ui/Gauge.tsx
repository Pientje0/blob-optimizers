interface GaugeProps {
  /** 0-100, or null when unavailable. */
  value: number | null;
  size?: number;
  stroke?: number;
  label?: string;
}

/** Radial usage gauge whose colour shifts from accent to warning/critical as load rises. */
export function Gauge({ value, size = 92, stroke = 8, label }: GaugeProps) {
  const r = (size - stroke) / 2;
  const c = 2 * Math.PI * r;
  const v = value == null ? 0 : Math.max(0, Math.min(100, value));
  const color = v >= 90 ? "rgb(var(--c-bad))" : v >= 75 ? "rgb(var(--c-warn))" : "url(#gauge-g)";

  return (
    <div className="relative shrink-0" style={{ width: size, height: size }}>
      <svg width={size} height={size} className="-rotate-90">
        <defs>
          <linearGradient id="gauge-g" x1="0" y1="0" x2="1" y2="1">
            <stop offset="0" stopColor="rgb(var(--c-accent))" />
            <stop offset="1" stopColor="rgb(var(--c-accent2))" />
          </linearGradient>
        </defs>
        <circle cx={size / 2} cy={size / 2} r={r} fill="none" stroke="rgb(var(--c-raised))" strokeWidth={stroke} />
        <circle
          cx={size / 2}
          cy={size / 2}
          r={r}
          fill="none"
          stroke={color}
          strokeWidth={stroke}
          strokeLinecap="round"
          strokeDasharray={c}
          strokeDashoffset={c * (1 - v / 100)}
          style={{ transition: "stroke-dashoffset 600ms cubic-bezier(.2,.8,.2,1)" }}
        />
      </svg>
      <div className="absolute inset-0 flex flex-col items-center justify-center">
        <span className="text-lg font-semibold tabular-nums">{value == null ? "—" : `${Math.round(v)}%`}</span>
        {label && <span className="text-[10.5px] text-muted">{label}</span>}
      </div>
    </div>
  );
}

/** Thin horizontal usage bar. */
export function UsageBar({ value, className = "" }: { value: number; className?: string }) {
  const v = Math.max(0, Math.min(100, value));
  const color = v >= 90 ? "bg-bad" : v >= 75 ? "bg-warn" : "bg-gradient-to-r from-accent to-accent2";
  return (
    <div className={`h-1.5 w-full overflow-hidden rounded-full bg-raised ${className}`}>
      <div className={`h-full rounded-full ${color}`} style={{ width: `${v}%`, transition: "width 600ms cubic-bezier(.2,.8,.2,1)" }} />
    </div>
  );
}
