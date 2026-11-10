//! Windows services: list, change startup type, start/stop, restore the original startup type.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::model::{self, StartType};
use crate::engine::types::{Callback, JobRequest, JobStatus, Shell};
use crate::error::{AppError, AppResult};
use crate::util::powershell;
use crate::util::serde_helpers::one_or_many;

/// Services Windows needs to boot, sign in, stay secure or stay updated. Never changed here.
const PROTECTED: &[&str] = &[
    "RpcSs", "RpcEptMapper", "DcomLaunch", "LSM", "SamSs", "Power", "PlugPlay", "ProfSvc", "EventLog", "Winmgmt",
    "CryptSvc", "Dhcp", "Dnscache", "nsi", "BrokerInfrastructure", "SystemEventsBroker", "Schedule", "UserManager",
    "CoreMessagingRegistrar", "StateRepository", "gpsvc", "AppXSvc", "ClipSVC", "camsvc", "TimeBrokerSvc", "LanmanWorkstation",
    "Audiosrv", "AudioEndpointBuilder", "Themes", "FontCache", "TrustedInstaller", "msiserver", "KeyIso", "VaultSvc",
    "BFE", "mpssvc", "WinDefend", "WdNisSvc", "Sense", "SecurityHealthService", "wscsvc", "SgrmBroker", "mpsdrv",
    "wuauserv", "UsoSvc", "WaaSMedicSvc", "BITS", "DoSvc", "InstallService", "EFS", "Netman", "netprofm", "NlaSvc",
    "Wcmsvc", "WlanSvc", "Appinfo", "seclogon", "sppsvc", "DeviceInstall", "DispBrokerDesktopSvc", "WinHttpAutoProxySvc",
];

struct Advice {
    name: &'static str,
    note: &'static str,
    suggested: StartType,
}

/// Services commonly tuned, with honest notes. Suggestions are shown, never applied automatically.
const ADVICE: &[Advice] = &[
    Advice { name: "DiagTrack", note: "Uploads diagnostic data to Microsoft.", suggested: StartType::Disabled },
    Advice { name: "dmwappushservice", note: "Routes device-management push messages; only used by company-managed PCs.", suggested: StartType::Manual },
    Advice { name: "MapsBroker", note: "Downloads offline maps for the Maps app.", suggested: StartType::Manual },
    Advice { name: "RetailDemo", note: "Store demo mode. Not used at home.", suggested: StartType::Disabled },
    Advice { name: "Fax", note: "Sends and receives faxes.", suggested: StartType::Manual },
    Advice { name: "lfsvc", note: "Geolocation; apps that use your location need it.", suggested: StartType::Manual },
    Advice { name: "WMPNetworkSvc", note: "Shares Windows Media Player libraries on the network.", suggested: StartType::Manual },
    Advice { name: "RemoteRegistry", note: "Lets other computers edit this PC's registry. Keep disabled.", suggested: StartType::Disabled },
    Advice { name: "XblAuthManager", note: "Xbox network sign-in for games. Manual starts it only when a game needs it.", suggested: StartType::Manual },
    Advice { name: "XblGameSave", note: "Cloud saves for Xbox network games. Manual starts it only when needed.", suggested: StartType::Manual },
    Advice { name: "XboxNetApiSvc", note: "Xbox Live networking for games. Manual starts it only when needed.", suggested: StartType::Manual },
    Advice { name: "Spooler", note: "Print Spooler. Needed to print or to use 'Print to PDF'.", suggested: StartType::Automatic },
    Advice { name: "SysMain", note: "Preloads frequently used apps into spare memory. Usually best left on.", suggested: StartType::Automatic },
    Advice { name: "WSearch", note: "Indexes files for fast Start and File Explorer search. Disabling makes search slow.", suggested: StartType::Automatic },
    Advice { name: "TabletInputService", note: "Touch keyboard and handwriting; also needed for emoji panel and some input methods.", suggested: StartType::Manual },
    Advice { name: "PhoneSvc", note: "Telephony state for Phone Link calls.", suggested: StartType::Manual },
    Advice { name: "WerSvc", note: "Windows Error Reporting.", suggested: StartType::Manual },
];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawService {
    name: String,
    display_name: String,
    state: String,
    start_mode: String,
    description: Option<String>,
    delayed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceInfo {
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub running: bool,
    /// Automatic, Manual, Disabled, Boot or System.
    pub start_mode: String,
    pub delayed: bool,
    pub protected: bool,
    pub note: Option<&'static str>,
    pub suggested: Option<StartType>,
    /// Start type before Blob Optimizers first changed it.
    pub original: Option<StartType>,
}

const LIST: &str = r#"$ErrorActionPreference = 'SilentlyContinue'
$out = @(foreach ($s in Get-CimInstance Win32_Service) {
    [ordered]@{ name = [string]$s.Name; displayName = [string]$s.DisplayName; state = [string]$s.State; startMode = [string]$s.StartMode;
        description = [string]$s.Description; delayed = [bool]$s.DelayedAutoStart }
})
ConvertTo-Json -InputObject $out -Compress"#;

fn is_protected(name: &str) -> bool {
    PROTECTED.iter().any(|p| p.eq_ignore_ascii_case(name))
}

fn is_per_user_instance(name: &str) -> bool {
    name.rsplit_once('_')
        .is_some_and(|(base, suffix)| !base.is_empty() && suffix.len() >= 4 && suffix.chars().all(|c| c.is_ascii_hexdigit()))
}

fn valid_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= 256 && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '$'))
}

