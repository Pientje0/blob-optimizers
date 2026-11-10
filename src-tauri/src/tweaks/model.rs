//! Declarative tweak model. A tweak is two lists of operations: `on` (the tweak applied) and `off`
//! (the Windows default). State is detected by checking which list currently matches.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::util::powershell;
use crate::util::registry::{self, Hive, RegData};

/// A setting that is not a plain registry value (network adapter properties, power plan values, …).
/// `check` is reviewed PowerShell that returns `$true`/`$false` (or `$null` when the setting does not
/// exist on this PC, which hides the tweak); `apply` makes it true and must fail with an exception.
/// Checks run together in one PowerShell process after `CHECK_PRELUDE`, so they can use its helpers.
#[derive(Debug)]
pub struct Cmd {
    pub key: &'static str,
    pub check: &'static str,
    pub apply: &'static str,
    pub describe: &'static str,
    pub admin: bool,
}

/// Shared, read-only helpers for all checks (computed once per refresh).
const CHECK_PRELUDE: &str = r#"
$__nics = @(Get-NetAdapter -Physical -ErrorAction SilentlyContinue | Where-Object Status -ne 'Disabled')
$__adv = if ($__nics.Count) { @(Get-NetAdapterAdvancedProperty -Name $__nics.Name -RegistryKeyword '*EEE', '*InterruptModeration', '*FlowControl' -ErrorAction SilentlyContinue) } else { @() }
$__ifs = @(Get-ChildItem 'HKLM:\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces' -ErrorAction SilentlyContinue | Where-Object {
  $p = Get-ItemProperty $_.PSPath
  ($p.DhcpIPAddress -and $p.DhcpIPAddress -ne '0.0.0.0') -or (@($p.IPAddress) | Where-Object { $_ -and $_ -ne '0.0.0.0' })
})
function __adv($k, $v) {
  $p = @($__adv | Where-Object { $_.RegistryKeyword -eq $k })
  if (-not $p.Count) { return $null }
  -not ($p | Where-Object { "$($_.RegistryValue)" -ne $v })
}
function __pw($sub, $set, $want) {
  $o = powercfg /q SCHEME_CURRENT $sub $set 2>$null | Out-String
  if ($LASTEXITCODE -ne 0) { return $null }
  $h = @([regex]::Matches($o, '0x[0-9a-fA-F]{8}') | ForEach-Object Value)
  if ($h.Count -lt 2) { return $null }
  ([Convert]::ToInt32($h[-2], 16) -eq $want) -and ([Convert]::ToInt32($h[-1], 16) -eq $want)
}
"#;

/// Results of the last check run: key → Some(matches) or None (not available on this PC).
struct CmdCache {
    at: Instant,
    values: HashMap<String, Option<bool>>,
}

static CMD_CACHE: Mutex<Option<CmdCache>> = Mutex::new(None);
/// Changes made here clear the cache; this only bounds how long changes made elsewhere go unnoticed.
const CMD_TTL: Duration = Duration::from_secs(5 * 60);

/// Forgets cached check results (after a change), so the next listing reads them again.
pub fn invalidate_cmd_cache() {
    if let Ok(mut c) = CMD_CACHE.lock() {
        *c = None;
    }
}

