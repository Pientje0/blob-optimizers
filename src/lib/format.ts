export function formatBytes(bytes: number | null | undefined, digits = 1): string {
  if (bytes == null || !isFinite(bytes)) return "—";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let i = 0;
  let v = bytes;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v.toFixed(i === 0 ? 0 : digits)} ${units[i]}`;
}

export function formatRate(bytesPerSec: number): string {
  // Network speeds are conventionally shown in bits.
  const bits = bytesPerSec * 8;
  if (bits < 1000) return `${bits.toFixed(0)} bps`;
  if (bits < 1e6) return `${(bits / 1e3).toFixed(1)} Kbps`;
  if (bits < 1e9) return `${(bits / 1e6).toFixed(1)} Mbps`;
  return `${(bits / 1e9).toFixed(2)} Gbps`;
}

export function formatPercent(v: number | null | undefined, digits = 0): string {
  if (v == null || !isFinite(v)) return "—";
  return `${v.toFixed(digits)}%`;
}

export function formatUptime(secs: number): string {
  const d = Math.floor(secs / 86400);
  const h = Math.floor((secs % 86400) / 3600);
  const m = Math.floor((secs % 3600) / 60);
  if (d > 0) return `${d}d ${h}h`;
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}

export function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms} ms`;
  const s = ms / 1000;
  if (s < 60) return `${s.toFixed(1)} s`;
  const m = Math.floor(s / 60);
  return `${m}m ${Math.round(s % 60)}s`;
}

export function formatTime(ts: number): string {
  const d = new Date(ts);
  const today = new Date();
  const time = d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });
  return d.toDateString() === today.toDateString() ? time : `${d.toLocaleDateString()} ${time}`;
}

export function formatMhz(mhz: number | null | undefined): string {
  if (!mhz) return "—";
  return mhz >= 1000 ? `${(mhz / 1000).toFixed(2)} GHz` : `${mhz} MHz`;
}

/** Strips vendor noise like "(TM)" and "6-Core Processor" for compact display. */
export function shortName(name: string): string {
  return name
    .replace(/\((R|TM|tm|r)\)/g, "")
    .replace(/\s+\d+-Core Processor/i, "")
    .replace(/\s+CPU\s+@.*$/i, "")
    .replace(/\s{2,}/g, " ")
    .trim();
}
