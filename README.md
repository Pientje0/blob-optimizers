# Blob Optimizers

All-in-one Windows PC & gaming optimization suite. Tauri 2 (Rust) + React + TypeScript + Tailwind + Recharts + Lucide.

## Install with one command

Open PowerShell and run:

```powershell
irm https://raw.githubusercontent.com/Pientje09-abe/blob-optimizers/main/install.ps1 | iex
```

This downloads the latest release to `%LOCALAPPDATA%\Programs\Blob Optimizers Portable`, checks its SHA-256 against the release, adds a Start menu shortcut and starts it. No administrator rights are needed to install; run the same command again to update. Prefer an installer? Download the `setup.exe` or `.msi` from [Releases](https://github.com/Pientje09-abe/blob-optimizers/releases/latest).

## Run from source

Double-click **`Run Blob Optimizers.bat`**. It starts the release build, building it first if needed. Run `"Run Blob Optimizers.bat" rebuild` after changing the code.

```bash
npm install
npm run tauri dev      # development
npm run tauri build    # installer (NSIS + MSI) in src-tauri/target/release/bundle
```

Requirements: Node 18+, Rust (MSVC toolchain), Visual Studio C++ Build Tools, WebView2 (preinstalled on Windows 10/11).

See [RELEASE.md](RELEASE.md) for installing, publishing a release and known limitations, and [CHANGELOG.md](CHANGELOG.md) for what's in each version.

## Layout

```
src/                      React frontend
  context/                SystemContext (live polling + hardware cache), SettingsContext
  components/             layout (Sidebar), ui (Card, Gauge, Badge), charts, dashboard widgets
  pages/                  one file per sidebar section
  lib/                    api.ts (typed Rust commands), format, insights (recommendations)
  navigation.ts           sidebar definition
src-tauri/
  scripts/hardware.ps1    read-only hardware/Windows detection (emits JSON)
  src/commands/           Tauri command handlers exposed to the UI
  src/system/             live.rs (sysinfo), perf.rs (PDH CPU/GPU counters), hardware.rs
  src/util/               hidden PowerShell runner, elevation check, serde helpers
```

## Stage status

| Stage | Scope | Status |
|------|-------|--------|
| 1 | Foundation, sidebar, dashboard, dark theme, IPC | Done |
| 2 | Hardware detection, live monitoring | Done |
| 3 | Terminal command engine | Done |
| 4 | Debloater, app uninstaller | Done |
| 5 | Windows tweaks, startup & process manager | Done |
| 6 | Gaming optimization | Done |
| 7 | Network, backup & restore, history, settings | Done |
| 8 | Polish & release (update checks, installers, release docs) | Done |

## Network, Restore & Backup, Settings (stage 7)

- **Network** (`src-tauri/src/network`): adapters with IP, gateway, DNS (manual/automatic), Wi-Fi SSID/signal/band, network category and live per-adapter traffic. Change DNS per adapter (automatic, public presets, or validated custom IPs), enable/disable adapters, switch Private/Public, all with confirmation. Step-by-step diagnostics (adapter → IP → router → internet → DNS) and a DNS speed test that times real UDP lookups against the current resolver and public ones. Repair tools: flush DNS, renew IP, reset Winsock, reset TCP/IP (reviewed catalog operations with effects shown first).
- **Restore & Backup** (`src-tauri/src/backup.rs`): System Protection status, create restore points, list them and roll back (restart, strong confirmation). One place for everything that can be undone: tweak snapshots, the last gaming preset, changed services, removed startup entries, removed Store apps and `.reg` backups from leftover cleanup. "Reset all tweaks to Windows defaults" with preview. Tweak profiles can be saved to `Documents\Blob Optimizers` and applied later (or on another PC) after a preview. Operation history marks changes that cannot be reversed automatically.
- **Updates** (`src-tauri/src/updates.rs`): reads the latest GitHub release of the repository set in Settings › Updates, compares versions and links to the release page (only `https://github.com/<repo>/…` addresses are opened). Optional check once per launch. Nothing is downloaded or installed automatically.
- **Settings** (`src-tauri/src/settings.rs`): theme (dark/light/system, title bar follows), accent colour, monitoring refresh rate, success notifications, start with Windows (HKCU Run) and start minimised, default gaming preset, restore-point default for batch changes, history size (older logs are pruned), data folder. Language is English for now.

## Gaming (`src-tauri/src/gaming`)

- **Presets** (`mod.rs`): Balanced, Gaming and Maximum Performance are lists of catalogue tweaks (category `gaming`, plus a few performance ones) and a power plan. The UI previews every change (current → target) before applying, can create a restore point first, and "Revert last preset" undoes exactly what the last preset changed and switches back to the previous power plan. Maximum Performance carries an explicit warning; no preset claims an FPS gain.
- **Windows gaming features**: Game Mode, background recording (Game DVR), optimizations for windowed games, hardware-accelerated GPU scheduling, mouse acceleration, Sticky Keys prompt, MMCSS game priority, SystemResponsiveness, network throttling — each with description, trade-off, reset and undo. Tweaks can now declare `default_on` when Windows' own default is the "on" state.
- **Game profiles** (`games.rs`): games detected from Steam, Epic, GOG, Riot and Roblox (or added by path). Per-executable GPU preference (Windows Graphics settings), fullscreen optimizations (compatibility flag, other flags preserved) and CPU priority (IFEO `PerfOptions`, admin).
- **Displays** (`display.rs`, `scripts/display.ps1`): every attached display with the refresh rates it supports at its current resolution; changing it shows a 15-second keep/revert prompt like Windows.
- **FPS monitor** (`fps.rs`): counts DXGI (DirectX 10-12) and Direct3D 9 present events per process through a real-time ETW session, like PresentMon, reporting FPS, frame time and 1% lows alongside CPU/GPU load with a bottleneck hint. It runs as `blob-optimizers.exe --fps-helper`, unelevated when the account may trace (administrators, *Performance Log Users*), otherwise through UAC. Blob Optimizers' own rendering is excluded. Vulkan/OpenGL-only games are not counted.
- **Network for gaming**: Wi-Fi vs cable advice and a latency/jitter/packet-loss test using the reviewed `ping` operation.

## Windows optimization (`src-tauri/src/tweaks`, `src-tauri/src/system/processes.rs`)

- **Tweaks** (`catalog.rs`, `model.rs`): 94 reviewed tweaks (Performance, Latency, Gaming, Network, Privacy incl. app permissions, Windows UI/Update preferences, System). Besides registry values and services, a tweak can be a reviewed PowerShell check/apply pair (`Op::Cmd`, used for network-adapter properties and power-plan values); all checks run in one background PowerShell pass and unsupported ones are hidden. Each is two operation lists: *applied* and *Windows default*. State is read natively from the registry (`util/registry.rs`) and shown as Applied / Windows default / Custom. Before every change the touched values are snapshotted (`tweak-backups.json`), so each tweak supports apply, reset to default and *undo last change*. Batch "Apply recommended" can create a restore point first. No tweak touches Defender, the firewall or Windows Update services (enforced by a test).
- **System tab** (`system.rs`): power plans (switch, add High/Ultimate Performance, delete custom, restore defaults), screen/sleep timeouts, hibernation; virtual memory (automatic or custom size, never removed, 1 GB minimum); storage cleanup of fixed temp/cache locations with a scan first; services (`services.rs`) with protected security/update/networking services and remembered original startup types.
- **Startup** (`startup.rs`): Run keys, Startup folders and Store app startup tasks. Enable/disable uses Task Manager's `StartupApproved` data (fully reversible); removing a registry entry saves it for one-click restore, folder shortcuts go to the Recycle Bin. Boot delays are shown when Windows has recorded them.
- **Processes**: live list via sysinfo with publishers read in the background; core Windows/security processes and Blob Optimizers' own processes are locked; termination re-checks PID + start time to avoid PID reuse.

## Apps: debloater and uninstaller (`src-tauri/src/apps`)

- **Inventory** (`scripts/apps-inventory.ps1`): desktop programs from the three Uninstall registry views plus Store (Appx) packages for the current user; friendly Store names come from the Start menu.
- **Classification** (`knowledge.rs`): curated rules give each app a category (Microsoft / Gaming / Third-party), a type (app, runtime, driver, system) and a recommendation: *Safe to remove*, *Your choice*, *Caution* (with the consequence spelled out) or *Protected*. Protected items (Store, Windows Security, winget, Edge/WebView2, runtimes, shell components, anything Windows flags non-removable) are rejected by the backend even if requested.
- **Removal**: Store apps via `Remove-AppxPackage` for the current user; desktop programs with their own registered uninstaller (MSI → `msiexec /x … /passive`, otherwise the quiet or normal uninstall string read from the registry, never from the UI). The script waits for hand-off helpers and confirms the registry entry is gone. Cancelling stops waiting only; the vendor uninstaller is never killed midway.
- **Restore point** before removal (optional, on by default for desktop programs and Caution apps). If it fails, nothing is removed.
- **Reinstall** (`removed-apps.json`): removed Store apps are recorded; reinstall re-registers the copy Windows keeps, or downloads by Store ID through winget.
- **Leftovers** (`leftovers.rs`): exact-name matches only, in Program Files, ProgramData, AppData and the program's own install folder, plus matching `Software` registry keys. Everything is re-validated against an allow-list, items shared with still-installed programs are dropped, folders go to the Recycle Bin and registry keys are exported to `.reg` backups before deletion. Personal settings are never pre-selected.
- **Windows features** (`features.rs`): listed via `Win32_OptionalFeature` (no admin needed), toggled with `Enable/Disable-WindowsOptionalFeature`; required features cannot be turned off.

## Command engine (`src-tauri/src/engine`)

Every system operation goes through the engine; other modules submit a `JobRequest`.

- **Catalog** (`catalog.rs`): reviewed operations with typed, validated parameters (no string pasting of raw input). Each declares whether it needs admin, changes the system, can be cancelled safely, its time limit and an explanation of its effects.
- **Typed commands** (`validate.rs`): the Terminal tab accepts PowerShell/CMD. Catastrophic patterns (formatting disks, deleting drive roots or Windows folders, deleting shadow copies, breaking boot config) are blocked; risky ones (Defender/Firewall/Update, registry, wildcard/recursive deletes, killing processes, remote code) require confirmation with the reason shown. The backend re-checks on submit.
- **Execution** (`runner.rs`, `launch.rs`): hidden processes, live output streamed as `engine:output` events, `\r` progress handled in place, UTF-8/UTF-16 output decoding, progress via `##PROGRESS <pct> <msg>` lines or a per-operation regex, timeouts and cancellation that kill the whole process tree.
- **Elevation**: when the app is not elevated, admin operations launch through UAC (`ShellExecuteEx runas`). The elevated helper writes output to files the app tails live and watches a cancel file, so cancellation and timeouts still work. The sidebar "Elevate" button restarts the whole app as administrator (installed builds only).
- **Conflicts**: system-changing operations and typed commands run one at a time; read-only operations can run alongside. Max 4 concurrent jobs.
- **History** (`history.rs`): `%LOCALAPPDATA%\com.blob.optimizers\engine\history.jsonl` plus a JSONL output log per job (last 500 kept).

Windows PowerShell 5.1 normally serialises errors as CLIXML when redirected; `scripts/engine-wrapper.ps1` renders all streams as plain text and maps errors to a non-zero exit code.

## Notes on monitoring

- CPU and GPU utilisation come from Windows PDH counters (`% Processor Utility`, `GPU Engine`), the same sources as Task Manager. sysinfo's CPU usage is only a fallback because it reports 100% on some recent Windows builds.
- Polling pauses while the window is hidden and never overlaps (next poll is scheduled after the previous one returns).
- GPU temperature is not exposed by standard Windows APIs; CPU temperature appears only when the firmware publishes ACPI thermal zones.
- Run `cargo test --lib` in `src-tauri` to check the monitor and hardware detection against the real machine.
