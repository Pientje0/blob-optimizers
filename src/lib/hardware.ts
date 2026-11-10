import type { HardwareInfo } from "./api";

/** The GPU most likely used for games: the one with the most VRAM (a dedicated card beats an iGPU). */
export function primaryGpu(hw: HardwareInfo | null) {
  if (!hw || hw.gpus.length === 0) return null;
  return [...hw.gpus].sort((a, b) => (b.vramBytes ?? 0) - (a.vramBytes ?? 0))[0];
}

export function memorySpeed(hw: HardwareInfo | null): number | null {
  const m = hw?.memory.modules[0];
  if (!m) return null;
  return m.configuredMhz || m.speedMhz || null;
}
