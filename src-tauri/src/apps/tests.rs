//! Real-system tests. They only create and remove their own fake entries (an HKCU uninstall entry,
//! a folder in %LOCALAPPDATA% and an HKCU\Software key), never touching real apps.

use std::time::{Duration, Instant};

use super::inventory::{InstalledApp, Kind};
use super::*;
use crate::engine::types::JobStatus;
use crate::engine::Engine;

fn engine(dir: &std::path::Path) -> Engine {
    Engine::new(dir.join("engine"), false, Box::new(|_, _| {}))
}

fn wait(e: &Engine, id: &str) -> crate::engine::types::HistoryEntry {
    let start = Instant::now();
    loop {
        if let Some(h) = e.history(20).into_iter().find(|h| h.id == id) {
            return h;
        }
        assert!(start.elapsed() < Duration::from_secs(120), "job did not finish");
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn temp_dir(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("blob-apps-test-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&p).unwrap();
    p
}

#[test]
fn inventory_lists_and_classifies_real_apps() {
    let apps = inventory::scan().expect("inventory");
    assert!(apps.iter().any(|a| a.kind == Kind::Appx), "no Store apps found");
    assert!(apps.iter().any(|a| a.kind == Kind::Win32), "no desktop programs found");
    // Critical packages present on every Windows 11 install must be protected.
    for pkg in ["Microsoft.WindowsStore", "Microsoft.SecHealthUI", "Microsoft.DesktopAppInstaller"] {
        if let Some(a) = apps.iter().find(|a| a.package_name.as_deref() == Some(pkg)) {
            assert_eq!(a.recommendation, knowledge::Recommendation::Protected, "{pkg}");
        }
    }
    assert!(apps.iter().all(|a| !a.name.is_empty()));
}

#[test]
#[ignore = "manual: prints how every installed app is classified"]
fn print_classification() {
    for a in inventory::scan().unwrap() {
        println!("{:?} | {:?} | {:?} | {} | {}", a.recommendation, a.category, a.kind, a.name, a.package_name.unwrap_or_default());
    }
}

#[test]
fn features_list_works_without_admin() {
    let f = features::list().expect("features");
    assert!(f.len() > 20);
    assert!(f.iter().any(|x| x.common));
}

#[test]
fn uninstall_script_runs_registered_uninstaller_and_detects_removal() {
    let key = "BlobOptimizersTestApp";
    let reg = format!(r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\{key}");
    let uninstall = format!(r#"cmd.exe /c reg delete "{reg}" /f"#);
    let status = std::process::Command::new("reg")
        .args(["add", &reg, "/v", "DisplayName", "/d", "Blob Test App", "/f"])
        .status()
        .unwrap();
    assert!(status.success());
    std::process::Command::new("reg").args(["add", &reg, "/v", "UninstallString", "/d", &uninstall, "/f"]).status().unwrap();

    let dir = temp_dir("uninstall");
    let apps = Apps::new(dir.join("apps"));
    let inv = apps.inventory(true).unwrap();
    let app = inv.iter().find(|a| a.name == "Blob Test App").expect("fake app listed");
    assert_eq!(app.scope, "user");

    let e = engine(&dir);
    let job = e.submit(apps.uninstall_win32(&app.id).unwrap()).unwrap();
    let h = wait(&e, &job.id);
    let out: Vec<String> = e.output(&job.id).unwrap().into_iter().map(|l| l.text).collect();
    // Clean up regardless of the result.
    let _ = std::process::Command::new("reg").args(["delete", &reg, "/f"]).output();
    assert_eq!(h.status, JobStatus::Succeeded, "{out:#?}");
    assert!(out.iter().any(|l| l.contains("was removed")), "{out:#?}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn protected_apps_are_refused() {
    let dir = temp_dir("protected");
    let apps = Apps::new(dir.join("apps"));
    let inv = apps.inventory(true).unwrap();
    if let Some(store) = inv.iter().find(|a| a.package_name.as_deref() == Some("Microsoft.WindowsStore")) {
        assert!(apps.remove_appx(&[store.id.clone()]).is_err());
    }
    assert!(apps.remove_appx(&["appx:Not.A.Real.Package".into()]).is_err());
    assert!(apps.uninstall_win32("win32:HKLM64:..\\..\\Run").is_err());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn leftovers_are_found_and_cleaned_with_backup() {
    let name = "BlobOptimizersLeftoverTest";
    let local = std::path::PathBuf::from(std::env::var("LOCALAPPDATA").unwrap()).join(name);
    std::fs::create_dir_all(local.join("cache")).unwrap();
    std::fs::write(local.join("cache").join("data.bin"), vec![0u8; 4096]).unwrap();
    let reg = format!(r"HKCU\Software\{name}");
    std::process::Command::new("reg").args(["add", &reg, "/v", "Setting", "/d", "1", "/f"]).output().unwrap();

    let dir = temp_dir("leftovers");
    let apps = Apps::new(dir.join("apps"));
    let fake = InstalledApp {
        id: "win32:HKCU:fake-leftover-test".into(),
        kind: Kind::Win32,
        name: name.into(),
        publisher: "Blob Test Publisher".into(),
        version: "1.0".into(),
        install_date: None,
        size_bytes: None,
        install_location: None,
        category: knowledge::Category::ThirdParty,
        recommendation: knowledge::Recommendation::Optional,
        app_kind: knowledge::AppKind::App,
        note: String::new(),
        preinstalled: false,
        scope: "user".into(),
        uninstall_method: inventory::UninstallMethod::Interactive,
        reinstallable: false,
        package_name: None,
        package_full_name: None,
        package_family: None,
        store_id: None,
        registry_key: None,
    };
    apps.snapshots.lock().unwrap().insert(fake.id.clone(), fake.clone());

    let scan = apps.scan_leftovers(&fake.id).expect("scan");
    let folder = scan.items.iter().find(|i| i.kind == leftovers::LeftoverKind::Folder).expect("folder found");
    assert_eq!(folder.size_bytes, 4096);
    assert!(folder.user_data && !folder.requires_admin && !folder.default_selected);
    let key = scan.items.iter().find(|i| i.kind == leftovers::LeftoverKind::Registry).expect("key found");

    let e = engine(&dir);
    let req = apps.cleanup_leftovers(&scan.scan_id, &[folder.id.clone(), key.id.clone()]).unwrap();
    assert!(!req.requires_admin);
    let job = e.submit(req).unwrap();
    let h = wait(&e, &job.id);
    let out: Vec<String> = e.output(&job.id).unwrap().into_iter().map(|l| l.text).collect();
    let folder_gone = !local.exists();
    let key_gone = !std::process::Command::new("reg").args(["query", &reg]).output().unwrap().status.success();
    // Clean up in case the test failed midway.
    let _ = std::fs::remove_dir_all(&local);
    let _ = std::process::Command::new("reg").args(["delete", &reg, "/f"]).output();
    let backups: Vec<_> = std::fs::read_dir(dir.join("apps").join("backups").join("registry")).map(|d| d.flatten().collect()).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);

    assert_eq!(h.status, JobStatus::Succeeded, "{out:#?}");
    assert!(folder_gone && key_gone, "{out:#?}");
    assert_eq!(backups.len(), 1, "registry backup written");
}
