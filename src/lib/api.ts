// Typed wrappers around the Rust commands. Keep in sync with src-tauri/src/commands.
import { invoke } from "@tauri-apps/api/core";

export interface AppInfo {
  version: string;
  elevated: boolean;
}

export interface ProcessSummary {
  name: string;
  instances: number;
  cpu: number;
  memory: number;
}

export interface DiskUsage {
  mount: string;
  name: string;
  kind: string;
  total: number;
  available: number;
}

export interface LiveStats {
  cpuUsage: number;
  perCore: number[];
  cpuFreqMhz: number;
  cpuTempC: number | null;
  memUsed: number;
  memTotal: number;
  swapUsed: number;
  swapTotal: number;
  gpuUsage: number | null;
  gpuMemUsed: number | null;
  netRxPerSec: number;
  netTxPerSec: number;
  interfaces: { name: string; rxPerSec: number; txPerSec: number; totalRx: number; totalTx: number }[];
  disks: DiskUsage[];
  processCount: number;
  topProcesses: ProcessSummary[];
  uptimeSecs: number;
}

export interface HardwareInfo {
  windows: {
    productName: string;
    displayVersion: string;
    build: string;
    edition: string;
    architecture: string;
    installDate: string | null;
    computerName: string;
    lastBoot: string | null;
  };
  cpu: {
    name: string;
    manufacturer: string;
    cores: number;
    threads: number;
    maxClockMhz: number;
    l2CacheKb: number;
    l3CacheKb: number;
    socket: string;
    virtualization: boolean;
  };
  gpus: {
    name: string;
    manufacturer: string;
    vramBytes: number | null;
    driverVersion: string;
    driverDate: string | null;
    resolution: string | null;
    refreshRate: number | null;
    status: string;
  }[];
  memory: {
    totalBytes: number;
    modules: {
      capacityBytes: number;
      speedMhz: number;
      configuredMhz: number;
      manufacturer: string;
      partNumber: string;
      slot: string;
    }[];
  };
  disks: {
    number: number;
    model: string;
    mediaType: string;
    busType: string;
    sizeBytes: number;
    health: string;
    temperature: number | null;
    wearPercent: number | null;
    volumes: { letter: string; label: string; fileSystem: string; sizeBytes: number; freeBytes: number }[];
  }[];
  board: {
    manufacturer: string;
    product: string;
    systemModel: string;
    biosVendor: string;
    biosVersion: string;
    biosDate: string | null;
    firmware: string | null;
  };
  network: {
    name: string;
    description: string;
    status: string;
    linkSpeed: string;
    mac: string;
    mediaType: string;
    ipv4: string | null;
  }[];
  monitors: {
    name: string | null;
    manufacturer: string | null;
    serial: string | null;
    sizeInches: number | null;
    year: number | null;
  }[];
  config: {
    powerPlan: { guid: string; name: string } | null;
    gameModeEnabled: boolean;
    hagsEnabled: boolean | null;
  };
  collectedAt: string;
}

// ---------------- Command engine ----------------

export type Shell = "powershell" | "cmd";
export type JobStatus = "running" | "succeeded" | "failed" | "cancelled" | "timedOut" | "declined";
export type Stream = "out" | "err" | "info";

export interface OutputLine {
  stream: Stream;
  text: string;
  transient?: boolean;
}

export interface HistoryEntry {
  id: string;
  title: string;
  source: "catalog" | "user" | "internal";
  operationId: string | null;
  shell: Shell;
  command: string;
  elevated: boolean;
  modifiesSystem: boolean;
  cancellable: boolean;
  startedAt: number;
  finishedAt: number | null;
  status: JobStatus;
  exitCode: number | null;
  error: string | null;
  progress: { percent: number; message: string | null } | null;
  lineCount: number;
}

export type ParamKind =
  | { type: "host" }
  | { type: "int"; min: number; max: number }
  | { type: "choice"; options: string[] };

