import { Activity, Gauge as GaugeIcon, Info, Loader2, Play, ShieldCheck, Square } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { CartesianGrid, Legend, Line, LineChart, ResponsiveContainer, Tooltip, YAxis } from "recharts";
import { useSystem } from "../../context/SystemContext";
import { useToast } from "../../context/ToastContext";
import { api, errorMessage, type FpsSample } from "../../lib/api";
import { Card } from "../ui/Card";

interface Point {
  t: number;
  fps: number | null;
  low: number | null;
  cpu: number;
  gpu: number | null;
  maxCore: number;
}

const MAX_POINTS = 300;

/** Plain-language reading of what limits performance, from measured load only. */
function bottleneck(points: Point[]): string | null {
  const recent = points.slice(-20).filter((p) => p.fps != null);
  if (recent.length < 10) return null;
  const avg = (f: (p: Point) => number) => recent.reduce((s, p) => s + f(p), 0) / recent.length;
  const gpu = avg((p) => p.gpu ?? 0);
  const core = avg((p) => p.maxCore);
  if (gpu >= 93) return "The graphics card is fully busy: performance is GPU-limited. Lower resolution or graphics settings (or enable upscaling) for more FPS.";
  if (core >= 90 && gpu < 80) return "One CPU core is maxed while the GPU has headroom: performance is CPU-limited. Close background apps; lowering graphics settings won't help much.";
  if (gpu < 60 && core < 70) return "Neither CPU nor GPU is fully used. The game may be frame-capped (V-Sync, FPS limit) or waiting on something else.";
  return null;
}