pub struct Services {
    path: PathBuf,
    originals: Arc<Mutex<HashMap<String, StartType>>>,
}

impl Services {
    pub fn new(data_dir: PathBuf) -> Self {
        let path = data_dir.join("service-originals.json");
        let originals = std::fs::read_to_string(&path).ok().and_then(|s| serde_json::from_str::<HashMap<String, u32>>(&s).ok()).unwrap_or_default();
        let originals = originals.into_iter().filter_map(|(k, v)| StartType::from_registry(v).map(|s| (k, s))).collect();
        Self { path, originals: Arc::new(Mutex::new(originals)) }
    }

    pub fn list(&self) -> AppResult<Vec<ServiceInfo>> {
        #[derive(Deserialize)]
        struct W(#[serde(deserialize_with = "one_or_many")] Vec<RawService>);
        let W(raw) = powershell::run_json(LIST, Duration::from_secs(60))?;
        let originals = self.originals.lock().map(|o| o.clone()).unwrap_or_default();
        let mut out: Vec<ServiceInfo> = raw
            .into_iter()
            // Per-user service instances (Name_1a2b3c) are managed through their template service.
            .filter(|r| !is_per_user_instance(&r.name))
            .map(|r| {
                let advice = ADVICE.iter().find(|a| a.name.eq_ignore_ascii_case(&r.name));
                ServiceInfo {
                    protected: is_protected(&r.name) || matches!(r.start_mode.as_str(), "Boot" | "System"),
                    note: advice.map(|a| a.note),
                    suggested: advice.map(|a| a.suggested),
                    original: originals.get(&r.name).copied(),
                    running: r.state.eq_ignore_ascii_case("Running"),
                    start_mode: if r.start_mode == "Auto" { "Automatic".into() } else { r.start_mode },
                    delayed: r.delayed,
                    description: r.description.filter(|d| !d.is_empty()),
                    display_name: r.display_name,
                    name: r.name,
                }
            })
            .collect();
        out.sort_by_key(|a| a.display_name.to_lowercase());
        Ok(out)
    }

    fn check(&self, name: &str) -> AppResult<()> {
        if !valid_name(name) {
            return Err(AppError::Other("Invalid service name.".into()));
        }
        if is_protected(name) {
            return Err(AppError::Other(format!("{name} is essential to Windows (security, updates, networking or sign-in) and is not changed here.")));
        }
        if model::service_start(name).is_none() {
            return Err(AppError::Other(format!("Service {name} was not found or has a boot/system start type.")));
        }
        Ok(())
    }

    pub fn set_start(&self, name: &str, start: StartType) -> AppResult<JobRequest> {
        self.check(name)?;
        let previous = model::service_start(name);
        let mut r = JobRequest::internal(format!("Set {name} to {}", start.ps()), Shell::PowerShell, format!("{}'{name} is now {}.'", model::service_script(name, start), start.ps()));
        r.operation_id = Some("services.set_start".into());
        r.requires_admin = true;
        r.timeout = Duration::from_secs(120);
        let store = self.originals.clone();
        let path = self.path.clone();
        let n = name.to_string();
        r.on_complete = Some(Callback(Arc::new(move |e| {
            if e.status != JobStatus::Succeeded {
                return;
            }
            let Some(prev) = previous else { return };
            let Ok(mut o) = store.lock() else { return };
            // Remember only the first original value; forget it once the service is back to it.
            if o.get(&n) == Some(&start) || (!o.contains_key(&n) && prev == start) {
                o.remove(&n);
            } else {
                o.entry(n.clone()).or_insert(prev);
            }
            let raw: HashMap<&String, u32> = o.iter().map(|(k, v)| (k, v.registry_value())).collect();
            if let Ok(json) = serde_json::to_string_pretty(&raw) {
                let _ = std::fs::write(&path, json);
            }
        })));
        Ok(r)
    }

    pub fn control(&self, name: &str, start: bool) -> AppResult<JobRequest> {
        self.check(name)?;
        let script = if start {
            format!("Start-Service -Name '{name}' -ErrorAction Stop\n'{name} started.'")
        } else {
            format!("Stop-Service -Name '{name}' -ErrorAction Stop\n'{name} stopped.'")
        };
        let mut r = JobRequest::internal(format!("{} {name}", if start { "Start" } else { "Stop" }), Shell::PowerShell, script);
        r.operation_id = Some("services.control".into());
        r.requires_admin = true;
        r.timeout = Duration::from_secs(120);
        Ok(r)
    }
}