export interface Operation {
  id: string;
  title: string;
  description: string;
  effects: string;
  category: string;
  shell: Shell;
  script: string;
  requiresAdmin: boolean;
  modifiesSystem: boolean;
  cancellable: boolean;
  risk: "safe" | "caution" | "high";
  timeoutSecs: number;
  durationHint: string;
  params: { name: string; label: string; kind: ParamKind; default: string }[];
}

export interface Validation {
  level: "ok" | "warn" | "block";
  reasons: string[];
}

export interface ActiveJob {
  entry: HistoryEntry;
  lines: OutputLine[];
}

// ---------------- Apps ----------------

export type Recommendation = "safe" | "optional" | "caution" | "protected";
export type AppCategory = "microsoft" | "gaming" | "thirdParty";

export interface InstalledApp {
  id: string;
  kind: "win32" | "appx";
  name: string;
  publisher: string;
  version: string;
  installDate: string | null;
  sizeBytes: number | null;
  installLocation: string | null;
  category: AppCategory;
  recommendation: Recommendation;
  appKind: "app" | "runtime" | "driver" | "system";
  note: string;
  preinstalled: boolean;
  scope: "machine" | "user";
  uninstallMethod: "quiet" | "msi" | "interactive" | "store";
  reinstallable: boolean;
  packageName: string | null;
  storeId: string | null;
}

export interface RemovedApp {
  name: string;
  packageName: string;
  storeId: string | null;
  removedAt: number;
}

export interface Leftover {
  id: string;
  kind: "folder" | "registry";
  path: string;
  sizeBytes: number;
  items: number;
  reason: string;
  userData: boolean;
  requiresAdmin: boolean;
  defaultSelected: boolean;
}

export interface LeftoverScan {
  scanId: string;
  appName: string;
  items: Leftover[];
}

export interface WindowsFeature {
  name: string;
  title: string;
  description: string | null;
  enabled: boolean;
  advice: "keepOn" | "keepOff" | "either" | "protected" | null;
  common: boolean;
}

// ---------------- Tweaks & system ----------------

export type TweakCategory = "performance" | "privacy" | "windows" | "system" | "gaming";
export type TweakState = "on" | "off" | "custom";

export interface TweakInfo {
  id: string;
  title: string;
  description: string;
  caveat: string | null;
  category: TweakCategory;
  group: string;
  recommend: "apply" | "keep" | "either";
  after: "nothing" | "explorer" | "signOut" | "restart";
  requiresAdmin: boolean;
  state: TweakState;
  /** Current state is what Windows ships with. */
  atDefault: boolean;
  /** Windows' default is the "on" state (e.g. Game Mode). */
  defaultOn: boolean;
  changesOn: string[];
  changesOff: string[];
  canUndo: boolean;
  lastChangeAt: number | null;
}

export type TweakTarget = "on" | "off" | "default" | "undo";

// ---------------- Gaming ----------------

export interface PresetInfo {
  id: "balanced" | "gaming" | "maximum";
  name: string;
  description: string;
  warning: string | null;
  plan: "balanced" | "high" | "ultimate";
  active: boolean;
  changes: { id: string; title: string; caveat: string | null; from: TweakState; to: TweakState }[];
  planChange: string | null;
  requiresAdmin: boolean;
}

export interface LastPreset {
  preset: string;
  at: number;
  tweaks: string[];
  previousPlanName: string | null;
}

export interface Game {
  id: string;
  source: "steam" | "epic" | "gog" | "riot" | "roblox" | "custom";
  name: string;
  installDir: string;
  exe: string | null;
}

export interface GameProfile {
  gpu: "default" | "powerSaving" | "highPerformance";
  fullscreenOptimizations: boolean;
  priority: "normal" | "aboveNormal" | "high";
}

export interface DisplayInfo {
  device: string;
  adapter: string;
  monitor: string;
  primary: boolean;
  width: number;
  height: number;
  hz: number;
  rates: number[];
}

