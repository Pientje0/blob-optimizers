//! Safety screening for commands typed into the Terminal tab.
//!
//! Catalog operations are reviewed in advance; free-form commands are checked here. Catastrophic
//! patterns are blocked outright, risky ones need explicit confirmation with the reason shown.

use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;

use super::types::Shell;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Ok,
    Warn,
    Block,
}

#[derive(Debug, Clone, Serialize)]
pub struct Validation {
    pub level: Level,
    pub reasons: Vec<String>,
}

struct Rule {
    level: Level,
    pattern: Regex,
    reason: &'static str,
}

fn rule(level: Level, pattern: &str, reason: &'static str) -> Rule {
    Rule { level, pattern: Regex::new(&format!("(?i){pattern}")).expect("valid rule regex"), reason }
}

const DELETE: &str = r"(?:\b(?:remove-item|rm|ri|del|erase|rd|rmdir|rmdir\.exe)\b)";
/// Drive roots and core system / profile folders, in literal or environment-variable form.
const CRITICAL_PATH: &str = r#"(?:\b[a-z]:\\?(?:\*|["'\s]|$)|\b[a-z]:\\(?:windows|program files(?: \(x86\))?|programdata|users|boot|recovery)\\?(?:\*|["'\s]|$)|(?:\$env:|%)(?:windir|systemroot|systemdrive|programfiles|programdata|userprofile)%?\\?(?:\*|["'\s]|$)|\\system32\b)"#;

static RULES: LazyLock<Vec<Rule>> = LazyLock::new(|| {
    use Level::*;
    vec![
        // ---- Blocked: destroys data or makes Windows unbootable ----
        rule(Block, r"\bformat(?:\.com)?\s+[a-z]:", "Formats a drive, erasing all data on it."),
        rule(Block, r"\b(?:format-volume|clear-disk|initialize-disk|remove-partition|diskpart)\b", "Modifies disks or partitions and can erase data."),
        rule(Block, &format!(r"{DELETE}.*{CRITICAL_PATH}"), "Deletes a drive root or a core Windows/user folder."),
        rule(Block, r"\b(?:vssadmin\b.*\bdelete\b|wmic\b.*shadowcopy\b.*\bdelete\b|wbadmin\b.*\bdelete\b)", "Deletes restore points / shadow copies, removing your ability to roll back."),
        rule(Block, r"\bbcdedit\b.*/(?:delete|deletevalue|set\b.*\b(?:recoveryenabled|bootstatuspolicy))", "Changes boot configuration in a way that can prevent Windows from starting."),
        rule(Block, r"\bcipher\b.*\s/w\b", "Overwrites all free disk space (irreversible and very long)."),
        rule(Block, r"\b(?:takeown|icacls)\b.*\\(?:windows|system32)\b", "Changes ownership/permissions of Windows system files."),
        rule(Block, r"\breg(?:\.exe)?\s+delete\s+hk(?:lm|ey_local_machine)\\(?:system|software)\\?(?:\s|$)", "Deletes an entire core registry hive."),
        // ---- Warn: security features ----
        rule(Warn, r"\bset-mppreference\b.*-disable", "Turns off a Microsoft Defender protection feature."),
        rule(Warn, r"\badd-mppreference\b.*-exclusion", "Adds a Microsoft Defender exclusion; files there will not be scanned."),
        rule(Warn, r"\bnetsh\b.*firewall\b.*\b(?:off|disable)\b|\bset-netfirewallprofile\b.*-enabled\s+(?:\$?false|0)", "Turns off Windows Firewall."),
        rule(Warn, r"\b(?:wuauserv|usosvc|waasmedicsvc|bits)\b", "Affects the Windows Update services; disabling them stops security updates."),
        rule(Warn, r"\b(?:windefend|wscsvc|securityhealthservice|mpssvc)\b", "Affects a Windows security service."),
        // ---- Warn: destructive or system-changing ----
        rule(Warn, &format!(r"{DELETE}.*(?:\*|\?|-recurse|\s/s\b)"), "Deletes files using wildcards or recursion."),
        rule(Warn, r"\b(?:reg(?:\.exe)?\s+(?:delete|add|import)|remove-item\b.*\bhk(?:lm|cu|cr|u)\b|remove-itemproperty|set-itemproperty|new-itemproperty)\b", "Changes the registry."),
        rule(Warn, r"\b(?:stop-process|taskkill|kill)\b", "Terminates processes; unsaved work in them is lost."),
        rule(Warn, r"\b(?:sc(?:\.exe)?\s+(?:delete|config|stop)|set-service|stop-service|remove-service)\b", "Changes or stops Windows services."),
        rule(Warn, r"\b(?:shutdown|restart-computer|stop-computer|logoff)\b", "Restarts, shuts down or signs out of Windows."),
        rule(Warn, r"\b(?:remove-appxpackage|remove-appxprovisionedpackage|uninstall-package|winget\s+uninstall|msiexec\b.*/x)\b", "Uninstalls software."),
        rule(Warn, r"\b(?:disable-windowsoptionalfeature|dism\b.*/(?:disable-feature|remove-package))", "Removes or disables Windows components."),
        rule(Warn, r"\bbcdedit\b", "Changes boot configuration."),
        rule(Warn, r"\b(?:iex|invoke-expression)\b.*\b(?:iwr|irm|invoke-webrequest|invoke-restmethod|downloadstring|net\.webclient)\b|\b(?:iwr|irm|invoke-webrequest|invoke-restmethod)\b.*\|\s*(?:iex|invoke-expression)\b", "Downloads and runs code from the internet."),
        rule(Warn, r"\bset-executionpolicy\b", "Changes the PowerShell script execution policy."),
        rule(Warn, r"\bnetsh\s+(?:winsock|int(?:erface)?\s+ip)\s+reset\b", "Resets network stack settings (restart required)."),
    ]
});

