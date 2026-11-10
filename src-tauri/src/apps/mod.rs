//! Installed apps: inventory, removal (desktop programs and Store apps), reinstall, leftover cleanup
//! and optional Windows features. Every change is built here as a reviewed script and run through the
//! command engine; the UI only passes ids that are checked against the latest inventory.

pub mod features;
pub mod inventory;
pub mod knowledge;
pub mod ledger;
pub mod leftovers;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::engine::types::{now_ms, Callback, JobRequest, JobStatus, Shell};
use crate::error::{AppError, AppResult};
use crate::util::powershell;
use inventory::{InstalledApp, Kind};
use knowledge::Recommendation;
use ledger::{Ledger, RemovedApp};
use leftovers::{Leftover, LeftoverScan};

const UNINSTALL_WIN32: &str = include_str!("../../scripts/uninstall-win32.ps1");
const REMOVE_APPX: &str = include_str!("../../scripts/appx-remove.ps1");
const REINSTALL_APPX: &str = include_str!("../../scripts/appx-reinstall.ps1");

fn ps_quote(s: &str) -> String {
    s.replace('\'', "''")
}

fn err(msg: impl Into<String>) -> AppError {
    AppError::Other(msg.into())
}

pub struct Apps {
    inventory: Mutex<Option<Vec<InstalledApp>>>,
    /// Apps as they were when an uninstall was requested; leftover scans need their details later.
    snapshots: Mutex<HashMap<String, InstalledApp>>,
    scans: Mutex<HashMap<String, LeftoverScan>>,
    pub ledger: Arc<Ledger>,
    backup_dir: PathBuf,
}