export interface FpsProcess {
  pid: number;
  name: string;
  fps: number;
  frameMs: number;
  low1Fps: number;
}

export interface FpsSample {
  t: number;
  processes: FpsProcess[];
  error: string | null;
  needsAdmin: boolean;
}

export interface PowerPlan {
  guid: string;
  name: string;
  active: boolean;
  builtIn: boolean;
}

export interface SystemInfo {
  plans: PowerPlan[];
  monitorAc: number | null;
  monitorDc: number | null;
  sleepAc: number | null;
  sleepDc: number | null;
  hasBattery: boolean;
  hibernate: boolean;
  hiberfilBytes: number | null;
  pagefile: {
    automatic: boolean;
    ramMb: number;
    files: { path: string; allocatedMb: number; currentUsageMb: number; peakUsageMb: number; initialMb: number | null; maximumMb: number | null }[];
  };
}

export interface CleanupItem {
  id: string;
  bytes: number;
  files: number;
}

export interface ServiceInfo {
  name: string;
  displayName: string;
  description: string | null;
  running: boolean;
  startMode: string;
  delayed: boolean;
  protected: boolean;
  note: string | null;
  suggested: "automatic" | "manual" | "disabled" | null;
  original: "automatic" | "manual" | "disabled" | null;
}

export interface StartupEntry {
  id: string;
  source: "registry" | "folder" | "store";
  location: string;
  name: string;
  command: string;
  path: string | null;
  publisher: string | null;
  enabled: boolean;
  scope: "user" | "machine";
  impactMs: number | null;
  advice: "safeToDisable" | "caution" | "keep";
  note: string;
  removable: boolean;
  policy: boolean;
}

export interface StartupList {
  impactAvailable: boolean;
  entries: StartupEntry[];
  removed: { name: string; location: string; removedAt: number }[];
}

export interface ProcessInfo {
  pid: number;
  parent: number | null;
  name: string;
  path: string | null;
  publisher: string | null;
  cpu: number;
  memory: number;
  startTime: number;
  protection: "none" | "caution" | "protected";
  note: string | null;
}

// ---------------- Network, restore, settings ----------------

export interface NetAdapter {
  index: number;
  name: string;
  description: string;
  status: string;
  mediaType: string;
  physical: boolean;
  linkSpeed: string;
  mac: string;
  ipv4: string[];
  ipv6: string[];
  gateway: string;
  dhcp: boolean;
  mtu: number;
  dns: string[];
  dnsManual: boolean;
  category: string | null;
  networkName: string | null;
  ssid: string | null;
  signal: number | null;
  radio: string | null;
  band: string | null;
  channel: number | null;
}

export interface NetCheck {
  id: string;
  label: string;
  ok: boolean;
  detail: string;
  advice: string | null;
}

export interface DnsBench {
  name: string;
  server: string;
  current: boolean;
  uncachedMs: number | null;
  cachedMs: number | null;
  failures: number;
  queries: number;
}

export interface RegBackup {
  file: string;
  created: number;
  sizeBytes: number;
  keys: string[];
  machineWide: boolean;
}

export interface ProfileFile {
  file: string;
  created: number;
  computer: string;
  count: number;
}

export interface ImportChange {
  id: string;
  title: string;
  from: TweakState;
  target: TweakTarget;
}

export interface AppSettings {
  theme: "dark" | "light" | "system";
  accent: string;
  refreshMs: number;
  notifySuccess: boolean;
  startMinimized: boolean;
  restorePointDefault: boolean;
  historyLimit: number;
  defaultPreset: "balanced" | "gaming" | "maximum";
  language: string;
  checkUpdates: boolean;
  updateRepo: string;
  startWithWindows: boolean;
}

export interface UpdateInfo {
  current: string;
  latest: string;
  available: boolean;
  url: string;
  name: string;
  notes: string;
}

