//! Process manager: running processes with CPU/RAM, publisher, protection rules, and safe termination.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

use crate::engine::types::{JobRequest, Shell};
use crate::error::{AppError, AppResult};
use crate::util::powershell;

/// Ending these crashes or locks Windows, or disables security. Never offered.
const PROTECTED: &[&str] = &[
    "system", "system idle process", "idle", "registry", "memory compression", "secure system", "smss.exe", "csrss.exe",
    "wininit.exe", "winlogon.exe", "services.exe", "lsass.exe", "lsaiso.exe", "svchost.exe", "fontdrvhost.exe", "dwm.exe",
    "wudfhost.exe", "msmpeng.exe", "nissrv.exe", "securityhealthservice.exe", "mpdefendercoreservice.exe", "sgrmbroker.exe",
    "lsm.exe", "conhost.exe",
];

/// Ending these is possible but has a visible effect.
const CAUTION: &[(&str, &str)] = &[
    ("explorer.exe", "The taskbar, Start and desktop disappear until Windows restarts them (usually automatic)."),
    ("sihost.exe", "Shell infrastructure: notifications and some taskbar features stop until you sign in again."),
    ("ctfmon.exe", "Text input: typing in some apps, the emoji panel and input methods may stop working."),
    ("audiodg.exe", "Audio engine: sound cuts out briefly and restarts."),
    ("spoolsv.exe", "Print spooler: printing stops until the service restarts."),
    ("searchhost.exe", "Windows Search: Start search restarts."),
    ("startmenuexperiencehost.exe", "Start menu: restarts automatically."),
    ("runtimebroker.exe", "Manages Store app permissions; apps may lose access to camera/microphone/location until restarted."),
    ("securityhealthsystray.exe", "Windows Security tray icon."),
    ("smartscreen.exe", "Windows SmartScreen reputation checks."),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Protection {
    None,
    Caution,
    Protected,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessInfo {
    pub pid: u32,
    pub parent: Option<u32>,
    pub name: String,
    pub path: Option<String>,
    pub publisher: Option<String>,
    /// Percent of total CPU capacity.
    pub cpu: f32,
    pub memory: u64,
    pub start_time: u64,
    pub protection: Protection,
    pub note: Option<&'static str>,
}

pub struct ProcessManager {
    sys: Mutex<System>,
    publishers: Arc<Mutex<HashMap<String, Option<String>>>>,
    fetching: Arc<AtomicBool>,
}

impl ProcessManager {
    pub fn new() -> Self {
        let mut sys = System::new();
        sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::new().with_cpu().with_memory().with_exe(UpdateKind::OnlyIfNotSet));
        Self { sys: Mutex::new(sys), publishers: Arc::default(), fetching: Arc::default() }
    }

    fn classify(name: &str, pid: u32, ours: &HashSet<u32>) -> (Protection, Option<&'static str>) {
        let n = name.to_lowercase();
        if ours.contains(&pid) {
            return (Protection::Protected, Some("Part of Blob Optimizers."));
        }
        if PROTECTED.contains(&n.as_str()) || pid <= 4 {
            return (Protection::Protected, Some("Core Windows or security process. Ending it can crash or lock Windows."));
        }
        if let Some((_, note)) = CAUTION.iter().find(|(c, _)| *c == n) {
            return (Protection::Caution, Some(note));
        }
        (Protection::None, None)
    }

    /// This process and everything it started (WebView2 helpers).
    fn own_tree(sys: &System) -> HashSet<u32> {
        let me = std::process::id();
        let mut ours = HashSet::from([me]);
        loop {
            let before = ours.len();
            for (pid, p) in sys.processes() {
                if p.parent().is_some_and(|pp| ours.contains(&pp.as_u32())) {
                    ours.insert(pid.as_u32());
                }
            }
            if ours.len() == before {
                break;
            }
        }
        ours
    }

    pub fn list(&self) -> Vec<ProcessInfo> {
        let mut sys = match self.sys.lock() {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        sys.refresh_cpu_list(sysinfo::CpuRefreshKind::new());
        sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::new().with_cpu().with_memory().with_exe(UpdateKind::OnlyIfNotSet));
        let cpus = sys.cpus().len().max(1) as f32;
        let ours = Self::own_tree(&sys);
        let publishers = self.publishers.lock().map(|p| p.clone()).unwrap_or_default();
        let mut missing: Vec<String> = Vec::new();

        let out = sys
            .processes()
            .iter()
            .filter(|(pid, _)| pid.as_u32() != 0)
            .map(|(pid, p)| {
                let name = p.name().to_string_lossy().into_owned();
                let path = p.exe().map(|e| e.to_string_lossy().into_owned()).filter(|s| !s.is_empty());
                let publisher = path.as_ref().and_then(|x| match publishers.get(x) {
                    Some(v) => v.clone(),
                    None => {
                        missing.push(x.clone());
                        None
                    }
                });
                let (protection, note) = Self::classify(&name, pid.as_u32(), &ours);
                ProcessInfo {
                    pid: pid.as_u32(),
                    parent: p.parent().map(|x| x.as_u32()),
                    name,
                    path,
                    publisher,
                    cpu: (p.cpu_usage() / cpus).clamp(0.0, 100.0),
                    memory: p.memory(),
                    start_time: p.start_time(),
                    protection,
                    note,
                }
            })
            .collect();
        drop(sys);
        self.fetch_publishers(missing);
        out
    }

    /// Reads company names from executables in the background (file version info).
    fn fetch_publishers(&self, mut paths: Vec<String>) {
        paths.sort();
        paths.dedup();
        if paths.is_empty() || self.fetching.swap(true, Ordering::SeqCst) {
            return;
        }
        let store = self.publishers.clone();
        let flag = self.fetching.clone();
        std::thread::spawn(move || {
            let list = paths.iter().map(|p| format!("'{}'", p.replace('\'', "''"))).collect::<Vec<_>>().join(",");
            let script = format!(
                "$ErrorActionPreference='SilentlyContinue'\n$m=@{{}}\nforeach ($p in @({list})) {{ $c = (Get-Item -LiteralPath $p).VersionInfo.CompanyName; $m[$p] = if ($c) {{ $c.Trim() }} else {{ '' }} }}\n$m | ConvertTo-Json -Compress"
            );
            let result: Option<HashMap<String, String>> = powershell::run_json(&script, Duration::from_secs(60)).ok();
            if let Ok(mut s) = store.lock() {
                for p in &paths {
                    let v = result.as_ref().and_then(|m| m.get(p)).filter(|c| !c.is_empty()).cloned();
                    s.insert(p.clone(), v);
                }
            }
            flag.store(false, Ordering::SeqCst);
        });
    }

    /// Validates that `pid` is still the process the user saw, and builds the termination job.
    pub fn end(&self, pid: u32, name: &str, start_time: u64, tree: bool, as_admin: bool) -> AppResult<JobRequest> {
        let mut sys = self.sys.lock().map_err(|_| AppError::Other("process lock poisoned".into()))?;
        sys.refresh_processes(ProcessesToUpdate::Some(&[Pid::from_u32(pid)]), true);
        let p = sys.process(Pid::from_u32(pid)).ok_or_else(|| AppError::Other(format!("{name} has already exited.")))?;
        let actual = p.name().to_string_lossy().into_owned();
        // Guards against PID reuse between listing and clicking.
        if !actual.eq_ignore_ascii_case(name) || p.start_time() != start_time {
            return Err(AppError::Other(format!("{name} has already exited; process {pid} is now a different program.")));
        }
        let ours = Self::own_tree(&sys);
        if Self::classify(&actual, pid, &ours).0 == Protection::Protected {
            return Err(AppError::Other(format!("{actual} is protected: ending it could crash or lock Windows.")));
        }
        drop(sys);
        let script = if tree {
            format!("& taskkill.exe /PID {pid} /T /F\nif ($LASTEXITCODE) {{ exit $LASTEXITCODE }}")
        } else {
            format!("Stop-Process -Id {pid} -Force -ErrorAction Stop\n'Ended {actual} (PID {pid}).'")
        };
        let mut r = JobRequest::internal(format!("End {actual}{}", if tree { " and its children" } else { "" }), Shell::PowerShell, script);
        r.operation_id = Some("processes.end".into());
        r.requires_admin = as_admin;
        r.timeout = Duration::from_secs(60);
        Ok(r)
    }
}
