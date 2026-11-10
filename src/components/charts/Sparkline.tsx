import { Area, AreaChart, ResponsiveContainer, YAxis } from "recharts";
import type { HistoryPoint } from "../../context/SystemContext";

interface SparklineProps {
  data: HistoryPoint[];
  dataKey: keyof HistoryPoint;
  color?: string;
  height?: number;
  /** Fixed 0-100 domain for percentages; omit for auto-scaling values like network rates. */
  percent?: boolean;
}

export function Sparkline({ data, dataKey, color = "rgb(var(--c-accent))", height = 44, percent = true }: SparklineProps) {
  const id = `spark-${String(dataKey)}`;
  return (
    <div style={{ height }} className="w-full">
      <ResponsiveContainer width="100%" height="100%">
        <AreaChart data={data} margin={{ top: 2, right: 0, bottom: 0, left: 0 }}>
          <defs>
            <linearGradient id={id} x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stopColor={color} stopOpacity={0.35} />
              <stop offset="100%" stopColor={color} stopOpacity={0} />
            </linearGradient>
          </defs>
          <YAxis hide domain={percent ? [0, 100] : [0, "auto"]} />
          <Area
            type="monotone"
            dataKey={dataKey}
            stroke={color}
            strokeWidth={1.75}
            fill={`url(#${id})`}
            isAnimationActive={false}
            connectNulls
          />
        </AreaChart>
      </ResponsiveContainer>
    </div>
  );
}
