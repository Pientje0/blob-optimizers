//! Installed-app inventory: raw data from `scripts/apps-inventory.ps1`, classified with `knowledge`.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::knowledge::{self, AppKind, Category, Recommendation};
use crate::error::AppResult;
use crate::util::powershell;
use crate::util::serde_helpers::one_or_many;

const SCRIPT: &str = include_str!("../../scripts/apps-inventory.ps1");

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawWin32 {
    pub hive: String,
    pub key: String,
    pub name: String,
    pub publisher: String,
    pub version: String,
    pub install_date: String,
    pub size_kb: Option<u64>,
    pub install_location: String,
    pub uninstall_string: String,
    pub quiet: bool,
    pub msi: bool,
    pub no_remove: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawAppx {
    pub name: String,
    pub full_name: String,
    pub family: String,
    pub display_name: Option<String>,
    pub publisher: String,
    pub version: String,
    pub signature_kind: String,
    pub non_removable: bool,
    pub install_location: String,
    pub install_date: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawInventory {
    #[serde(deserialize_with = "one_or_many")]
    win32: Vec<RawWin32>,
    #[serde(deserialize_with = "one_or_many")]
    appx: Vec<RawAppx>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Win32,
    Appx,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UninstallMethod {
    /// Vendor's silent uninstall command.
    Quiet,
    /// Windows Installer with a progress-only UI.
    Msi,
    /// Vendor's own uninstall wizard (may ask questions).
    Interactive,
    /// Remove-AppxPackage for the current user.
    Store,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledApp {
    pub id: String,
    pub kind: Kind,
    pub name: String,
    pub publisher: String,
    pub version: String,
    pub install_date: Option<String>,
    pub size_bytes: Option<u64>,
    pub install_location: Option<String>,
    pub category: Category,
    pub recommendation: Recommendation,
    pub app_kind: AppKind,
    pub note: String,
    pub preinstalled: bool,
    /// "machine" (all users) or "user".
    pub scope: String,
    pub uninstall_method: UninstallMethod,
    pub reinstallable: bool,
    /// Appx package name / full name / family (Store apps only).
    pub package_name: Option<String>,
    pub package_full_name: Option<String>,
    pub package_family: Option<String>,
    pub store_id: Option<String>,
    /// Registry location of the uninstall entry (desktop programs only).
    pub registry_key: Option<String>,
}

fn pretty_date(d: &str) -> Option<String> {
    let d = d.trim();
    (d.len() == 8 && d.chars().all(|c| c.is_ascii_digit())).then(|| format!("{}-{}-{}", &d[0..4], &d[4..6], &d[6..8]))
}

pub fn registry_path(hive: &str, key: &str) -> Option<String> {
    let base = match hive {
        "HKLM64" => r"HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
        "HKLM32" => r"HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
        "HKCU" => r"HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
        _ => return None,
    };
    // Uninstall subkey names never contain path separators; refuse anything that tries.
    (!key.is_empty() && !key.contains(['\\', '/', '\0'])).then(|| format!(r"{base}\{key}"))
}

fn win32(mut r: RawWin32) -> InstalledApp {
    r.name = r.name.trim_matches(|c: char| c.is_whitespace() || c.is_control() || c == '\u{200b}').to_string();
    let class = knowledge::classify_win32(&r.name, &r.publisher, &r.uninstall_string);
    let method = if r.msi || r.uninstall_string.to_ascii_lowercase().contains("msiexec") {
        UninstallMethod::Msi
    } else if r.quiet {
        UninstallMethod::Quiet
    } else {
        UninstallMethod::Interactive
    };
    let rec = if r.no_remove { Recommendation::Protected } else { class.rec };
    InstalledApp {
        id: format!("win32:{}:{}", r.hive, r.key),
        kind: Kind::Win32,
        registry_key: registry_path(&r.hive, &r.key),
        name: r.name,
        publisher: r.publisher,
        version: r.version,
        install_date: pretty_date(&r.install_date),
        size_bytes: r.size_kb.filter(|s| *s > 0).map(|kb| kb * 1024),
        install_location: Some(r.install_location).filter(|s| !s.is_empty()),
        category: class.category,
        recommendation: rec,
        app_kind: class.kind,
        note: if r.no_remove { "This program does not allow removal.".into() } else { class.note.into() },
        preinstalled: class.preinstalled,
        scope: if r.hive == "HKCU" { "user".into() } else { "machine".into() },
        uninstall_method: method,
        reinstallable: false,
        package_name: None,
        package_full_name: None,
        package_family: None,
        store_id: None,
    }
}

fn appx(r: RawAppx) -> InstalledApp {
    let rule = knowledge::appx_rule(&r.name);
    let microsoft = r.publisher.starts_with("Microsoft");
    let system = r.non_removable || r.signature_kind.eq_ignore_ascii_case("System");

    let (category, rec, kind, note, preinstalled) = match rule {
        Some(rule) => (
            rule.category,
            rule.rec,
            rule.kind,
            rule.note.to_string(),
            // Driver companion apps come with the driver, not with Windows.
            rule.rec != Recommendation::Protected && rule.kind != AppKind::Driver && !knowledge::is_user_installed(&r.name),
        ),
        None if system => (Category::Microsoft, Recommendation::Protected, AppKind::System, "Built-in Windows component.".to_string(), false),
        None if microsoft => (
            Category::Microsoft,
            Recommendation::Caution,
            AppKind::App,
            "Microsoft app without specific guidance; it may be used by a Windows feature.".to_string(),
            r.signature_kind.eq_ignore_ascii_case("Store"),
        ),
        None if r.signature_kind.eq_ignore_ascii_case("Developer") => (
            Category::ThirdParty,
            Recommendation::Optional,
            AppKind::App,
            "Installed by you or by another program's installer. Removing it may break that program.".to_string(),
            false,
        ),
        None => (Category::ThirdParty, Recommendation::Optional, AppKind::App, "Store app you installed.".to_string(), false),
    };
    // A package Windows flags as non-removable stays protected no matter what the rules say.
    let rec = if system { Recommendation::Protected } else { rec };
    let store_id = rule.and_then(|r| r.store_id).map(String::from);
    let name = rule
        .filter(|r| !r.pattern.ends_with('*'))
        .map(|r| r.name.to_string())
        .or_else(|| r.display_name.clone().filter(|n| !n.trim().is_empty()))
        .unwrap_or_else(|| knowledge::derive_name(&r.name));

    InstalledApp {
        id: format!("appx:{}", r.full_name),
        kind: Kind::Appx,
        name,
        publisher: r.publisher,
        version: r.version,
        install_date: r.install_date.as_deref().and_then(pretty_date),
        size_bytes: None,
        install_location: Some(r.install_location).filter(|s| !s.is_empty()),
        category,
        recommendation: rec,
        app_kind: kind,
        note,
        preinstalled,
        scope: "user".into(),
        uninstall_method: UninstallMethod::Store,
        reinstallable: store_id.is_some() || (microsoft && preinstalled),
        package_name: Some(r.name),
        package_full_name: Some(r.full_name),
        package_family: Some(r.family),
        store_id,
        registry_key: None,
    }
}

pub fn scan() -> AppResult<Vec<InstalledApp>> {
    let raw: RawInventory = powershell::run_json(SCRIPT, Duration::from_secs(90))?;
    let mut apps: Vec<InstalledApp> = raw.win32.into_iter().map(win32).chain(raw.appx.into_iter().map(appx)).collect();
    // The same program can be registered in both 32- and 64-bit views; keep one.
    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.id.cmp(&b.id)));
    apps.dedup_by(|a, b| a.kind == Kind::Win32 && b.kind == Kind::Win32 && a.name == b.name && a.version == b.version && a.publisher == b.publisher);
    Ok(apps)
}
