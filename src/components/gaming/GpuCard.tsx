import { AlertTriangle, CheckCircle2, MonitorSmartphone } from "lucide-react";
import { useSystem } from "../../context/SystemContext";
import { formatBytes, shortName } from "../../lib/format";
import { primaryGpu } from "../../lib/hardware";
import { Card, InfoRow, Skeleton } from "../ui/Card";

/** Driver dates are YYYY-MM-DD. */
function ageDays(date: string | null) {
  if (!date) return null;
  const t = Date.parse(date);
  return Number.isNaN(t) ? null : Math.floor((Date.now() - t) / 86_400_000);
}

export function GpuCard() {
  const { hardware: hw, live } = useSystem();
  const gpu = primaryGpu(hw);
  if (!hw || !gpu) return <Skeleton className="h-64" />;

  const age = ageDays(gpu.driverDate);
  const tips: { ok: boolean; text: string }[] = [];
  if (age != null) {
    tips.push(
      age > 180
        ? { ok: false, text: `Your graphics driver is ${Math.round(age / 30)} months old. New drivers often include fixes and optimizations for recent games; update from ${gpu.manufacturer.split(" ")[0]}'s website or app.` }
        : { ok: true, text: `Graphics driver is recent (${gpu.driverDate}).` },
    );
  }
  if (hw.gpus.length > 1) {
    tips.push({
      ok: false,
      text: `This PC has ${hw.gpus.length} graphics processors. Use game profiles below to make sure games run on the ${shortName(gpu.name)} instead of the integrated graphics.`,
    });
  }
  if (hw.memory.modules.length === 1) tips.push({ ok: false, text: "Only one memory stick is installed. Two matched sticks (dual channel) noticeably help CPU-bound games." });
  if (hw.memory.modules.some((m) => m.configuredMhz && m.speedMhz && m.configuredMhz < m.speedMhz)) {
    tips.push({ ok: false, text: "Memory runs below its rated speed. Enabling XMP/EXPO in the BIOS lets it run at the advertised speed." });
  }
  if (hw.memory.totalBytes < 15 * 1024 ** 3) tips.push({ ok: false, text: "Less than 16 GB of RAM. Many current games recommend 16 GB." });

  return (
    <Card title="Graphics and hardware" icon={MonitorSmartphone} className="h-full">
      <div className="text-[13px]">
        <InfoRow label="GPU" value={shortName(gpu.name)} />
        <InfoRow label="VRAM" value={`${live?.gpuMemUsed != null ? `${formatBytes(live.gpuMemUsed)} used of ` : ""}${formatBytes(gpu.vramBytes, 0)}`} />
        <InfoRow label="Driver" value={`${gpu.driverVersion}${gpu.driverDate ? ` (${gpu.driverDate})` : ""}`} />
        <InfoRow label="CPU" value={shortName(hw.cpu.name)} />
        <InfoRow label="RAM" value={`${formatBytes(hw.memory.totalBytes, 0)} · ${hw.memory.modules.length} stick${hw.memory.modules.length === 1 ? "" : "s"}`} />
      </div>
      <ul className="mt-4 space-y-2">
        {tips.map((t) => (
          <li key={t.text} className="flex gap-2 text-xs leading-relaxed">
            {t.ok ? <CheckCircle2 size={14} className="mt-px shrink-0 text-good" /> : <AlertTriangle size={14} className="mt-px shrink-0 text-warn" />}
            <span className={t.ok ? "text-muted" : ""}>{t.text}</span>
          </li>
        ))}
      </ul>
    </Card>
  );
}