export function FpsMonitor() {
  const { live } = useSystem();
  const toast = useToast();
  const [running, setRunning] = useState(false);
  const [starting, setStarting] = useState(false);
  const [sample, setSample] = useState<FpsSample | null>(null);
  const [target, setTarget] = useState<number | null>(null);
  const [points, setPoints] = useState<Point[]>([]);
  const liveRef = useRef(live);
  liveRef.current = live;

  const start = async (elevate: boolean) => {
    setStarting(true);
    try {
      await api.fpsStart(elevate);
      setRunning(true);
      setPoints([]);
      setSample(null);
    } catch (e) {
      toast("error", "Could not start the FPS monitor", errorMessage(e));
    } finally {
      setStarting(false);
    }
  };

  const stop = async () => {
    await api.fpsStop().catch(() => undefined);
    setRunning(false);
  };

  // Stop the helper when leaving the page.
  useEffect(() => () => void api.fpsStop().catch(() => undefined), []);

  useEffect(() => {
    if (!running) return;
    const t = window.setInterval(async () => {
      try {
        const [isRunning, s] = await api.fpsPoll();
        if (!isRunning) return setRunning(false);
        if (!s || s.t === sample?.t) return;
        setSample(s);
        if (s.error) return;
        const proc = s.processes.find((p) => p.pid === target) ?? s.processes[0];
        const l = liveRef.current;
        setPoints((pts) =>
          [...pts, { t: s.t, fps: proc ? Math.round(proc.fps) : null, low: proc ? Math.round(proc.low1Fps) : null, cpu: l?.cpuUsage ?? 0, gpu: l?.gpuUsage ?? null, maxCore: Math.max(0, ...(l?.perCore ?? [0])) }].slice(-MAX_POINTS),
        );
      } catch {
        /* transient */
      }
    }, 1000);
    return () => window.clearInterval(t);
  }, [running, target, sample?.t]);

  const current = sample?.processes.find((p) => p.pid === target) ?? sample?.processes[0];
  const stats = useMemo(() => {
    const f = points.map((p) => p.fps).filter((x): x is number => x != null);
    const l = points.map((p) => p.low).filter((x): x is number => x != null);
    if (!f.length) return null;
    return { avg: Math.round(f.reduce((a, b) => a + b, 0) / f.length), min: Math.min(...f), low: Math.round(l.reduce((a, b) => a + b, 0) / Math.max(1, l.length)), seconds: f.length };
  }, [points]);
  const hint = bottleneck(points);

  return (
    <Card
      title="Performance monitor"
      icon={Activity}
      actions={
        running ? (
          <button className="btn-ghost px-3 py-1.5 text-xs" onClick={stop}>
            <Square size={13} /> Stop
          </button>
        ) : (
          <button className="btn-primary px-3 py-1.5 text-xs" disabled={starting} onClick={() => start(false)}>
            {starting ? <Loader2 size={13} className="animate-spin" /> : <Play size={13} />} Start measuring
          </button>
        )
      }
    >
      {sample?.error ? (
        <div className="space-y-3 rounded-xl border border-warn/30 bg-warn/10 p-4 text-[13px]">
          <p>{sample.error}</p>
          {sample.needsAdmin && (
            <button className="btn-primary px-3 py-1.5 text-xs" onClick={() => start(true)}>
              <ShieldCheck size={13} /> Measure with administrator rights
            </button>
          )}
        </div>
      ) : !running && !points.length ? (
        <div className="flex gap-3 rounded-xl bg-raised/60 p-4 text-[13px] leading-relaxed text-muted">
          <Info size={16} className="mt-0.5 shrink-0 text-accent" />
          <span>
            Start measuring, then play. Blob Optimizers counts the frames each game presents (from Windows' own graphics events, like PresentMon) and records
            CPU and GPU load alongside, so you can compare settings and presets with real numbers. DirectX 9-12 games are measured; games using only Vulkan or
            OpenGL may not appear.
          </span>
        </div>
      ) : (
        <div className="space-y-4">
          <div className="flex flex-wrap items-end gap-6">
            <div>
              <div className="label">{current ? current.name.replace(/\.exe$/i, "") : "Waiting for a game…"}</div>
              <div className="text-4xl font-semibold tabular-nums">
                {current ? Math.round(current.fps) : "—"}
                <span className="ml-1 text-sm font-normal text-muted">FPS</span>
              </div>
            </div>
            <div className="text-[13px]">
              <div className="text-muted">Frame time</div>
              <div className="font-medium tabular-nums">{current ? `${current.frameMs.toFixed(1)} ms` : "—"}</div>
            </div>
            <div className="text-[13px]">
              <div className="text-muted">1% low</div>
              <div className="font-medium tabular-nums">{current ? `${Math.round(current.low1Fps)} FPS` : "—"}</div>
            </div>
            {stats && (
              <div className="text-[13px]">
                <div className="text-muted">Session ({stats.seconds}s)</div>
                <div className="font-medium tabular-nums">
                  avg {stats.avg} · min {stats.min} · 1% low {stats.low}
                </div>
              </div>
            )}
            <span className="flex-1" />
            {running && <span className="flex items-center gap-1.5 text-xs text-good"><span className="h-2 w-2 animate-pulse rounded-full bg-good" /> Measuring</span>}
          </div>

          {sample && sample.processes.length > 1 && (
            <div className="flex flex-wrap gap-1.5">
              {sample.processes.slice(0, 6).map((p) => (
                <button
                  key={p.pid}
                  onClick={() => setTarget(p.pid)}
                  className={`rounded-full border px-2.5 py-1 text-xs ${p.pid === current?.pid ? "border-accent/50 bg-accent/10" : "border-line text-muted hover:text-fg"}`}
                >
                  {p.name.replace(/\.exe$/i, "")} · {Math.round(p.fps)}
                </button>
              ))}
            </div>
          )}

          <div className="h-56 w-full">
            <ResponsiveContainer width="100%" height="100%">
              <LineChart data={points} margin={{ top: 6, right: 6, bottom: 0, left: -10 }}>
                <CartesianGrid stroke="rgb(var(--c-line))" strokeOpacity={0.5} vertical={false} />
                <YAxis yAxisId="fps" tick={{ fill: "rgb(var(--c-muted))", fontSize: 10 }} axisLine={false} tickLine={false} />
                <YAxis yAxisId="pct" orientation="right" domain={[0, 100]} tick={{ fill: "rgb(var(--c-muted))", fontSize: 10 }} axisLine={false} tickLine={false} unit="%" />
                <Tooltip
                  contentStyle={{ background: "rgb(var(--c-raised))", border: "1px solid rgb(var(--c-line))", borderRadius: 10, fontSize: 12 }}
                  labelFormatter={() => ""}
                  formatter={(v: number, name: string) => [name.includes("%") ? `${Math.round(v)}%` : Math.round(v), name.replace(" %", "")]}
                />
                <Legend wrapperStyle={{ fontSize: 11 }} />
                <Line yAxisId="fps" dataKey="fps" name="FPS" stroke="rgb(var(--c-accent))" strokeWidth={2} dot={false} isAnimationActive={false} connectNulls />
                <Line yAxisId="fps" dataKey="low" name="1% low" stroke="rgb(var(--c-warn))" strokeWidth={1.25} dot={false} isAnimationActive={false} connectNulls />
                <Line yAxisId="pct" dataKey="gpu" name="GPU %" stroke="rgb(var(--c-accent2))" strokeWidth={1.25} strokeOpacity={0.7} dot={false} isAnimationActive={false} />
                <Line yAxisId="pct" dataKey="maxCore" name="Busiest CPU core %" stroke="rgb(var(--c-good))" strokeWidth={1.25} strokeOpacity={0.7} dot={false} isAnimationActive={false} />
              </LineChart>
            </ResponsiveContainer>
          </div>
          {hint && (
            <p className="flex gap-2 text-[13px] leading-relaxed">
              <GaugeIcon size={15} className="mt-0.5 shrink-0 text-accent" /> {hint}
            </p>
          )}
        </div>
      )}
    </Card>
  );
}
