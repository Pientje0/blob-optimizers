import { Area, AreaChart, CartesianGrid, ResponsiveContainer, Tooltip, YAxis } from "recharts";
import type { HistoryPoint } from "../../context/SystemContext";
import { formatRate } from "../../lib/format";

export function NetworkChart({ data, height = 150 }: { data: HistoryPoint[]; height?: number }) {
  return (
    <div style={{ height }} className="w-full">
      <ResponsiveContainer width="100%" height="100%">
        <AreaChart data={data} margin={{ top: 6, right: 4, bottom: 0, left: 0 }}>
          <defs>
            <linearGradient id="net-rx" x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stopColor="rgb(var(--c-accent2))" stopOpacity={0.35} />
              <stop offset="100%" stopColor="rgb(var(--c-accent2))" stopOpacity={0} />
            </linearGradient>
            <linearGradient id="net-tx" x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stopColor="rgb(var(--c-accent))" stopOpacity={0.3} />
              <stop offset="100%" stopColor="rgb(var(--c-accent))" stopOpacity={0} />
            </linearGradient>
          </defs>
          <CartesianGrid stroke="rgb(var(--c-line))" strokeOpacity={0.5} vertical={false} />
          <YAxis
            width={64}
            tickFormatter={(v: number) => formatRate(v)}
            tick={{ fill: "rgb(var(--c-muted))", fontSize: 10 }}
            axisLine={false}
            tickLine={false}
          />
          <Tooltip
            cursor={{ stroke: "rgb(var(--c-line))" }}
            contentStyle={{
              background: "rgb(var(--c-raised))",
              border: "1px solid rgb(var(--c-line))",
              borderRadius: 10,
              fontSize: 12,
            }}
            labelFormatter={() => ""}
            formatter={(v: number, name: string) => [formatRate(v), name === "rx" ? "Download" : "Upload"]}
          />
          <Area type="monotone" dataKey="rx" stroke="rgb(var(--c-accent2))" strokeWidth={1.75} fill="url(#net-rx)" isAnimationActive={false} />
          <Area type="monotone" dataKey="tx" stroke="rgb(var(--c-accent))" strokeWidth={1.75} fill="url(#net-tx)" isAnimationActive={false} />
        </AreaChart>
      </ResponsiveContainer>
    </div>
  );
}
