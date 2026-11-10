//! System settings: power plans and timeouts, hibernation, virtual memory, storage cleanup.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::engine::types::{JobRequest, Shell};
use crate::error::{AppError, AppResult};
use crate::util::powershell;
use crate::util::serde_helpers::one_or_many;

const INFO: &str = include_str!("../../scripts/system-info.ps1");
const CLEANUP: &str = include_str!("../../scripts/cleanup.ps1");

pub const HIGH_PERFORMANCE: &str = "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c";
pub const ULTIMATE_PERFORMANCE: &str = "e9a42b02-d5df-448d-aa00-03f14749eb61";
const BUILT_IN: &[&str] = &["381b4222-f694-41f0-9685-ff5bb260df2e", "a1841308-3541-4fab-bc81-f71556f20b4a", HIGH_PERFORMANCE];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PowerPlan {
    pub guid: String,
    pub name: String,
    pub active: bool,
    #[serde(default)]
    pub built_in: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageFile {
    pub path: String,
    pub allocated_mb: u32,
    pub current_usage_mb: u32,
    pub peak_usage_mb: u32,
    pub initial_mb: Option<u32>,
    pub maximum_mb: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageFileInfo {
    pub automatic: bool,
    #[serde(deserialize_with = "one_or_many")]
    pub files: Vec<PageFile>,
    pub ram_mb: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    #[serde(deserialize_with = "one_or_many")]
    pub plans: Vec<PowerPlan>,
    pub monitor_ac: Option<u32>,
    pub monitor_dc: Option<u32>,
    pub sleep_ac: Option<u32>,
    pub sleep_dc: Option<u32>,
    pub has_battery: bool,
    pub hibernate: bool,
    pub hiberfil_bytes: Option<u64>,
    pub pagefile: PageFileInfo,
}

pub fn info() -> AppResult<SystemInfo> {
    let mut i: SystemInfo = powershell::run_json(INFO, Duration::from_secs(60))?;
    for p in &mut i.plans {
        p.built_in = BUILT_IN.contains(&p.guid.as_str());
    }
    Ok(i)
}

fn err(m: impl Into<String>) -> AppError {
    AppError::Other(m.into())
}

fn job(title: impl Into<String>, op: &str, script: String, admin: bool) -> JobRequest {
    let mut r = JobRequest::internal(title, Shell::PowerShell, script);
    r.operation_id = Some(op.into());
    r.requires_admin = admin;
    r.timeout = Duration::from_secs(10 * 60);
    r
}

fn known_plan(guid: &str) -> AppResult<PowerPlan> {
    let g = guid.to_ascii_lowercase();
    info()?.plans.into_iter().find(|p| p.guid == g).ok_or_else(|| err("That power plan no longer exists."))
}

pub fn set_active_plan(guid: &str) -> AppResult<JobRequest> {
    let p = known_plan(guid)?;
    Ok(job(format!("Switch power plan to {}", p.name), "power.set_active", format!("powercfg /setactive {}\nif ($LASTEXITCODE) {{ exit $LASTEXITCODE }}\n'Active power plan: {}'", p.guid, p.name.replace('\'', "''")), false))
}

/// Adds High performance or Ultimate Performance (hidden on many PCs) and activates it.
pub fn add_plan(which: &str) -> AppResult<JobRequest> {
    let (source, name) = match which {
        "high" => (HIGH_PERFORMANCE, "High performance"),
        "ultimate" => (ULTIMATE_PERFORMANCE, "Ultimate Performance"),
        _ => return Err(err("Unknown plan.")),
    };
    let script = format!(
        r#"$out = powercfg -duplicatescheme {source} 2>&1
if ($LASTEXITCODE) {{ [Console]::Error.WriteLine("Windows did not allow adding this plan: $out"); [Console]::Error.WriteLine('Some PCs with Modern Standby only support the Balanced plan.'); exit 1 }}
if ("$out" -match '([0-9a-fA-F]{{8}}-[0-9a-fA-F-]{{27}})') {{
    powercfg /setactive $Matches[1]
    "Added and activated {name} ($($Matches[1]))"
}}"#
    );
    Ok(job(format!("Add {name} power plan"), "power.add_plan", script, false))
}

pub fn delete_plan(guid: &str) -> AppResult<JobRequest> {
    let p = known_plan(guid)?;
    if p.active {
        return Err(err("Switch to another plan before deleting the active one."));
    }
    if p.built_in || p.guid == "381b4222-f694-41f0-9685-ff5bb260df2e" {
        return Err(err("Built-in power plans are not deleted."));
    }
    Ok(job(format!("Delete power plan {}", p.name), "power.delete_plan", format!("powercfg /delete {}\nif ($LASTEXITCODE) {{ exit $LASTEXITCODE }}\n'Deleted.'", p.guid), false))
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Timeouts {
    pub monitor_ac: u32,
    pub sleep_ac: u32,
    pub monitor_dc: Option<u32>,
    pub sleep_dc: Option<u32>,
}

pub fn set_timeouts(t: Timeouts) -> AppResult<JobRequest> {
    let all = [Some(t.monitor_ac), Some(t.sleep_ac), t.monitor_dc, t.sleep_dc];
    if all.iter().flatten().any(|m| *m > 600) {
        return Err(err("Timeouts must be between 0 (never) and 600 minutes."));
    }
    let mut s = format!("powercfg /change monitor-timeout-ac {}\npowercfg /change standby-timeout-ac {}\n", t.monitor_ac, t.sleep_ac);
    if let Some(v) = t.monitor_dc {
        s.push_str(&format!("powercfg /change monitor-timeout-dc {v}\n"));
    }
    if let Some(v) = t.sleep_dc {
        s.push_str(&format!("powercfg /change standby-timeout-dc {v}\n"));
    }
    s.push_str("'Timeouts updated for the active power plan.'");
    Ok(job("Change screen and sleep timeouts", "power.timeouts", s, false))
}

pub fn set_hibernate(on: bool) -> AppResult<JobRequest> {
    let s = if on {
        "powercfg /hibernate on\nif ($LASTEXITCODE) { exit $LASTEXITCODE }\n'Hibernation turned on. Fast Startup is available again.'"
    } else {
        "powercfg /hibernate off\nif ($LASTEXITCODE) { exit $LASTEXITCODE }\n'Hibernation turned off and hiberfil.sys removed. Fast Startup is unavailable while hibernation is off.'"
    };
    Ok(job(if on { "Turn on hibernation" } else { "Turn off hibernation" }, "power.hibernate", s.into(), true))
}

pub fn restore_default_plans() -> JobRequest {
    job(
        "Restore default power plans",
        "power.restore_defaults",
        "powercfg -restoredefaultschemes\nif ($LASTEXITCODE) { exit $LASTEXITCODE }\n'Power plans reset to Windows defaults. Custom plans were removed.'".into(),
        true,
    )
}

/// Automatic page file, or a fixed size on the system drive. The page file is never removed entirely.
pub fn set_pagefile(automatic: bool, initial_mb: Option<u32>, maximum_mb: Option<u32>) -> AppResult<JobRequest> {
    let mut s = String::from("$cs = Get-CimInstance Win32_ComputerSystem\n");
    if automatic {
        s.push_str("Set-CimInstance -InputObject $cs -Property @{ AutomaticManagedPagefile = $true }\n'Windows now manages the page file size. Restart to apply.'");
        return Ok(job("Let Windows manage the page file", "system.pagefile", s, true));
    }
    let (init, max) = (initial_mb.ok_or_else(|| err("Enter an initial size."))?, maximum_mb.ok_or_else(|| err("Enter a maximum size."))?);
    let ram = info()?.pagefile.ram_mb.max(1024);
    if init < 1024 {
        return Err(err("Keep at least 1024 MB: without a page file, apps can crash when memory runs out and crash dumps cannot be saved."));
    }
    if max < init {
        return Err(err("The maximum size must be at least the initial size."));
    }
    if max > (ram * 4).min(262_144) {
        return Err(err(format!("The maximum size is limited to {} MB (4x installed RAM).", (ram * 4).min(262_144))));
    }
    s.push_str(&format!(
        r#"if ($cs.AutomaticManagedPagefile) {{ Set-CimInstance -InputObject $cs -Property @{{ AutomaticManagedPagefile = $false }} }}
$name = "$env:SystemDrive\pagefile.sys"
$pf = Get-CimInstance Win32_PageFileSetting | Where-Object {{ $_.Name -eq $name }}
if ($pf) {{ Set-CimInstance -InputObject $pf -Property @{{ InitialSize = [uint32]{init}; MaximumSize = [uint32]{max} }} }}
else {{ New-CimInstance -ClassName Win32_PageFileSetting -Property @{{ Name = $name; InitialSize = [uint32]{init}; MaximumSize = [uint32]{max} }} | Out-Null }}
"Page file on $env:SystemDrive set to {init}-{max} MB. Restart to apply.""#
    ));
    Ok(job(format!("Set page file to {init}-{max} MB"), "system.pagefile", s, true))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupItem {
    pub id: String,
    pub bytes: u64,
    pub files: u64,
}

pub const CLEANUP_IDS: &[&str] = &["userTemp", "winTemp", "thumbnails", "shaders", "errorReports", "updateLogs", "recycleBin"];
const ADMIN_CLEANUP: &[&str] = &["winTemp", "errorReports", "updateLogs"];

pub fn cleanup_scan() -> AppResult<Vec<CleanupItem>> {
    #[derive(Deserialize)]
    struct W(#[serde(deserialize_with = "one_or_many")] Vec<CleanupItem>);
    let script = CLEANUP.replace("__MODE__", "scan").replace("__SELECTED__", "");
    let W(items) = powershell::run_json(&script, Duration::from_secs(180))?;
    Ok(items)
}

pub fn cleanup(ids: &[String]) -> AppResult<JobRequest> {
    if ids.is_empty() {
        return Err(err("Nothing selected."));
    }
    if let Some(bad) = ids.iter().find(|i| !CLEANUP_IDS.contains(&i.as_str())) {
        return Err(err(format!("Unknown cleanup category '{bad}'.")));
    }
    let list = ids.iter().map(|i| format!("'{i}'")).collect::<Vec<_>>().join(",");
    let script = CLEANUP.replace("__MODE__", "clean").replace("__SELECTED__", &list);
    let admin = ids.iter().any(|i| ADMIN_CLEANUP.contains(&i.as_str()));
    let mut r = job("Clean up storage", "system.cleanup", script, admin);
    r.timeout = Duration::from_secs(30 * 60);
    Ok(r)
}