impl Apps {
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            inventory: Mutex::new(None),
            snapshots: Mutex::new(HashMap::new()),
            scans: Mutex::new(HashMap::new()),
            ledger: Arc::new(Ledger::load(data_dir.join("removed-apps.json"))),
            backup_dir: data_dir.join("backups").join("registry"),
        }
    }

    pub fn inventory(&self, refresh: bool) -> AppResult<Vec<InstalledApp>> {
        if !refresh {
            if let Some(v) = self.inventory.lock().ok().and_then(|g| g.clone()) {
                return Ok(v);
            }
        }
        let fresh = inventory::scan()?;
        if let Ok(mut g) = self.inventory.lock() {
            *g = Some(fresh.clone());
        }
        Ok(fresh)
    }

    fn find(&self, id: &str) -> AppResult<InstalledApp> {
        self.inventory(false)?
            .into_iter()
            .find(|a| a.id == id)
            .ok_or_else(|| err("That app is no longer in the installed list. Refresh and try again."))
    }

    fn removable(app: &InstalledApp) -> AppResult<()> {
        if app.recommendation == Recommendation::Protected {
            return Err(err(format!("{} is protected: {}", app.name, app.note)));
        }
        Ok(())
    }

    /// Uninstalls one desktop program with its own uninstaller.
    pub fn uninstall_win32(&self, id: &str) -> AppResult<JobRequest> {
        let app = self.find(id)?;
        if app.kind != Kind::Win32 {
            return Err(err("Not a desktop program."));
        }
        Self::removable(&app)?;
        let key = app.registry_key.clone().ok_or_else(|| err("Missing uninstall registry entry."))?;
        let script = UNINSTALL_WIN32.replace("__KEY__", &ps_quote(&key)).replace("__NAME__", &ps_quote(&app.name));
        if let Ok(mut s) = self.snapshots.lock() {
            s.insert(app.id.clone(), app.clone());
        }
        let mut req = JobRequest::internal(format!("Uninstall {}", app.name), Shell::PowerShell, script);
        req.operation_id = Some("apps.uninstall".into());
        // Cancelling only stops waiting; killing a vendor uninstaller midway could corrupt the install.
        req.cancellable = true;
        req.kill_tree = false;
        req.timeout = Duration::from_secs(20 * 60);
        Ok(req)
    }

    /// Removes Store apps for the current user in one job; removed ones are recorded for reinstall.
    pub fn remove_appx(&self, ids: &[String]) -> AppResult<JobRequest> {
        if ids.is_empty() {
            return Err(err("Nothing selected."));
        }
        let inv = self.inventory(false)?;
        let mut apps = Vec::new();
        for id in ids {
            let app = inv.iter().find(|a| &a.id == id).ok_or_else(|| err("An app is no longer installed. Refresh and try again."))?;
            if app.kind != Kind::Appx {
                return Err(err(format!("{} is not a Store app.", app.name)));
            }
            Self::removable(app)?;
            apps.push(app.clone());
        }
        let list = apps
            .iter()
            .map(|a| {
                format!(
                    "@{{ name = '{}'; package = '{}'; fullName = '{}' }}",
                    ps_quote(&a.name),
                    ps_quote(a.package_name.as_deref().unwrap_or_default()),
                    ps_quote(a.package_full_name.as_deref().unwrap_or_default())
                )
            })
            .collect::<Vec<_>>()
            .join(",\n    ");
        let script = REMOVE_APPX.replace("__PACKAGES__", &format!("\n    {list}\n"));
        let title = if apps.len() == 1 { format!("Remove {}", apps[0].name) } else { format!("Remove {} Store apps", apps.len()) };

        let ledger = self.ledger.clone();
        let mut req = JobRequest::internal(title, Shell::PowerShell, script);
        req.operation_id = Some("apps.remove_appx".into());
        req.timeout = Duration::from_secs(15 * 60);
        req.on_complete = Some(Callback(Arc::new(move |_entry| {
            // Record exactly the packages that are gone now, whatever the job's overall result.
            let names: Vec<String> = apps.iter().filter_map(|a| a.package_name.clone()).collect();
            let quoted = names.iter().map(|n| format!("'{}'", ps_quote(n))).collect::<Vec<_>>().join(",");
            let check = format!("@({quoted}) | ForEach-Object {{ if (-not (Get-AppxPackage -Name $_)) {{ $_ }} }}");
            let Ok(out) = powershell::run(&check, Duration::from_secs(60)) else { return };
            let gone: Vec<&str> = out.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
            for a in apps.iter().filter(|a| a.package_name.as_deref().is_some_and(|n| gone.contains(&n))) {
                ledger.add(RemovedApp {
                    name: a.name.clone(),
                    package_name: a.package_name.clone().unwrap_or_default(),
                    package_full_name: a.package_full_name.clone().unwrap_or_default(),
                    package_family: a.package_family.clone().unwrap_or_default(),
                    store_id: a.store_id.clone(),
                    install_location: a.install_location.clone(),
                    removed_at: now_ms(),
                });
            }
        })));
        Ok(req)
    }

    pub fn reinstall_appx(&self, package_name: &str) -> AppResult<JobRequest> {
        let item = self.ledger.find(package_name).ok_or_else(|| err("This app is not in the removed-apps list."))?;
        let script = REINSTALL_APPX
            .replace("__PACKAGE__", &ps_quote(&item.package_name))
            .replace("__NAME__", &ps_quote(&item.name))
            .replace("__STOREID__", &ps_quote(item.store_id.as_deref().unwrap_or("")))
            .replace("__LOCATION__", &ps_quote(item.install_location.as_deref().unwrap_or("")));
        let ledger = self.ledger.clone();
        let package = item.package_name.clone();
        let mut req = JobRequest::internal(format!("Reinstall {}", item.name), Shell::PowerShell, script);
        req.operation_id = Some("apps.reinstall_appx".into());
        req.cancellable = true;
        req.timeout = Duration::from_secs(20 * 60);
        req.on_complete = Some(Callback(Arc::new(move |entry| {
            if entry.status == JobStatus::Succeeded {
                ledger.remove(&package);
            }
        })));
        Ok(req)
    }

    /// Looks for leftovers of a program uninstalled in this session.
    pub fn scan_leftovers(&self, id: &str) -> AppResult<LeftoverScan> {
        let app = self
            .snapshots
            .lock()
            .ok()
            .and_then(|s| s.get(id).cloned())
            .ok_or_else(|| err("Leftover scans are available for programs uninstalled in this session."))?;
        let installed = self.inventory(true)?;
        if installed.iter().any(|a| a.id == app.id) {
            return Err(err(format!("{} is still installed, so its files are not leftovers.", app.name)));
        }
        let items = leftovers::scan(&app, &installed)?;
        let scan = LeftoverScan { scan_id: format!("{}-{}", now_ms(), id.len()), app_name: app.name.clone(), items };
        if let Ok(mut s) = self.scans.lock() {
            s.insert(scan.scan_id.clone(), scan.clone());
        }
        Ok(scan)
    }

    pub fn cleanup_leftovers(&self, scan_id: &str, item_ids: &[String]) -> AppResult<JobRequest> {
        let scan = self
            .scans
            .lock()
            .ok()
            .and_then(|s| s.get(scan_id).cloned())
            .ok_or_else(|| err("This scan has expired. Scan again."))?;
        let chosen: Vec<Leftover> = scan.items.iter().filter(|i| item_ids.contains(&i.id)).cloned().collect();
        if chosen.is_empty() {
            return Err(err("Nothing selected."));
        }
        let script = leftovers::cleanup_script(&chosen, &self.backup_dir);
        let mut req = JobRequest::internal(format!("Clean up {} leftovers", scan.app_name), Shell::PowerShell, script);
        req.operation_id = Some("apps.cleanup_leftovers".into());
        req.requires_admin = chosen.iter().any(|i| i.requires_admin);
        req.timeout = Duration::from_secs(15 * 60);
        Ok(req)
    }

    pub fn toggle_feature(&self, name: &str, enable: bool) -> AppResult<JobRequest> {
        if !features::valid_name(name) {
            return Err(err("Invalid feature name."));
        }
        let all = features::list()?;
        let feature = all.iter().find(|f| f.name == name).ok_or_else(|| err("Unknown Windows feature."))?;
        if !enable && features::advice(name) == Some(features::FeatureAdvice::Protected) {
            return Err(err(format!("{} is required by Windows and is not turned off here.", feature.title)));
        }
        if feature.enabled == enable {
            return Err(err(format!("{} is already {}.", feature.title, if enable { "on" } else { "off" })));
        }
        let verb = if enable { "Turn on" } else { "Turn off" };
        let mut req = JobRequest::internal(format!("{verb} {}", feature.title), Shell::PowerShell, features::toggle_script(name, enable));
        req.operation_id = Some("features.toggle".into());
        req.requires_admin = true;
        req.timeout = Duration::from_secs(45 * 60);
        req.progress_pattern = Some(r"(\d{1,3}(?:\.\d+)?)\s?%");
        Ok(req)
    }
}
