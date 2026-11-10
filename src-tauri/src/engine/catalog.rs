//! Predefined, reviewed operations. System modifications in the app only ever run commands from
//! catalogs like this one, with parameters validated before substitution.

use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::Duration;

use regex::Regex;
use serde::Serialize;

use super::types::{JobRequest, Shell, Source};
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
#[allow(dead_code)] // all variants are part of the catalog vocabulary used by later modules
pub enum Risk {
    /// Read-only or trivially reversible.
    Safe,
    /// Changes system state; reversible or low impact.
    Caution,
    /// Significant change; read the explanation first.
    High,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
#[allow(dead_code)]
pub enum ParamKind {
    /// Hostname, IPv4 or IPv6 address.
    Host,
    Int { min: i64, max: i64 },
    Choice { options: &'static [&'static str] },
    /// Short free text limited to letters, digits, spaces and `.,_()-:` (safe inside quotes).
    Text { max: usize },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Param {
    pub name: &'static str,
    pub label: &'static str,
    pub kind: ParamKind,
    pub default: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Operation {
    pub id: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    /// What happens to the system, including side effects and whether it can be undone.
    pub effects: &'static str,
    pub category: &'static str,
    pub shell: Shell,
    pub script: &'static str,
    pub requires_admin: bool,
    pub modifies_system: bool,
    pub cancellable: bool,
    pub risk: Risk,
    pub timeout_secs: u64,
    pub duration_hint: &'static str,
    pub params: &'static [Param],
    #[serde(skip)]
    pub progress_pattern: Option<&'static str>,
}

const PERCENT: &str = r"(\d{1,3}(?:\.\d+)?)\s?%";

const HOST: Param = Param { name: "host", label: "Host or IP address", kind: ParamKind::Host, default: "1.1.1.1" };

pub static OPERATIONS: &[Operation] = &[
    // ---------------- Diagnostics (read-only) ----------------
    Operation {
        id: "diag.ipconfig",
        title: "Network configuration",
        description: "Shows every network adapter with its IP addresses, DNS servers, gateway and DHCP details.",
        effects: "Read-only. Nothing is changed.",
        category: "Diagnostics",
        shell: Shell::Cmd,
        script: "ipconfig /all",
        requires_admin: false,
        modifies_system: false,
        cancellable: true,
        risk: Risk::Safe,
        timeout_secs: 60,
        duration_hint: "Instant",
        params: &[],
        progress_pattern: None,
    },
    Operation {
        id: "diag.ping",
        title: "Ping",
        description: "Measures round-trip latency and packet loss to a host.",
        effects: "Read-only. Sends a few ICMP echo packets.",
        category: "Diagnostics",
        shell: Shell::Cmd,
        script: "ping -n {count} {host}",
        requires_admin: false,
        modifies_system: false,
        cancellable: true,
        risk: Risk::Safe,
        timeout_secs: 120,
        duration_hint: "A few seconds",
        params: &[HOST, Param { name: "count", label: "Packets", kind: ParamKind::Int { min: 1, max: 50 }, default: "4" }],
        progress_pattern: None,
    },
    Operation {
        id: "diag.tracert",
        title: "Trace route",
        description: "Lists each network hop between this PC and a host, useful for locating where latency is introduced.",
        effects: "Read-only.",
        category: "Diagnostics",
        shell: Shell::Cmd,
        script: "tracert -d -h 20 {host}",
        requires_admin: false,
        modifies_system: false,
        cancellable: true,
        risk: Risk::Safe,
        timeout_secs: 300,
        duration_hint: "Up to a minute",
        params: &[HOST],
        progress_pattern: None,
    },
    Operation {
        id: "diag.systeminfo",
        title: "System information",
        description: "Windows' own summary: OS build, install date, boot time, BIOS, memory, hotfixes and network cards.",
        effects: "Read-only.",
        category: "Diagnostics",
        shell: Shell::Cmd,
        script: "systeminfo",
        requires_admin: false,
        modifies_system: false,
        cancellable: true,
        risk: Risk::Safe,
        timeout_secs: 120,
        duration_hint: "A few seconds",
        params: &[],
        progress_pattern: None,
    },
    Operation {
        id: "diag.power_plans",
        title: "List power plans",
        description: "Lists installed power plans; the active one is marked with *.",
        effects: "Read-only.",
        category: "Diagnostics",
        shell: Shell::Cmd,
        script: "powercfg /list",
        requires_admin: false,
        modifies_system: false,
        cancellable: true,
        risk: Risk::Safe,
        timeout_secs: 30,
        duration_hint: "Instant",
        params: &[],
        progress_pattern: None,
    },
    Operation {
        id: "diag.running_services",
        title: "Running services",
        description: "Lists every Windows service that is currently running, with its startup type.",
        effects: "Read-only.",
        category: "Diagnostics",
        shell: Shell::PowerShell,
        script: "Get-Service | Where-Object Status -eq 'Running' | Sort-Object DisplayName | Format-Table -AutoSize Name, DisplayName, StartType",
        requires_admin: false,
        modifies_system: false,
        cancellable: true,
        risk: Risk::Safe,
        timeout_secs: 60,
        duration_hint: "A few seconds",
        params: &[],
        progress_pattern: None,
    },
    Operation {
        id: "diag.dns_cache",
        title: "View DNS cache",
        description: "Shows the names Windows has recently resolved and cached.",
        effects: "Read-only.",
        category: "Diagnostics",
        shell: Shell::PowerShell,
        script: "Get-DnsClientCache | Sort-Object Entry -Unique | Format-Table -AutoSize Entry, Type, TimeToLive, Data",
        requires_admin: false,
        modifies_system: false,
        cancellable: true,
        risk: Risk::Safe,
        timeout_secs: 60,
        duration_hint: "Instant",
        params: &[],
        progress_pattern: None,
    },
    // ---------------- Maintenance ----------------
    Operation {
        id: "maint.flush_dns",
        title: "Flush DNS cache",
        description: "Clears cached name lookups so Windows asks the DNS server again. Fixes sites that moved or stale lookups after changing DNS.",
        effects: "Clears the DNS resolver cache. The cache rebuilds automatically as you browse; nothing else changes.",
        category: "Maintenance",
        shell: Shell::Cmd,
        script: "ipconfig /flushdns",
        requires_admin: false,
        modifies_system: true,
        cancellable: false,
        risk: Risk::Safe,
        timeout_secs: 60,
        duration_hint: "Instant",
        params: &[],
        progress_pattern: None,
    },
    Operation {
        id: "maint.restart_explorer",
        title: "Restart File Explorer",
        description: "Restarts the Windows shell (taskbar, Start and File Explorer) so interface tweaks take effect without signing out.",
        effects: "The taskbar and open File Explorer windows close for a few seconds and reopen. Other apps are not affected.",
        category: "Maintenance",
        shell: Shell::PowerShell,
        script: "Stop-Process -Name explorer -Force -ErrorAction SilentlyContinue\nStart-Sleep -Seconds 2\nif (-not (Get-Process -Name explorer -ErrorAction SilentlyContinue)) { Start-Process explorer.exe }\n'File Explorer restarted.'",
        requires_admin: false,
        modifies_system: false,
        cancellable: false,
        risk: Risk::Safe,
        timeout_secs: 60,
        duration_hint: "A few seconds",
        params: &[],
        progress_pattern: None,
    },
    // ---------------- Network resets ----------------
    Operation {
        id: "net.renew_ip",
        title: "Release and renew IP address",
        description: "Asks the router for a fresh IP address on every adapter. Fixes '169.254' addresses and some 'no internet' problems.",
        effects: "All connections drop for a few seconds while the address is renewed. Nothing is changed permanently.",
        category: "Network",
        shell: Shell::Cmd,
        script: "ipconfig /release & ipconfig /renew",
        requires_admin: true,
        modifies_system: true,
        cancellable: false,
        risk: Risk::Safe,
        timeout_secs: 180,
        duration_hint: "Under a minute",
        params: &[],
        progress_pattern: None,
    },
    Operation {
        id: "net.reset_winsock",
        title: "Reset Winsock",
        description: "Resets the Windows network socket catalogue, which can fix connectivity broken by VPNs, proxies or network software.",
        effects: "Removes third-party network layers (some VPN, proxy and filtering software must be reinstalled or repaired afterwards). Requires a restart. Not reversible.",
        category: "Network",
        shell: Shell::Cmd,
        script: "netsh winsock reset",
        requires_admin: true,
        modifies_system: true,
        cancellable: false,
        risk: Risk::High,
        timeout_secs: 120,
        duration_hint: "A few seconds + restart",
        params: &[],
        progress_pattern: None,
    },
    Operation {
        id: "net.reset_tcpip",
        title: "Reset TCP/IP settings",
        description: "Restores the TCP/IP stack's registry settings to Windows defaults.",
        effects: "Manually set IP addresses, gateways and DNS servers are cleared (adapters go back to automatic). Requires a restart. Not reversible.",
        category: "Network",
        shell: Shell::Cmd,
        script: "netsh int ip reset & netsh int ipv6 reset",
        requires_admin: true,
        modifies_system: true,
        cancellable: false,
        risk: Risk::High,
        timeout_secs: 120,
        duration_hint: "A few seconds + restart",
        params: &[],
        progress_pattern: None,
    },
    // ---------------- Restore points ----------------
    Operation {
        id: "restore.create_point",
        title: "Create restore point",
        description: "Creates a Windows System Restore point you can roll back to if a later change causes problems.",
        effects: "Adds a restore point (uses some disk space reserved for System Protection). Windows normally allows one per 24 hours; Blob Optimizers lifts that limit only for this request and puts your setting back afterwards. Needs System Protection to be on for the system drive.",
        category: "Restore",
        shell: Shell::PowerShell,
        script: r#"$key = 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\SystemRestore'
$prev = (Get-ItemProperty -LiteralPath $key -Name SystemRestorePointCreationFrequency -ErrorAction SilentlyContinue).SystemRestorePointCreationFrequency
try {
    Set-ItemProperty -LiteralPath $key -Name SystemRestorePointCreationFrequency -Value 0 -Type DWord
    '##PROGRESS 20 Creating restore point'
    Checkpoint-Computer -Description 'Blob Optimizers: {description}' -RestorePointType MODIFY_SETTINGS -ErrorAction Stop
    $rp = Get-ComputerRestorePoint | Sort-Object SequenceNumber | Select-Object -Last 1
    "Created restore point #$($rp.SequenceNumber): $($rp.Description)"
} catch {
    if ($_.Exception.Message -match '(?i)disabled|service cannot be started|0x80042306') {
        throw 'System Protection is turned off for the system drive, so no restore point could be created. Run "Turn on System Protection" first.'
    }
    throw
} finally {
    if ($null -eq $prev) { Remove-ItemProperty -LiteralPath $key -Name SystemRestorePointCreationFrequency -ErrorAction SilentlyContinue }
    else { Set-ItemProperty -LiteralPath $key -Name SystemRestorePointCreationFrequency -Value $prev -Type DWord }
}"#,
        requires_admin: true,
        modifies_system: true,
        cancellable: false,
        risk: Risk::Safe,
        timeout_secs: 900,
        duration_hint: "Under a minute",
        params: &[Param { name: "description", label: "Description", kind: ParamKind::Text { max: 60 }, default: "Manual restore point" }],
        progress_pattern: None,
    },
    Operation {
        id: "restore.enable_protection",
        title: "Turn on System Protection",
        description: "Enables System Restore for the system drive so restore points can be created.",
        effects: "Turns on System Protection for the Windows drive. Windows reserves a share of disk space (usually a few percent) for restore points. Can be turned off again from Windows' System Protection settings.",
        category: "Restore",
        shell: Shell::PowerShell,
        script: "Enable-ComputerRestore -Drive \"$env:SystemDrive\\\"\n\"System Protection is on for $env:SystemDrive\"",
        requires_admin: true,
        modifies_system: true,
        cancellable: false,
        risk: Risk::Caution,
        timeout_secs: 300,
        duration_hint: "A few seconds",
        params: &[],
        progress_pattern: None,
    },
    // ---------------- Repair ----------------
    Operation {
        id: "repair.dism_check",
        title: "Check Windows image health",
        description: "Quickly checks whether the Windows component store has already been flagged as corrupted.",
        effects: "Read-only. Does not scan or repair anything.",
        category: "Repair",
        shell: Shell::Cmd,
        script: "DISM /Online /Cleanup-Image /CheckHealth",
        requires_admin: true,
        modifies_system: false,
        cancellable: true,
        risk: Risk::Safe,
        timeout_secs: 600,
        duration_hint: "Under a minute",
        params: &[],
        progress_pattern: Some(PERCENT),
    },
    Operation {
        id: "repair.dism_scan",
        title: "Scan Windows image",
        description: "Fully scans the Windows component store for corruption without repairing it.",
        effects: "Read-only. Uses noticeable disk and CPU while it runs.",
        category: "Repair",
        shell: Shell::Cmd,
        script: "DISM /Online /Cleanup-Image /ScanHealth",
        requires_admin: true,
        modifies_system: false,
        cancellable: true,
        risk: Risk::Safe,
        timeout_secs: 3600,
        duration_hint: "5–20 minutes",
        params: &[],
        progress_pattern: Some(PERCENT),
    },
    Operation {
        id: "repair.sfc_verify",
        title: "Verify system files",
        description: "Checks protected Windows system files for corruption and reports problems without changing anything.",
        effects: "Read-only. Results are also written to C:\\Windows\\Logs\\CBS\\CBS.log.",
        category: "Repair",
        shell: Shell::Cmd,
        script: "sfc /verifyonly",
        requires_admin: true,
        modifies_system: false,
        cancellable: true,
        risk: Risk::Safe,
        timeout_secs: 3600,
        duration_hint: "5–15 minutes",
        params: &[],
        progress_pattern: Some(PERCENT),
    },
    Operation {
        id: "repair.dism_restore",
        title: "Repair Windows image",
        description: "Repairs the Windows component store using Windows Update as the source. Run this before 'Repair system files' if corruption was found.",
        effects: "Replaces corrupted Windows components with clean copies downloaded from Microsoft. Needs an internet connection. Repairs are not reversible, but only restore original Microsoft files. Cannot be cancelled once started.",
        category: "Repair",
        shell: Shell::Cmd,
        script: "DISM /Online /Cleanup-Image /RestoreHealth",
        requires_admin: true,
        modifies_system: true,
        cancellable: false,
        risk: Risk::Caution,
        timeout_secs: 5400,
        duration_hint: "10–30 minutes",
        params: &[],
        progress_pattern: Some(PERCENT),
    },
    Operation {
        id: "repair.sfc_scan",
        title: "Repair system files",
        description: "Scans protected Windows system files and replaces corrupted ones with the cached original copies.",
        effects: "Replaces corrupted system files with Microsoft's originals. A restart may be required. Cannot be cancelled once started.",
        category: "Repair",
        shell: Shell::Cmd,
        script: "sfc /scannow",
        requires_admin: true,
        modifies_system: true,
        cancellable: false,
        risk: Risk::Caution,
        timeout_secs: 5400,
        duration_hint: "5–20 minutes",
        params: &[],
        progress_pattern: Some(PERCENT),
    },
];

static HOST_RE: LazyLock<Regex> = LazyLock::new(|| {
    // Hostname labels, or an IPv4/IPv6 literal. Never starts with '-' so it cannot become a switch.
    Regex::new(r"^(?:[A-Za-z0-9](?:[A-Za-z0-9-]{0,61}[A-Za-z0-9])?(?:\.[A-Za-z0-9](?:[A-Za-z0-9-]{0,61}[A-Za-z0-9])?)*|[0-9A-Fa-f:.]{2,45})$").unwrap()
});

static TEXT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[\p{L}\p{N} .,_()\-:]+$").unwrap());

pub fn find(id: &str) -> Option<&'static Operation> {
    OPERATIONS.iter().find(|o| o.id == id)
}

fn validate_param(p: &Param, value: &str) -> AppResult<String> {
    let v = value.trim();
    let bad = |why: &str| AppError::Other(format!("{}: {why}", p.label));
    match &p.kind {
        ParamKind::Host => {
            if v.len() > 253 || !HOST_RE.is_match(v) {
                return Err(bad("enter a valid hostname or IP address"));
            }
        }
        ParamKind::Int { min, max } => {
            let n: i64 = v.parse().map_err(|_| bad("must be a whole number"))?;
            if n < *min || n > *max {
                return Err(bad(&format!("must be between {min} and {max}")));
            }
            return Ok(n.to_string());
        }
        ParamKind::Choice { options } => {
            if !options.contains(&v) {
                return Err(bad("not an allowed option"));
            }
        }
        ParamKind::Text { max } => {
            if v.is_empty() || v.chars().count() > *max || !TEXT_RE.is_match(v) {
                return Err(bad(&format!("use up to {max} letters, digits, spaces or . , _ ( ) - :")));
            }
        }
    }
    Ok(v.to_string())
}

/// Validates parameters and produces the job request for a catalog operation.
pub fn request(id: &str, params: &HashMap<String, String>) -> AppResult<JobRequest> {
    let op = find(id).ok_or_else(|| AppError::Other(format!("Unknown operation '{id}'")))?;
    if let Some(unknown) = params.keys().find(|k| !op.params.iter().any(|p| p.name == k.as_str())) {
        return Err(AppError::Other(format!("Unexpected parameter '{unknown}'")));
    }
    let mut script = op.script.to_string();
    for p in op.params {
        let raw = params.get(p.name).map(String::as_str).unwrap_or(p.default);
        let value = validate_param(p, raw)?;
        script = script.replace(&format!("{{{}}}", p.name), &value);
    }
    Ok(JobRequest {
        title: op.title.to_string(),
        source: Source::Catalog,
        operation_id: Some(op.id.to_string()),
        shell: op.shell,
        command: script,
        requires_admin: op.requires_admin,
        modifies_system: op.modifies_system,
        cancellable: op.cancellable,
        timeout: Duration::from_secs(op.timeout_secs),
        progress_pattern: op.progress_pattern,
        kill_tree: true,
        on_complete: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_params_are_substituted() {
        let mut ids: Vec<_> = OPERATIONS.iter().map(|o| o.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), OPERATIONS.len());
        for op in OPERATIONS {
            let req = request(op.id, &HashMap::new()).expect(op.id);
            for p in op.params {
                assert!(!req.command.contains(&format!("{{{}}}", p.name)), "{} left a placeholder", op.id);
            }
        }
    }

    #[test]
    fn rejects_injection_in_parameters() {
        for bad in ["1.1.1.1 & del C:\\x", "-t", "a;b", "$(calc)", "host name", "a\"b", ""] {
            let p = HashMap::from([("host".to_string(), bad.to_string())]);
            assert!(request("diag.ping", &p).is_err(), "accepted {bad:?}");
        }
        let p = HashMap::from([("host".to_string(), "one.one.one.one".to_string()), ("count".to_string(), "3".to_string())]);
        assert_eq!(request("diag.ping", &p).unwrap().command, "ping -n 3 one.one.one.one");
        let p = HashMap::from([("count".to_string(), "999".to_string())]);
        assert!(request("diag.ping", &p).is_err());
        let p = HashMap::from([("evil".to_string(), "x".to_string())]);
        assert!(request("diag.ping", &p).is_err());
        for bad in ["x' ; Remove-Item C:\\", "a\"b", "$(x)", "`n"] {
            let p = HashMap::from([("description".to_string(), bad.to_string())]);
            assert!(request("restore.create_point", &p).is_err(), "accepted {bad:?}");
        }
        let p = HashMap::from([("description".to_string(), "Before removing Xbox app (2)".to_string())]);
        assert!(request("restore.create_point", &p).unwrap().command.contains("Blob Optimizers: Before removing Xbox app (2)"));
    }
}
