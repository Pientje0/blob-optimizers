# Changelog

## 0.2.0

- **Latency section** in three places: Gaming › Latency, Network › Latency & tweaks and Windows Tweaks › Latency. It lists every input, scheduling, power and network latency setting with a ping test.
- **29 new tweaks (94 in total; ones your hardware or Windows version doesn't support are hidden):**
  - Gaming: fullscreen optimizations off globally, multiplane overlay off, foreground-favouring CPU time slices (Win32PrioritySeparation), global timer resolution requests, USB selective suspend off, shorter keyboard repeat delay, Toggle/Filter Keys shortcuts off, Game Bar controller button and tips.
  - Network: Nagle's algorithm off, interrupt moderation, Energy-Efficient Ethernet and flow control off, local-first name resolution, LLMNR off.
  - Also performance (Edge background, search indexing, drag outlines, selection effects), privacy (inking/typing, online speech, error reporting), Windows (taskbar seconds, snap flyout, shake to minimise, dark mode) and system (verbose status, blue-screen details) tweaks.
- **Settings backed by reviewed PowerShell checks** (network-adapter properties, power-plan values). They're read in one background pass at startup, are hidden when your hardware doesn't support them, and can be undone.
- **Reorganised pages:**
  - Gaming: tabs for Presets & overview, FPS & performance, Latency, Games & displays.
  - Network: tabs for Adapters, Latency & tweaks, Diagnostics/DNS test/repair.
  - Windows Tweaks: Latency, Gaming, Network and All tabs.
  - Tweak lists: search, filters (applied / not applied / recommended) and a denser two-column layout.
- Gaming presets also handle the Toggle/Filter Keys prompts and Game Bar tips. Maximum Performance adds foreground-favouring time slices.

## 0.1.0 — first release

Everything below runs through the built-in command engine with reviewed, parameter-validated operations; each change is explained before it runs and logged in the Terminal tab.

- **Dashboard**: live CPU/GPU/RAM/disk/network, temperatures where Windows exposes them, PC and gaming status, recommendations, quick actions (the preset shortcut follows Settings › Default gaming preset and opens its preview), recent operations.
- **Hardware**: CPU, GPU (VRAM, driver), memory modules, drives (SSD/HDD, health), motherboard, BIOS, Windows, network adapters, monitors; manual refresh.
- **Gaming**: Balanced / Gaming / Maximum Performance presets with a full preview and one-click revert; Game Mode, Game DVR, HAGS, MMCSS and other gaming tweaks; per-game GPU preference, fullscreen optimisations and CPU priority; refresh-rate switching with keep/revert; ETW-based FPS monitor; latency test.
- **Windows Tweaks**: 56 tweaks (performance, privacy, Windows UI, update preferences, system), each with current/recommended state, undo and reset to default; power plans, virtual memory, storage cleanup, services.
- **Debloater & Uninstaller**: classified app inventory, Store app removal with reinstall, desktop uninstall via the registered uninstaller, leftover scan with Recycle Bin and `.reg` backups, optional Windows features.
- **Startup & Processes**: reversible enable/disable, removal with restore; process list with protected system processes.
- **Network**: adapters, DNS (presets, custom, speed test), diagnostics, safe repair tools.
- **Restore & Backup**: restore points, every undo snapshot in one place, reset all tweaks, exportable tweak profiles, history marking irreversible changes.
- **Terminal**: PowerShell/CMD with blocked catastrophic commands and confirmation for risky ones, live output, cancellation, timeouts, UAC elevation, history.
- **Settings**: theme, accent, refresh rate, start with Windows/minimised, notifications, default preset, restore-point default, history size, update checks.
- **Installers**: NSIS (`…_x64-setup.exe`) and MSI.
