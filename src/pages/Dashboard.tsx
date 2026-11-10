import {
  Activity,
  AlertTriangle,
  ArrowDown,
  ArrowUp,
  CheckCircle2,
  ChevronRight,
  Cpu,
  Gamepad2,
  HardDrive,
  History,
  Lightbulb,
  MemoryStick,
  MonitorSmartphone,
  Network,
  XCircle,
  Zap,
} from "lucide-react";
import { useMemo } from "react";
import { useSystem } from "../context/SystemContext";
import { useEngine } from "../context/EngineContext";
import { useSettings } from "../context/SettingsContext";
import { setIntent } from "../lib/intent";
import { JobStatusBadge } from "../components/terminal/JobStatus";
import { MetricCard } from "../components/dashboard/MetricCard";
import { StatusCard } from "../components/dashboard/StatusCard";
import { Sparkline } from "../components/charts/Sparkline";
import { NetworkChart } from "../components/charts/NetworkChart";
import { Badge, Card, InfoRow, Skeleton } from "../components/ui/Card";
import { UsageBar } from "../components/ui/Gauge";
import { formatBytes, formatMhz, formatPercent, formatRate, formatTime, formatUptime, shortName } from "../lib/format";
import { gamingStatus, pcStatus, recommendations, type Level } from "../lib/insights";
import { memorySpeed, primaryGpu } from "../lib/hardware";
import { findNav, type PageId } from "../navigation";

const PRESET_NAME = { balanced: "Balanced", gaming: "Gaming", maximum: "Maximum Performance" } as const;

const LEVEL_ICON: Record<Level, JSX.Element> = {
  bad: <XCircle size={16} className="text-bad" />,
  warn: <AlertTriangle size={16} className="text-warn" />,
  good: <CheckCircle2 size={16} className="text-good" />,
};