/// Runs every command check in one hidden PowerShell process if the cache is missing or stale.
pub fn refresh_cmd_states(cmds: &[&'static Cmd]) {
    if cmds.is_empty() {
        return;
    }
    // One refresh at a time: a caller arriving during a refresh waits for it and then uses its result.
    static REFRESHING: Mutex<()> = Mutex::new(());
    let _running = REFRESHING.lock();
    if CMD_CACHE.lock().ok().and_then(|c| c.as_ref().map(|c| c.at.elapsed() < CMD_TTL)).unwrap_or(false) {
        return;
    }
    let mut script = String::from("$ErrorActionPreference = 'SilentlyContinue'\n");
    script.push_str(CHECK_PRELUDE);
    script.push_str("$__r = [ordered]@{}\n");
    for c in cmds {
        script.push_str(&format!(
            "try {{ $__v = & {{ {} }}; $__r['{}'] = if ($null -eq $__v) {{ $null }} else {{ [bool]$__v }} }} catch {{ $__r['{}'] = $null }}\n",
            c.check, c.key, c.key
        ));
    }
    script.push_str("$__r | ConvertTo-Json -Compress\n");
    let values: HashMap<String, Option<bool>> = powershell::run_json(&script, Duration::from_secs(45)).unwrap_or_default();
    if let Ok(mut c) = CMD_CACHE.lock() {
        *c = Some(CmdCache { at: Instant::now(), values });
    }
}

fn cmd_state(key: &str) -> Option<bool> {
    CMD_CACHE.lock().ok().and_then(|c| c.as_ref().and_then(|c| c.values.get(key).copied().flatten()))
}

#[derive(Debug, Clone, Copy)]
pub enum V {
    D(u32),
    S(&'static str),
}

impl V {
    pub fn data(self) -> RegData {
        match self {
            V::D(v) => RegData::Dword(v),
            V::S(s) => RegData::String(s.to_string()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum StartType {
    Automatic,
    Manual,
    Disabled,
}

impl StartType {
    pub fn registry_value(self) -> u32 {
        match self {
            StartType::Automatic => 2,
            StartType::Manual => 3,
            StartType::Disabled => 4,
        }
    }
    pub fn from_registry(v: u32) -> Option<Self> {
        match v {
            2 => Some(StartType::Automatic),
            3 => Some(StartType::Manual),
            4 => Some(StartType::Disabled),
            _ => None,
        }
    }
    pub fn ps(self) -> &'static str {
        match self {
            StartType::Automatic => "Automatic",
            StartType::Manual => "Manual",
            StartType::Disabled => "Disabled",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Op {
    /// Value is set to exactly this.
    Set(Hive, &'static str, &'static str, V),
    /// Windows default that may be stored or simply absent: matches either; applying writes the value.
    Opt(Hive, &'static str, &'static str, V),
    /// Value is absent.
    Del(Hive, &'static str, &'static str),
    /// Key is absent.
    DelKey(Hive, &'static str),
    /// Service start type.
    Svc(&'static str, StartType),
    /// Reviewed PowerShell check/apply pair.
    Cmd(&'static Cmd),
}

impl Op {
    pub fn needs_admin(&self) -> bool {
        match self {
            Op::Set(h, ..) | Op::Opt(h, ..) | Op::Del(h, ..) | Op::DelKey(h, ..) => *h == Hive::LocalMachine,
            Op::Svc(..) => true,
            Op::Cmd(c) => c.admin,
        }
    }

    pub fn matches(&self) -> bool {
        match *self {
            Op::Set(h, p, n, v) => registry::read(h, p, n).as_ref().is_some_and(|d| same(d, &v.data())),
            Op::Opt(h, p, n, v) => registry::read(h, p, n).as_ref().is_none_or(|d| same(d, &v.data())),
            Op::Del(h, p, n) => registry::read(h, p, n).is_none(),
            Op::DelKey(h, p) => !registry::key_exists(h, p),
            Op::Svc(name, start) => service_start(name) == Some(start),
            Op::Cmd(c) => cmd_state(c.key) == Some(true),
        }
    }

    /// PowerShell that makes this operation true.
    pub fn script(&self) -> String {
        match *self {
            Op::Set(h, p, n, v) | Op::Opt(h, p, n, v) => registry::ps_set(h, p, n, &v.data()),
            Op::Del(h, p, n) => registry::ps_delete_value(h, p, n),
            Op::DelKey(h, p) => registry::ps_delete_key(h, p),
            Op::Svc(name, start) => service_script(name, start),
            Op::Cmd(c) => format!("{}\n", c.apply),
        }
    }

    pub fn describe(&self) -> String {
        match *self {
            Op::Set(h, p, n, v) | Op::Opt(h, p, n, v) => format!("{}\\{p} › {} = {}", h.reg(), if n.is_empty() { "(Default)" } else { n }, show(v)),
            Op::Del(h, p, n) => format!("{}\\{p} › {n} removed", h.reg()),
            Op::DelKey(h, p) => format!("{}\\{p} removed", h.reg()),
            Op::Svc(name, start) => format!("Service {name} › {}", start.ps()),
            Op::Cmd(c) => c.describe.to_string(),
        }
    }
}

fn show(v: V) -> String {
    match v {
        V::D(d) => d.to_string(),
        V::S(s) => format!("\"{s}\""),
    }
}

/// Strings compare case-insensitively; numbers exactly.
fn same(a: &RegData, b: &RegData) -> bool {
    match (a, b) {
        (RegData::String(x) | RegData::ExpandString(x), RegData::String(y)) => x.eq_ignore_ascii_case(y),
        _ => a == b,
    }
}

pub fn service_start(name: &str) -> Option<StartType> {
    let path = format!(r"SYSTEM\CurrentControlSet\Services\{name}");
    match registry::read(Hive::LocalMachine, &path, "Start")? {
        RegData::Dword(v) => StartType::from_registry(v),
        _ => None,
    }
}

pub fn service_script(name: &str, start: StartType) -> String {
    let n = name.replace('\'', "''");
    let mut s = format!("Set-Service -Name '{n}' -StartupType {} -ErrorAction Stop\n", start.ps());
    match start {
        StartType::Disabled => s.push_str(&format!("Stop-Service -Name '{n}' -Force -ErrorAction SilentlyContinue\n")),
        StartType::Automatic => s.push_str(&format!("Start-Service -Name '{n}' -ErrorAction SilentlyContinue\n")),
        StartType::Manual => {}
    }
    s
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Category {
    Performance,
    Privacy,
    Windows,
    System,
    /// Shown on the Gaming page instead of Windows Tweaks.
    Gaming,
    /// Shown on the Network page.
    Network,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Recommend {
    /// Recommended to apply the tweak.
    Apply,
    /// Recommended to keep the Windows default.
    Keep,
    /// Personal preference.
    Either,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum After {
    Nothing,
    /// File Explorer / taskbar must restart to show the change.
    Explorer,
    SignOut,
    Restart,
}

pub struct Tweak {
    pub id: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    /// Side effects and trade-offs, shown before applying.
    pub caveat: Option<&'static str>,
    pub category: Category,
    pub group: &'static str,
    pub on: &'static [Op],
    pub off: &'static [Op],
    pub recommend: Recommend,
    pub after: After,
    /// Minimum Windows build (22000 = Windows 11).
    pub min_build: u32,
    /// Windows' own default is the `on` list (e.g. Game Mode). Normally the default is `off`.
    pub default_on: bool,
    /// Also listed in the Latency sections (Gaming and Network pages).
    pub latency: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum State {
    /// The `on` list matches.
    On,
    /// The `off` list matches.
    Off,
    /// Neither: changed elsewhere (e.g. in Windows Settings or by policy).
    Custom,
}

impl Tweak {
    pub fn cmds(&self) -> impl Iterator<Item = &'static Cmd> + '_ {
        self.on.iter().chain(self.off).filter_map(|op| match op {
            Op::Cmd(c) => Some(*c),
            _ => None,
        })
    }

    /// False when a command-backed setting does not exist on this PC (e.g. no adapter supports it).
    pub fn available(&self) -> bool {
        self.cmds().all(|c| cmd_state(c.key).is_some())
    }

    pub fn needs_admin(&self) -> bool {
        self.on.iter().chain(self.off).any(Op::needs_admin)
    }

    pub fn state(&self) -> State {
        if self.on.iter().all(Op::matches) {
            State::On
        } else if self.off.iter().all(Op::matches) {
            State::Off
        } else {
            State::Custom
        }
    }

    pub fn default_state(&self) -> State {
        if self.default_on {
            State::On
        } else {
            State::Off
        }
    }

    /// Operations that restore the Windows default.
    pub fn default_ops(&self) -> &'static [Op] {
        if self.default_on {
            self.on
        } else {
            self.off
        }
    }
}