export const api = {
  appInfo: () => invoke<AppInfo>("get_app_info"),
  liveStats: () => invoke<LiveStats>("get_live_stats"),
  hardwareInfo: (refresh = false) => invoke<HardwareInfo>("get_hardware_info", { refresh }),

  catalog: () => invoke<Operation[]>("engine_catalog"),
  runOperation: (id: string, params?: Record<string, string>) =>
    invoke<HistoryEntry>("engine_run_operation", { id, params }),
  validateCommand: (shell: Shell, command: string) => invoke<Validation>("engine_validate", { shell, command }),
  runCommand: (shell: Shell, command: string, confirmed: boolean, asAdmin: boolean) =>
    invoke<HistoryEntry>("engine_run_command", { shell, command, confirmed, asAdmin }),
  cancelJob: (id: string) => invoke<void>("engine_cancel", { id }),
  activeJobs: () => invoke<ActiveJob[]>("engine_active"),
  jobOutput: (id: string) => invoke<OutputLine[]>("engine_output", { id }),
  history: (limit?: number) => invoke<HistoryEntry[]>("engine_history", { limit }),
  clearHistory: () => invoke<void>("engine_clear_history"),
  relaunchAsAdmin: () => invoke<void>("relaunch_as_admin"),

  apps: (refresh = false) => invoke<InstalledApp[]>("apps_inventory", { refresh }),
  uninstallApp: (id: string) => invoke<HistoryEntry>("apps_uninstall", { id }),
  removeStoreApps: (ids: string[]) => invoke<HistoryEntry>("apps_remove_store", { ids }),
  removedApps: () => invoke<RemovedApp[]>("apps_removed"),
  reinstallApp: (packageName: string) => invoke<HistoryEntry>("apps_reinstall", { packageName }),
  scanLeftovers: (id: string) => invoke<LeftoverScan>("apps_scan_leftovers", { id }),
  cleanupLeftovers: (scanId: string, itemIds: string[]) =>
    invoke<HistoryEntry>("apps_cleanup_leftovers", { scanId, itemIds }),
  features: () => invoke<WindowsFeature[]>("features_list"),
  toggleFeature: (name: string, enable: boolean) => invoke<HistoryEntry>("features_toggle", { name, enable }),

  tweaks: () => invoke<TweakInfo[]>("tweaks_list"),
  applyTweaks: (changes: { id: string; target: TweakTarget }[]) => invoke<HistoryEntry>("tweaks_apply", { changes }),
  systemInfo: () => invoke<SystemInfo>("system_info"),
  setPowerPlan: (guid: string) => invoke<HistoryEntry>("power_set_active", { guid }),
  addPowerPlan: (which: "high" | "ultimate") => invoke<HistoryEntry>("power_add_plan", { which }),
  deletePowerPlan: (guid: string) => invoke<HistoryEntry>("power_delete_plan", { guid }),
  restoreDefaultPlans: () => invoke<HistoryEntry>("power_restore_defaults"),
  setTimeouts: (timeouts: { monitorAc: number; sleepAc: number; monitorDc: number | null; sleepDc: number | null }) =>
    invoke<HistoryEntry>("power_set_timeouts", { timeouts }),
  setHibernate: (on: boolean) => invoke<HistoryEntry>("power_set_hibernate", { on }),
  setPagefile: (automatic: boolean, initialMb: number | null, maximumMb: number | null) =>
    invoke<HistoryEntry>("pagefile_set", { automatic, initialMb, maximumMb }),
  cleanupScan: () => invoke<CleanupItem[]>("cleanup_scan"),
  cleanupRun: (ids: string[]) => invoke<HistoryEntry>("cleanup_run", { ids }),
  services: () => invoke<ServiceInfo[]>("services_list"),
  setServiceStart: (name: string, start: "automatic" | "manual" | "disabled") => invoke<HistoryEntry>("services_set_start", { name, start }),
  controlService: (name: string, start: boolean) => invoke<HistoryEntry>("services_control", { name, start }),
  startup: () => invoke<StartupList>("startup_list"),
  setStartupEnabled: (id: string, enabled: boolean) => invoke<HistoryEntry>("startup_set_enabled", { id, enabled }),
  removeStartup: (id: string) => invoke<HistoryEntry>("startup_remove", { id }),
  restoreStartup: (name: string, location: string) => invoke<HistoryEntry>("startup_restore", { name, location }),
  processes: () => invoke<ProcessInfo[]>("processes_list"),
  endProcess: (p: ProcessInfo, tree: boolean, asAdmin: boolean) =>
    invoke<HistoryEntry>("processes_end", { pid: p.pid, name: p.name, startTime: p.startTime, tree, asAdmin }),

  presets: () => invoke<[PresetInfo[], LastPreset | null]>("gaming_presets"),
  applyPreset: (id: string) => invoke<HistoryEntry>("gaming_apply_preset", { id }),
  revertPreset: () => invoke<HistoryEntry>("gaming_revert_preset"),
  games: () => invoke<[Game, GameProfile | null][]>("games_list"),
  addGame: (name: string, exe: string) => invoke<Game>("games_add", { name, exe }),
  removeGame: (id: string) => invoke<void>("games_remove", { id }),
  gameCandidates: (id: string) => invoke<{ path: string; sizeBytes: number }[]>("games_candidates", { id }),
  chooseGameExe: (id: string, exe: string) => invoke<void>("games_choose_exe", { id, exe }),
  setGameProfile: (exe: string, profile: GameProfile) => invoke<HistoryEntry>("games_set_profile", { exe, profile }),
  displays: () => invoke<DisplayInfo[]>("displays_list"),
  setRefreshRate: (device: string, hz: number) => invoke<HistoryEntry>("display_set_refresh_rate", { device, hz }),
  fpsStart: (elevate: boolean) => invoke<void>("fps_start", { elevate }),
  fpsPoll: () => invoke<[boolean, FpsSample | null]>("fps_poll"),
  fpsStop: () => invoke<void>("fps_stop"),

  netAdapters: () => invoke<NetAdapter[]>("network_adapters"),
  setDns: (index: number, servers: string[]) => invoke<HistoryEntry>("network_set_dns", { index, servers }),
  setAdapterEnabled: (index: number, enabled: boolean) => invoke<HistoryEntry>("network_set_adapter", { index, enabled }),
  setNetworkCategory: (index: number, isPrivate: boolean) => invoke<HistoryEntry>("network_set_category", { index, private: isPrivate }),
  netDiagnose: () => invoke<NetCheck[]>("network_diagnose"),
  dnsPresets: () => invoke<[string, string, string][]>("network_dns_presets"),
  dnsBenchmark: (current: string[]) => invoke<DnsBench[]>("network_dns_benchmark", { current }),
  restoreStatus: () => invoke<{ enabled: boolean; disabledByPolicy: boolean }>("restore_status"),
  restoreList: () => invoke<HistoryEntry>("restore_list"),
  restoreRollback: (sequence: number) => invoke<HistoryEntry>("restore_rollback", { sequence }),
  registryBackups: () => invoke<RegBackup[]>("backups_registry"),
  importRegistryBackup: (file: string) => invoke<HistoryEntry>("backups_registry_import", { file }),
  exportProfile: () => invoke<string>("profile_export"),
  profiles: () => invoke<[string, ProfileFile[]]>("profile_list"),
  previewProfile: (file: string) => invoke<ImportChange[]>("profile_preview", { file }),
  settingsGet: () => invoke<AppSettings>("settings_get"),
  settingsSet: (settings: AppSettings) => invoke<HistoryEntry | null>("settings_set", { settings }),
  appPaths: () => invoke<[string, string]>("app_paths"),
  checkUpdates: () => invoke<UpdateInfo>("updates_check"),
  openRelease: (url: string) => invoke<void>("updates_open", { url }),
};

/** Normalises errors thrown by `invoke` (Rust errors arrive as plain strings). */
export function errorMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return JSON.stringify(e);
}