export function Dashboard({ onNavigate }: { onNavigate: (id: PageId) => void }) {
  const { live, history, hardware: hw, liveError, displays } = useSystem();
  const { settings } = useSettings();
  const gpu = primaryGpu(hw);
  const recs = useMemo(() => recommendations(hw, live, displays), [hw, live, displays]);
  const status = useMemo(() => (live || hw ? pcStatus(hw, live) : null), [hw, live]);
  const gaming = useMemo(() => gamingStatus(hw, displays), [hw, displays]);
  const mainDisplay = displays?.find((d) => d.primary);
  const { running, history: operations } = useEngine();
  const recent = useMemo(() => [...running, ...operations].slice(0, 6), [running, operations]);

  const ramPct = live ? (live.memUsed / live.memTotal) * 100 : null;
  const systemDrive = live?.disks.find((d) => d.mount.toUpperCase() === "C:") ?? live?.disks[0];
  const diskPct = systemDrive ? ((systemDrive.total - systemDrive.available) / systemDrive.total) * 100 : null;

  return (
    <div className="space-y-5">
      {liveError && (
        <div className="card flex items-center gap-3 border-bad/40 px-5 py-3 text-sm text-bad">
          <XCircle size={16} /> Live monitoring failed: {liveError}
        </div>
      )}

      <div className="grid grid-cols-1 gap-5 lg:grid-cols-[1.4fr_1fr_1fr]">
        <section className="card glow-accent animate-fade-up flex flex-col justify-between p-5">
          <div>
            <div className="label">This PC</div>
            {hw ? (
              <>
                <div className="mt-2 text-xl font-semibold tracking-tight">{hw.windows.computerName}</div>
                <div className="mt-0.5 text-sm text-muted">
                  {hw.windows.productName} {hw.windows.displayVersion} · build {hw.windows.build}
                </div>
              </>
            ) : (
              <div className="mt-2 space-y-2">
                <Skeleton className="h-6 w-48" />
                <Skeleton className="h-4 w-64" />
              </div>
            )}
          </div>
          <div className="mt-4 flex flex-wrap gap-2">
            <Badge tone="accent">Uptime {live ? formatUptime(live.uptimeSecs) : "…"}</Badge>
            <Badge>{live ? `${live.processCount} processes` : "…"}</Badge>
            {hw?.config.powerPlan && <Badge>{hw.config.powerPlan.name} power plan</Badge>}
          </div>
        </section>
        <StatusCard icon={Activity} title="Overall PC status" status={status} />
        <StatusCard icon={Gamepad2} title="Gaming readiness" status={gaming} />
      </div>

      <div className="grid grid-cols-1 gap-5 md:grid-cols-2 2xl:grid-cols-4">
        <MetricCard
          icon={Cpu}
          title="Processor"
          subtitle={hw ? shortName(hw.cpu.name) : null}
          value={live?.cpuUsage ?? null}
          loading={!live}
          stats={[
            { label: "Clock", value: formatMhz(live?.cpuFreqMhz) },
            { label: "Temperature", value: live?.cpuTempC != null ? `${live.cpuTempC.toFixed(0)} °C` : "Not exposed" },
            { label: "Cores / threads", value: hw ? `${hw.cpu.cores} / ${hw.cpu.threads}` : "…" },
            { label: "Socket", value: hw?.cpu.socket || "…" },
          ]}
          chart={<Sparkline data={history} dataKey="cpu" />}
        />
        <MetricCard
          icon={MonitorSmartphone}
          title="Graphics"
          subtitle={gpu ? shortName(gpu.name) : hw ? "No GPU detected" : null}
          value={live?.gpuUsage ?? null}
          loading={!live}
          stats={[
            {
              label: "VRAM used",
              value: live?.gpuMemUsed != null ? `${formatBytes(live.gpuMemUsed)} / ${formatBytes(gpu?.vramBytes)}` : formatBytes(gpu?.vramBytes),
            },
            { label: "Temperature", value: "Needs vendor tools" },
            { label: "Driver", value: gpu?.driverVersion || "…" },
            { label: "Main display", value: mainDisplay ? `${mainDisplay.hz} Hz` : "…" },
          ]}
          chart={<Sparkline data={history} dataKey="gpu" color="rgb(var(--c-accent2))" />}
        />
        <MetricCard
          icon={MemoryStick}
          title="Memory"
          subtitle={hw ? `${formatBytes(hw.memory.totalBytes, 0)} · ${hw.memory.modules.length} module${hw.memory.modules.length === 1 ? "" : "s"}` : null}
          value={ramPct}
          loading={!live}
          stats={[
            { label: "In use", value: live ? formatBytes(live.memUsed) : "…" },
            { label: "Available", value: live ? formatBytes(live.memTotal - live.memUsed) : "…" },
            { label: "Speed", value: memorySpeed(hw) ? `${memorySpeed(hw)} MT/s` : "…" },
            { label: "Page file", value: live ? `${formatBytes(live.swapUsed)} / ${formatBytes(live.swapTotal)}` : "…" },
          ]}
          chart={<Sparkline data={history} dataKey="ram" color="rgb(var(--c-good))" />}
        />
        <MetricCard
          icon={HardDrive}
          title="Storage"
          subtitle={systemDrive ? `System drive ${systemDrive.mount}` : null}
          value={diskPct}
          gaugeLabel="used"
          loading={!live}
          stats={[
            { label: "Free", value: systemDrive ? formatBytes(systemDrive.available) : "…" },
            { label: "Capacity", value: systemDrive ? formatBytes(systemDrive.total) : "…" },
            { label: "Type", value: hw?.disks[0] ? `${hw.disks[0].mediaType} · ${hw.disks[0].busType}` : "…" },
            { label: "Health", value: hw?.disks[0]?.health || "…" },
          ]}
          chart={
            <div className="space-y-2 px-5 pb-5">
              {live?.disks.map((d) => (
                <div key={d.mount} className="text-xs">
                  <div className="mb-1 flex justify-between text-muted">
                    <span>{d.mount}</span>
                    <span className="tabular-nums">
                      {formatBytes(d.total - d.available)} / {formatBytes(d.total)}
                    </span>
                  </div>
                  <UsageBar value={((d.total - d.available) / d.total) * 100} />
                </div>
              ))}
            </div>
          }
        />
      </div>

      <div className="grid grid-cols-1 gap-5 xl:grid-cols-[1.6fr_1fr]">
        <Card
          title="Network activity"
          icon={Network}
          actions={
            <div className="flex gap-4 text-xs tabular-nums">
              <span className="flex items-center gap-1 text-accent2">
                <ArrowDown size={13} /> {live ? formatRate(live.netRxPerSec) : "…"}
              </span>
              <span className="flex items-center gap-1 text-accent">
                <ArrowUp size={13} /> {live ? formatRate(live.netTxPerSec) : "…"}
              </span>
            </div>
          }
        >
          <NetworkChart data={history} />
          {hw && (
            <div className="mt-3 flex flex-wrap gap-2">
              {hw.network
                .filter((n) => n.status === "Up")
                .map((n) => (
                  <Badge key={n.name} tone="good">
                    {n.name} · {n.linkSpeed}
                    {n.ipv4 ? ` · ${n.ipv4}` : ""}
                  </Badge>
                ))}
            </div>
          )}
        </Card>

        <Card
          title="System"
          icon={Cpu}
          actions={
            <button className="flex items-center gap-1 text-xs text-muted hover:text-fg" onClick={() => onNavigate("hardware")}>
              Details <ChevronRight size={13} />
            </button>
          }
        >
          {hw ? (
            <div className="text-[13px]">
              <InfoRow label="CPU" value={shortName(hw.cpu.name)} />
              <InfoRow label="GPU" value={gpu ? shortName(gpu.name) : "—"} />
              <InfoRow label="RAM" value={`${formatBytes(hw.memory.totalBytes, 0)} @ ${memorySpeed(hw) ?? "?"} MT/s`} />
              <InfoRow label="Motherboard" value={hw.board.product || hw.board.systemModel} />
              <InfoRow
                label="Storage"
                value={hw.disks.map((d) => `${d.model} (${formatBytes(d.sizeBytes, 0)})`).join(", ")}
              />
              <InfoRow label="Windows" value={`${hw.windows.productName} ${hw.windows.displayVersion}`} />
            </div>
          ) : (
            <div className="space-y-3">
              {Array.from({ length: 6 }).map((_, i) => (
                <Skeleton key={i} className="h-4 w-full" />
              ))}
            </div>
          )}
        </Card>
      </div>

      <div className="grid grid-cols-1 gap-5 xl:grid-cols-[1.6fr_1fr]">
        <Card title="Recommendations" icon={Lightbulb} actions={<Badge>{recs.length} found</Badge>}>
          {recs.length === 0 && (live || hw) ? (
            <div className="flex items-center gap-3 rounded-xl bg-good/10 px-4 py-3 text-sm text-good">
              <CheckCircle2 size={16} /> No issues detected right now.
            </div>
          ) : (
            <ul className="space-y-2">
              {recs.map((r) => (
                <li key={r.id} className="flex items-start gap-3 rounded-xl bg-raised/60 px-4 py-3">
                  <span className="mt-0.5">{LEVEL_ICON[r.level]}</span>
                  <div className="min-w-0 flex-1">
                    <div className="text-[13.5px] font-medium">{r.title}</div>
                    <div className="mt-0.5 text-xs leading-relaxed text-muted">{r.detail}</div>
                  </div>
                  {r.target && (
                    <button
                      className="btn-ghost shrink-0 px-2.5 py-1.5 text-xs"
                      onClick={() => onNavigate(r.target!)}
                      title={findNav(r.target).description}
                    >
                      {findNav(r.target).label} <ChevronRight size={13} />
                    </button>
                  )}
                </li>
              ))}
            </ul>
          )}
        </Card>

        <Card title="Top background processes" icon={Activity}>
          {live ? (
            <table className="w-full text-[13px]">
              <thead>
                <tr className="text-left text-[11px] uppercase tracking-wider text-muted">
                  <th className="pb-2 font-semibold">Process</th>
                  <th className="pb-2 text-right font-semibold">CPU</th>
                  <th className="pb-2 text-right font-semibold">Memory</th>
                </tr>
              </thead>
              <tbody>
                {live.topProcesses.map((p) => (
                  <tr key={p.name} className="border-t border-line/40">
                    <td className="max-w-0 truncate py-2 pr-2" title={p.name}>
                      {p.name.replace(/\.exe$/i, "")}
                      {p.instances > 1 && <span className="ml-1.5 text-[11px] text-muted">×{p.instances}</span>}
                    </td>
                    <td className="py-2 text-right tabular-nums">{formatPercent(p.cpu, 1)}</td>
                    <td className="py-2 text-right tabular-nums text-muted">{formatBytes(p.memory)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          ) : (
            <div className="space-y-3">
              {Array.from({ length: 6 }).map((_, i) => (
                <Skeleton key={i} className="h-4 w-full" />
              ))}
            </div>
          )}
        </Card>
      </div>

      <div className="grid grid-cols-1 gap-5 xl:grid-cols-[1.6fr_1fr]">
        <Card title="Quick optimizations" icon={Zap}>
          <p className="mb-3 text-xs text-muted">
            Shortcuts to the most useful actions. Every change is previewed and confirmed before it runs through the command engine.
          </p>
          <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
            {[
              {
                label: `Apply ${PRESET_NAME[settings.defaultPreset] ?? "Gaming"} preset`,
                go: () => {
                  setIntent({ kind: "preview-preset", preset: settings.defaultPreset });
                  onNavigate("gaming");
                },
              },
              { label: "Create restore point", go: () => onNavigate("restore") },
              { label: "Review startup apps", go: () => onNavigate("startup") },
              { label: "Remove bloatware", go: () => onNavigate("debloat") },
            ].map((a) => (
              <button key={a.label} className="btn-ghost justify-between" onClick={a.go}>
                {a.label}
                <ChevronRight size={14} className="text-muted" />
              </button>
            ))}
          </div>
        </Card>

        <Card
          title="Recent operations"
          icon={History}
          actions={
            <button className="flex items-center gap-1 text-xs text-muted hover:text-fg" onClick={() => onNavigate("terminal")}>
              All <ChevronRight size={13} />
            </button>
          }
        >
          {recent.length === 0 ? (
            <div className="flex h-full flex-col items-center justify-center rounded-xl border border-dashed border-line py-6 text-center text-xs text-muted">
              No operations yet.
              <span className="mt-1">Changes made through Blob Optimizers will be listed here.</span>
            </div>
          ) : (
            <ul className="space-y-1">
              {recent.map((e) => (
                <li key={e.id} className="flex items-center gap-3 rounded-lg px-1 py-1.5 text-[13px]">
                  <span className="min-w-0 flex-1 truncate" title={e.command}>
                    {e.title}
                  </span>
                  <span className="shrink-0 text-[11px] text-muted">{formatTime(e.startedAt)}</span>
                  <JobStatusBadge status={e.status} />
                </li>
              ))}
            </ul>
          )}
        </Card>
      </div>
    </div>
  );
}
