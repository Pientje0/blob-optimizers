import {
  Activity,
  Cpu,
  Gamepad2,
  History,
  LayoutDashboard,
  ListX,
  Network,
  PackageX,
  Power,
  Settings,
  SlidersHorizontal,
  TerminalSquare,
  type LucideIcon,
} from "lucide-react";

export type PageId =
  | "dashboard"
  | "hardware"
  | "gaming"
  | "debloat"
  | "tweaks"
  | "startup"
  | "processes"
  | "uninstaller"
  | "network"
  | "terminal"
  | "restore"
  | "settings";

export interface NavItem {
  id: PageId;
  label: string;
  icon: LucideIcon;
  description: string;
}

export interface NavGroup {
  title: string;
  items: NavItem[];
}

export const NAV: NavGroup[] = [
  {
    title: "Overview",
    items: [
      { id: "dashboard", label: "Dashboard", icon: LayoutDashboard, description: "Live system overview" },
      { id: "hardware", label: "Hardware", icon: Cpu, description: "Detected components and configuration" },
    ],
  },
  {
    title: "Optimize",
    items: [
      { id: "gaming", label: "Gaming", icon: Gamepad2, description: "Presets, Game Mode and power plans" },
      { id: "tweaks", label: "Windows Tweaks", icon: SlidersHorizontal, description: "Performance, privacy and UI tweaks" },
      { id: "debloat", label: "Debloater", icon: PackageX, description: "Remove preinstalled apps safely" },
      { id: "network", label: "Network", icon: Network, description: "Adapters, DNS and diagnostics" },
    ],
  },
  {
    title: "Manage",
    items: [
      { id: "startup", label: "Startup", icon: Power, description: "Control what launches with Windows" },
      { id: "processes", label: "Processes", icon: Activity, description: "Running processes and resource use" },
      { id: "uninstaller", label: "Uninstaller", icon: ListX, description: "Uninstall apps and clean leftovers" },
    ],
  },
  {
    title: "System",
    items: [
      { id: "terminal", label: "Terminal", icon: TerminalSquare, description: "Command engine, output and history" },
      { id: "restore", label: "Restore & Backup", icon: History, description: "Restore points and undo history" },
      { id: "settings", label: "Settings", icon: Settings, description: "Blob Optimizers preferences" },
    ],
  },
];

export const NAV_ITEMS: NavItem[] = NAV.flatMap((g) => g.items);
export const findNav = (id: PageId) => NAV_ITEMS.find((i) => i.id === id)!;
