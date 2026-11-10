//! Declarative tweak model. A tweak is two lists of operations: `on` (the tweak applied) and `off`
//! (the Windows default). State is detected by checking which list currently matches.

use serde::Serialize;

use crate::util::registry::{self, Hive, RegData};

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
}

impl Op {
    pub fn needs_admin(&self) -> bool {
        match self {
            Op::Set(h, ..) | Op::Opt(h, ..) | Op::Del(h, ..) | Op::DelKey(h, ..) => *h == Hive::LocalMachine,
            Op::Svc(..) => true,
        }
    }

    pub fn matches(&self) -> bool {
        match *self {
            Op::Set(h, p, n, v) => registry::read(h, p, n).as_ref().is_some_and(|d| same(d, &v.data())),
            Op::Opt(h, p, n, v) => registry::read(h, p, n).as_ref().is_none_or(|d| same(d, &v.data())),
            Op::Del(h, p, n) => registry::read(h, p, n).is_none(),
            Op::DelKey(h, p) => !registry::key_exists(h, p),
            Op::Svc(name, start) => service_start(name) == Some(start),
        }
    }

    /// PowerShell that makes this operation true.
    pub fn script(&self) -> String {
        match *self {
            Op::Set(h, p, n, v) | Op::Opt(h, p, n, v) => registry::ps_set(h, p, n, &v.data()),
            Op::Del(h, p, n) => registry::ps_delete_value(h, p, n),
            Op::DelKey(h, p) => registry::ps_delete_key(h, p),
            Op::Svc(name, start) => service_script(name, start),
        }
    }

    pub fn describe(&self) -> String {
        match *self {
            Op::Set(h, p, n, v) | Op::Opt(h, p, n, v) => format!("{}\\{p} › {} = {}", h.reg(), if n.is_empty() { "(Default)" } else { n }, show(v)),
            Op::Del(h, p, n) => format!("{}\\{p} › {n} removed", h.reg()),
            Op::DelKey(h, p) => format!("{}\\{p} removed", h.reg()),
            Op::Svc(name, start) => format!("Service {name} › {}", start.ps()),
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
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
