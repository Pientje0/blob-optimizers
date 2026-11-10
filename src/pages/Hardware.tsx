import {
  CircuitBoard,
  Cpu,
  HardDrive,
  Laptop,
  MemoryStick,
  Monitor,
  MonitorSmartphone,
  Network,
  RefreshCw,
  XCircle,
} from "lucide-react";
import { useSystem } from "../context/SystemContext";
import { Badge, Card, InfoRow, Skeleton } from "../components/ui/Card";
import { UsageBar } from "../components/ui/Gauge";
import { formatBytes, formatMhz, formatPercent } from "../lib/format";

function Loading() {
  return (
    <div className="space-y-3">
      {Array.from({ length: 5 }).map((_, i) => (
        <Skeleton key={i} className="h-4 w-full" />
      ))}
    </div>
  );
}

export function Hardware() {
  const { hardware: hw, hardwareLoading, hardwareError, refreshHardware, live } = useSystem();

  return (
    <div className="space-y-5">
      <div className="flex items-center justify-between">
        <p className="text-sm text-muted">
          {hw ? `Last detected ${new Date(hw.collectedAt).toLocaleString()}` : "Detecting hardware…"}
        </p>
        <button className="btn-ghost" onClick={refreshHardware} disabled={hardwareLoading}>
          <RefreshCw size={15} className={hardwareLoading ? "animate-spin" : ""} />
          {hardwareLoading ? "Detecting…" : "Refresh detection"}
        </button>
      </div>

      {hardwareError && (
        <div className="card flex items-center gap-3 border-bad/40 px-5 py-3 text-sm text-bad">
          <XCircle size={16} /> Hardware detection failed: {hardwareError}
        </div>
      )}

      <div className="grid grid-cols-1 gap-5 xl:grid-cols-2">
        <Card title="Processor" icon={Cpu}>
          {hw ? (
            <>
              <InfoRow label="Model" value={hw.cpu.name} />
              <InfoRow label="Manufacturer" value={hw.cpu.manufacturer.replace(/^Authentic|^Genuine/, "")} />
              <InfoRow label="Cores / threads" value={`${hw.cpu.cores} cores · ${hw.cpu.threads} threads`} />
              <InfoRow label="Base clock" value={formatMhz(hw.cpu.maxClockMhz)} />
              <InfoRow label="Current clock" value={formatMhz(live?.cpuFreqMhz)} />
              <InfoRow label="Usage" value={formatPercent(live?.cpuUsage, 1)} />
              <InfoRow
                label="Temperature"
                value={live?.cpuTempC != null ? `${live.cpuTempC.toFixed(1)} °C` : "Not exposed by this system's firmware"}
              />
              <InfoRow label="Cache" value={`L2 ${formatBytes(hw.cpu.l2CacheKb * 1024, 0)} · L3 ${formatBytes(hw.cpu.l3CacheKb * 1024, 0)}`} />
              <InfoRow label="Socket" value={hw.cpu.socket} />
              <InfoRow label="Virtualization" value={hw.cpu.virtualization ? "Enabled in firmware" : "Disabled"} />
              {live && (
                <div className="mt-4">
                  <div className="label mb-2">Per-thread usage</div>
                  <div className="grid grid-cols-6 gap-2 sm:grid-cols-8">
                    {live.perCore.map((v, i) => (
                      <div key={i} className="rounded-lg bg-raised/70 p-2 text-center">
                        <div className="text-[10px] text-muted">#{i}</div>
                        <div className="text-xs font-medium tabular-nums">{v.toFixed(0)}%</div>
                        <UsageBar value={v} className="mt-1" />
                      </div>
                    ))}
                  </div>
                </div>
              )}
            </>
          ) : (
            <Loading />
          )}
        </Card>

        <Card title="Graphics" icon={MonitorSmartphone}>
          {hw ? (
            <div className="space-y-5">
              {hw.gpus.map((g) => (
                <div key={g.name}>
                  <div className="mb-1 flex items-center gap-2">
                    <span className="font-medium">{g.name}</span>
                    {g.resolution && <Badge tone="accent">Active display</Badge>}
                  </div>
                  <InfoRow label="Manufacturer" value={g.manufacturer} />
                  <InfoRow label="VRAM" value={formatBytes(g.vramBytes, 0)} />
                  <InfoRow label="Driver" value={`${g.driverVersion}${g.driverDate ? ` (${g.driverDate})` : ""}`} />
                  {g.resolution && <InfoRow label="Output" value={`${g.resolution} @ ${g.refreshRate} Hz`} />}
                  <InfoRow label="Status" value={g.status} />
                </div>
              ))}
              <InfoRow label="Overall GPU usage" value={formatPercent(live?.gpuUsage, 0)} />
              <InfoRow label="Hardware-accelerated scheduling" value={hw.config.hagsEnabled == null ? "Not configured" : hw.config.hagsEnabled ? "On" : "Off"} />
              <p className="text-xs text-muted">
                GPU temperature is not exposed through standard Windows interfaces; it requires vendor tools (AMD Adrenalin / NVIDIA).
              </p>
            </div>
          ) : (
            <Loading />
          )}
        </Card>

        <Card title="Memory" icon={MemoryStick}>
          {hw ? (
            <>
              <InfoRow label="Installed" value={formatBytes(hw.memory.totalBytes, 1)} />
              <InfoRow label="In use" value={live ? `${formatBytes(live.memUsed)} (${formatPercent((live.memUsed / live.memTotal) * 100)})` : "…"} />
              <InfoRow label="Modules" value={`${hw.memory.modules.length}`} />
              <table className="mt-3 w-full text-[13px]">
                <thead>
                  <tr className="text-left text-[11px] uppercase tracking-wider text-muted">
                    <th className="pb-2 font-semibold">Slot</th>
                    <th className="pb-2 font-semibold">Size</th>
                    <th className="pb-2 font-semibold">Speed</th>
                    <th className="pb-2 font-semibold">Part number</th>
                  </tr>
                </thead>
                <tbody className="selectable">
                  {hw.memory.modules.map((m) => (
                    <tr key={m.slot} className="border-t border-line/40">
                      <td className="py-2">{m.slot}</td>
                      <td className="py-2">{formatBytes(m.capacityBytes, 0)}</td>
                      <td className="py-2 tabular-nums">
                        {m.configuredMhz || m.speedMhz} MT/s
                        {m.speedMhz && m.configuredMhz && m.configuredMhz < m.speedMhz ? (
                          <span className="ml-1 text-xs text-warn">(rated {m.speedMhz})</span>
                        ) : null}
                      </td>
                      <td className="py-2 font-mono text-xs text-muted">{m.partNumber || "—"}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </>
          ) : (
            <Loading />
          )}
        </Card>

        <Card title="Storage" icon={HardDrive}>
          {hw ? (
            <div className="space-y-5">
              {hw.disks.map((d) => (
                <div key={d.number}>
                  <div className="mb-1 flex flex-wrap items-center gap-2">
                    <span className="font-medium">{d.model}</span>
                    <Badge tone="accent">{d.mediaType || "Unknown"}</Badge>
                    <Badge>{d.busType}</Badge>
                    <Badge tone={d.health === "Healthy" ? "good" : "bad"}>{d.health || "Unknown health"}</Badge>
                  </div>
                  <InfoRow label="Capacity" value={formatBytes(d.sizeBytes, 0)} />
                  {d.temperature != null && <InfoRow label="Temperature" value={`${d.temperature} °C`} />}
                  {d.wearPercent != null && <InfoRow label="Wear" value={`${d.wearPercent}%`} />}
                  {d.volumes.map((v) => {
                    const used = v.sizeBytes - v.freeBytes;
                    return (
                      <div key={v.letter} className="mt-2 rounded-xl bg-raised/60 p-3 text-xs">
                        <div className="mb-1.5 flex justify-between">
                          <span className="font-medium">
                            {v.letter} {v.label && <span className="text-muted">({v.label})</span>}{" "}
                            <span className="text-muted">{v.fileSystem}</span>
                          </span>
                          <span className="tabular-nums text-muted">
                            {formatBytes(v.freeBytes)} free of {formatBytes(v.sizeBytes)}
                          </span>
                        </div>
                        <UsageBar value={(used / v.sizeBytes) * 100} />
                      </div>
                    );
                  })}
                </div>
              ))}
              <p className="text-xs text-muted">Temperature and wear appear when the drive reports them to Windows (often requires administrator rights).</p>
            </div>
          ) : (
            <Loading />
          )}
        </Card>

        <Card title="Motherboard & firmware" icon={CircuitBoard}>
          {hw ? (
            <>
              <InfoRow label="Board" value={hw.board.product} />
              <InfoRow label="Manufacturer" value={hw.board.manufacturer} />
              <InfoRow label="System model" value={hw.board.systemModel} />
              <InfoRow label="BIOS" value={`${hw.board.biosVersion}${hw.board.biosDate ? ` (${hw.board.biosDate})` : ""}`} />
              <InfoRow label="BIOS vendor" value={hw.board.biosVendor} />
              <InfoRow label="Firmware" value={hw.board.firmware ?? "—"} />
            </>
          ) : (
            <Loading />
          )}
        </Card>

        <Card title="Windows" icon={Laptop}>
          {hw ? (
            <>
              <InfoRow label="Edition" value={hw.windows.productName} />
              <InfoRow label="Version" value={hw.windows.displayVersion} />
              <InfoRow label="Build" value={hw.windows.build} mono />
              <InfoRow label="Architecture" value={hw.windows.architecture} />
              <InfoRow label="Computer name" value={hw.windows.computerName} />
              <InfoRow label="Installed" value={hw.windows.installDate ?? "—"} />
              <InfoRow label="Last boot" value={hw.windows.lastBoot ? new Date(hw.windows.lastBoot).toLocaleString() : "—"} />
              <InfoRow label="Power plan" value={hw.config.powerPlan?.name ?? "—"} />
              <InfoRow label="Game Mode" value={hw.config.gameModeEnabled ? "On" : "Off"} />
            </>
          ) : (
            <Loading />
          )}
        </Card>

        <Card title="Network adapters" icon={Network}>
          {hw ? (
            <div className="space-y-4">
              {hw.network.map((n) => (
                <div key={n.name}>
                  <div className="mb-1 flex items-center gap-2">
                    <span className="font-medium">{n.name}</span>
                    <Badge tone={n.status === "Up" ? "good" : "neutral"}>{n.status}</Badge>
                  </div>
                  <InfoRow label="Adapter" value={n.description} />
                  {n.status === "Up" && <InfoRow label="Link speed" value={n.linkSpeed} />}
                  {n.ipv4 && <InfoRow label="IPv4" value={n.ipv4} mono />}
                  <InfoRow label="MAC" value={n.mac || "—"} mono />
                </div>
              ))}
            </div>
          ) : (
            <Loading />
          )}
        </Card>

        <Card title="Monitors" icon={Monitor}>
          {hw ? (
            hw.monitors.length ? (
              <div className="space-y-4">
                {hw.monitors.map((m, i) => (
                  <div key={`${m.serial}-${i}`}>
                    <div className="mb-1 font-medium">{m.name || `Display ${i + 1}`}</div>
                    <InfoRow label="Manufacturer code" value={m.manufacturer ?? "—"} />
                    {m.sizeInches != null && <InfoRow label="Diagonal" value={`${m.sizeInches}"`} />}
                    {m.year ? <InfoRow label="Manufactured" value={`${m.year}`} /> : null}
                  </div>
                ))}
              </div>
            ) : (
              <p className="text-sm text-muted">No monitor information reported.</p>
            )
          ) : (
            <Loading />
          )}
        </Card>
      </div>
    </div>
  );
}