pub fn validate(shell: Shell, command: &str) -> Validation {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return Validation { level: Level::Block, reasons: vec!["Enter a command.".into()] };
    }
    if shell == Shell::Cmd && trimmed.contains(['\r', '\n']) {
        return Validation { level: Level::Block, reasons: vec!["CMD commands must be a single line.".into()] };
    }
    // Normalise whitespace so spacing tricks do not slip past the rules.
    let normalised = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut level = Level::Ok;
    let mut reasons = Vec::new();
    for r in RULES.iter() {
        if r.pattern.is_match(&normalised) {
            level = level.max(r.level);
            if !reasons.contains(&r.reason.to_string()) {
                reasons.push(r.reason.to_string());
            }
        }
    }
    if level == Level::Block {
        reasons.retain(|r| RULES.iter().any(|x| x.level == Level::Block && x.reason == r));
    }
    Validation { level, reasons }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lvl(cmd: &str) -> Level {
        validate(Shell::PowerShell, cmd).level
    }

    #[test]
    fn harmless_commands_pass() {
        for c in ["Get-Process", "ipconfig /all", "Get-ChildItem C:\\Users", "ping 1.1.1.1", "Get-Service wuauser*x"] {
            assert_eq!(lvl(c), Level::Ok, "{c}");
        }
    }

    #[test]
    fn catastrophic_commands_are_blocked() {
        for c in [
            "format C:",
            "Remove-Item C:\\ -Recurse -Force",
            "rd /s /q C:\\Windows",
            "Remove-Item  $env:windir\\*  -Recurse",
            "del /f /s /q %systemroot%\\*",
            "vssadmin delete shadows /all",
            "Clear-Disk -Number 0",
            "takeown /f C:\\Windows\\System32",
            "Remove-Item C:\\Users -Recurse",
        ] {
            assert_eq!(lvl(c), Level::Block, "{c}");
        }
    }

    #[test]
    fn risky_commands_need_confirmation() {
        for c in [
            "Set-MpPreference -DisableRealtimeMonitoring $true",
            "netsh advfirewall set allprofiles state off",
            "Stop-Service wuauserv",
            "Remove-Item C:\\Temp\\* -Recurse",
            "reg add HKCU\\Software\\X /v Y /d 1",
            "irm https://example.com/x.ps1 | iex",
            "Stop-Process -Name notepad",
            "Get-AppxPackage *xbox* | Remove-AppxPackage",
        ] {
            assert_eq!(lvl(c), Level::Warn, "{c}");
        }
    }
}
