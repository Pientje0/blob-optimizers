//! Optional Windows features (Windows Features dialog), listed via CIM and toggled through DISM cmdlets.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::AppResult;
use crate::util::powershell;
use crate::util::serde_helpers::one_or_many;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FeatureAdvice {
    /// Recommended on for most people.
    KeepOn,
    /// Recommended off unless you specifically need it.
    KeepOff,
    /// Depends on what you use.
    Either,
    /// Required by Windows; cannot be turned off here.
    Protected,
}

struct Known {
    name: &'static str,
    title: &'static str,
    description: &'static str,
    advice: FeatureAdvice,
}

const fn k(name: &'static str, title: &'static str, description: &'static str, advice: FeatureAdvice) -> Known {
    Known { name, title, description, advice }
}

use FeatureAdvice::*;

static KNOWN: &[Known] = &[
    k("Microsoft-Windows-Subsystem-Linux", "Windows Subsystem for Linux", "Run Linux tools and distributions. Needed for WSL.", Either),
    k("VirtualMachinePlatform", "Virtual Machine Platform", "Virtualization used by WSL 2 and Android apps. Running the hypervisor can slightly lower performance in some games.", Either),
    k("Microsoft-Hyper-V-All", "Hyper-V", "Microsoft's virtual machine platform. Running the hypervisor can slightly lower performance in some games, and some anti-cheat systems dislike it.", Either),
    k("HypervisorPlatform", "Windows Hypervisor Platform", "Lets third-party virtualization apps (e.g. VMware, VirtualBox, emulators) use the Windows hypervisor.", Either),
    k("Containers-DisposableClientVM", "Windows Sandbox", "A throwaway desktop for testing untrusted programs. Requires virtualization.", Either),
    k("NetFx3", ".NET Framework 3.5", "Older apps and games built for .NET 2.0/3.5 need this. Harmless to keep.", Either),
    k("NetFx4-AdvSrvs", ".NET Framework 4 Advanced Services", "Required by many apps and Windows components.", Protected),
    k("LegacyComponents", "Legacy Components", "Container for DirectPlay, used by some older games.", Either),
    k("DirectPlay", "DirectPlay", "Networking API for games from the early 2000s. Turn on only if an old game asks for it.", Either),
    k("SMB1Protocol", "SMB 1.0/CIFS file sharing", "Obsolete, insecure file-sharing protocol (exploited by WannaCry). Keep off unless an old NAS requires it.", KeepOff),
    k("SMB1Protocol-Client", "SMB 1.0 client", "Obsolete, insecure file-sharing client.", KeepOff),
    k("SMB1Protocol-Server", "SMB 1.0 server", "Obsolete, insecure file-sharing server.", KeepOff),
    k("MicrosoftWindowsPowerShellV2Root", "Windows PowerShell 2.0", "Outdated PowerShell engine that can be used to bypass security logging. Keep off.", KeepOff),
    k("MicrosoftWindowsPowerShellV2", "Windows PowerShell 2.0 Engine", "Outdated PowerShell engine. Keep off.", KeepOff),
    k("TelnetClient", "Telnet Client", "Unencrypted remote terminal client. Only for network troubleshooting.", KeepOff),
    k("TFTP", "TFTP Client", "Trivial file transfer client for network equipment.", KeepOff),
    k("IIS-WebServerRole", "Internet Information Services", "Web server. Keep off unless you develop or host websites.", KeepOff),
    k("WorkFolders-Client", "Work Folders Client", "Syncs files with a company Work Folders server.", Either),
    k("MediaPlayback", "Media Features", "Legacy Windows Media components. Some apps need them for audio/video playback.", KeepOn),
    k("WindowsMediaPlayer", "Windows Media Player Legacy", "The classic Windows Media Player.", Either),
    k("Printing-PrintToPDFServices-Features", "Microsoft Print to PDF", "The 'Microsoft Print to PDF' printer used to save anything as PDF.", KeepOn),
    k("Printing-XPSServices-Features", "XPS Document Writer", "Prints documents to the XPS format.", Either),
    k("Printing-Foundation-Features", "Print and Document Services", "Additional printing services; not needed for normal printing.", Either),
    k("SearchEngine-Client-Package", "Windows Search", "Indexing behind Start, File Explorer and Outlook search.", Protected),
    k("MSRDC-Infrastructure", "Remote Differential Compression", "Used by some Windows sync features and apps.", KeepOn),
    k("Windows-Defender-Default-Definitions", "Microsoft Defender definitions", "Baseline malware definitions for Microsoft Defender.", Protected),
    k("Windows-Defender-ApplicationGuard", "Microsoft Defender Application Guard", "Isolated browsing for untrusted sites (retired by Microsoft).", Either),
    k("Recall", "Recall", "Saves periodic snapshots of your screen so you can search past activity (Copilot+ PCs). Privacy-sensitive.", Either),
    k("Client-ProjFS", "Projected File System", "Used by some developer tools (e.g. Git virtual file systems).", Either),
    k("Microsoft-RemoteDesktopConnection", "Remote Desktop Connection", "The classic Remote Desktop client.", Either),
    k("Containers", "Containers", "Windows containers for developers.", Either),
];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawFeature {
    name: String,
    caption: Option<String>,
    state: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Feature {
    pub name: String,
    pub title: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub advice: Option<FeatureAdvice>,
    /// Listed with an explanation (shown by default); others appear under "show all".
    pub common: bool,
}

const SCRIPT: &str = r#"$ErrorActionPreference = 'Stop'
$f = @(Get-CimInstance Win32_OptionalFeature | Where-Object { $_.InstallState -in 1, 2 } | ForEach-Object {
    [ordered]@{ name = [string]$_.Name; caption = [string]$_.Caption; state = [int]$_.InstallState }
})
ConvertTo-Json -InputObject $f -Compress"#;

pub fn list() -> AppResult<Vec<Feature>> {
    #[derive(Deserialize)]
    struct Wrap(#[serde(deserialize_with = "one_or_many")] Vec<RawFeature>);
    let Wrap(raw) = powershell::run_json(SCRIPT, Duration::from_secs(60))?;
    let mut out: Vec<Feature> = raw
        .into_iter()
        .map(|r| {
            let known = KNOWN.iter().find(|k| k.name.eq_ignore_ascii_case(&r.name));
            Feature {
                title: known.map(|k| k.title.to_string()).or(r.caption.filter(|c| !c.is_empty())).unwrap_or_else(|| r.name.clone()),
                description: known.map(|k| k.description.to_string()),
                enabled: r.state == 1,
                advice: known.map(|k| k.advice),
                common: known.is_some(),
                name: r.name,
            }
        })
        .collect();
    out.sort_by(|a, b| b.common.cmp(&a.common).then(a.title.to_lowercase().cmp(&b.title.to_lowercase())));
    Ok(out)
}

pub fn advice(name: &str) -> Option<FeatureAdvice> {
    KNOWN.iter().find(|k| k.name.eq_ignore_ascii_case(name)).map(|k| k.advice)
}

pub fn valid_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= 100 && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

pub fn toggle_script(name: &str, enable: bool) -> String {
    let verb = if enable { "Enable" } else { "Disable" };
    let all = if enable { " -All" } else { "" };
    format!(
        r#"'##PROGRESS 10 {verb} {name}'
$r = {verb}-WindowsOptionalFeature -Online -FeatureName '{name}' -NoRestart{all} -ErrorAction Stop
if ($r.RestartNeeded) {{ 'Done. Restart Windows to finish this change.' }} else {{ 'Done. No restart needed.' }}"#
    )
}
