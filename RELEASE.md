# Releasing and installing Blob Optimizers

## Install

Run `Blob Optimizers_<version>_x64-setup.exe` (or the `.msi`). The installer is per-machine, so Windows asks for administrator permission once. Windows 10 (1809+) and Windows 11 are supported; WebView2 is already part of both.

The app itself runs as a standard user. Operations that need administrator rights ask through UAC when they run, or use **Elevate** in the sidebar to restart the whole app as administrator.

**Updating**: run the newer installer over the existing one. Settings, history, undo snapshots and backups (in `%LOCALAPPDATA%\com.blob.optimizers`) and saved profiles (in `Documents\Blob Optimizers`) are kept. **Uninstalling** the app leaves those folders too; delete them by hand for a clean removal.

> The installers are not code-signed. Windows SmartScreen will show "Windows protected your PC" until the build is signed or has built reputation: choose *More info → Run anyway*. Sign with `signtool` (or set `bundle.windows.certificateThumbprint` in `src-tauri/tauri.conf.json`) before distributing widely.

## Build a release

```bash
npm ci
npm run typecheck
cd src-tauri && cargo test --lib && cd ..
npm run tauri build
```

Output:

- `src-tauri/target/release/bundle/nsis/Blob Optimizers_<version>_x64-setup.exe`
- `src-tauri/target/release/bundle/msi/Blob Optimizers_<version>_x64_en-US.msi`

The first build downloads NSIS and WiX (cached afterwards). All PowerShell scripts are compiled into the executable with `include_str!`; there are no loose files to ship.

## Publish

1. Bump the version in `package.json`, `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json` (keep them equal) and add a `CHANGELOG.md` entry.
2. Build as above.
3. Create a GitHub release whose tag is the version (`v0.2.0` or `0.2.0`), mark it *latest*, attach both installers and paste the changelog entry as the notes.
4. Users who set Settings › Updates › *Release repository* to that `owner/name` see the new version in Settings (and, if enabled, a message at startup). Only GitHub's public "latest release" endpoint is read; nothing is downloaded or installed automatically, and only `https://github.com/<owner>/<name>/…` links are opened.

## Release checklist

- [ ] `npm run typecheck` is clean and `cargo test --lib` passes (the tests run real, harmless commands against this PC: registry round-trips in test keys, a dummy uninstaller, DNS lookups, process and service queries).
- [ ] Fresh install on a Windows 10 and a Windows 11 machine, as a standard user.
- [ ] Apply and revert each gaming preset; apply, undo and reset a tweak from each category; confirm states read back correctly.
- [ ] Disable/enable and remove/restore a startup entry; remove and reinstall a Store app (e.g. Microsoft Tips).
- [ ] Create a restore point; run an admin operation through UAC and cancel one mid-run.
- [ ] Settings: each option survives a restart; Start with Windows adds/removes the Run entry.
- [ ] Update check reports the published version.

## What cannot be fully reversed

The app marks these in the UI and history before they run:

- **Desktop program uninstalls** — reinstall from the vendor (Store apps can be reinstalled from Restore & Backup).
- **Storage cleanup** — temp and cache files are deleted, not moved to the Recycle Bin.
- **Network repair** (Winsock / TCP-IP reset) — restores Windows defaults; custom network stack settings are lost.
- **Deleted restore points / power plans you created** — gone once deleted (built-in plans can be restored).
- **Leftover cleanup** — folders go to the Recycle Bin and registry keys are exported to `.reg` first, so they are recoverable until the Recycle Bin is emptied.

A Windows restore point (offered before batch changes) covers most of the above.

## Known limitations

- GPU temperature is not exposed by standard Windows APIs and is not shown; CPU temperature appears only when the firmware publishes ACPI thermal zones.
- The FPS monitor counts DirectX 9–12 presents; Vulkan- and OpenGL-only games are not measured.
- English only.
- No automatic in-place updates: updates are found and linked, the user runs the installer.
- Blob Optimizers never changes Windows Defender, the firewall or Windows Update services; these are deliberately out of scope.
