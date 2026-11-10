//! Remembers Store apps removed through Blob Optimizers so they can be reinstalled later.

use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemovedApp {
    pub name: String,
    pub package_name: String,
    pub package_full_name: String,
    pub package_family: String,
    pub store_id: Option<String>,
    pub install_location: Option<String>,
    pub removed_at: u64,
}

pub struct Ledger {
    path: PathBuf,
    items: Mutex<Vec<RemovedApp>>,
}

impl Ledger {
    pub fn load(path: PathBuf) -> Self {
        let items = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Self { path, items: Mutex::new(items) }
    }

    fn save(&self, items: &[RemovedApp]) {
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_string_pretty(items) {
            let _ = std::fs::write(&self.path, json);
        }
    }

    pub fn list(&self) -> Vec<RemovedApp> {
        let mut v = self.items.lock().map(|i| i.clone()).unwrap_or_default();
        v.sort_by(|a, b| b.removed_at.cmp(&a.removed_at));
        v
    }

    pub fn add(&self, app: RemovedApp) {
        if let Ok(mut items) = self.items.lock() {
            items.retain(|i| i.package_name != app.package_name);
            items.push(app);
            self.save(&items);
        }
    }

    pub fn remove(&self, package_name: &str) {
        if let Ok(mut items) = self.items.lock() {
            items.retain(|i| i.package_name != package_name);
            self.save(&items);
        }
    }

    pub fn find(&self, package_name: &str) -> Option<RemovedApp> {
        self.items.lock().ok()?.iter().find(|i| i.package_name == package_name).cloned()
    }
}
