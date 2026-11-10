//! Tests use a private HKCU test key and a fake startup entry; real settings are only read.

use std::time::{Duration, Instant};

use super::model::{After, Category, Op, Recommend, State, Tweak, V};
use super::*;
use crate::engine::Engine;
use crate::util::registry::Hive::CurrentUser as CU;

const TEST_KEY: &str = r"Software\BlobOptimizersTweakTest";

fn run(script: &str) {
    // Plain PowerShell exits 1 when the last command "failed" even with SilentlyContinue (e.g. deleting
    // something already absent); terminating errors still fail before reaching the explicit exit.
    crate::util::powershell::run(&format!("$ErrorActionPreference='Stop'\n{script}\nexit 0"), Duration::from_secs(60)).expect("script runs");
}

fn engine(dir: &std::path::Path) -> Engine {
    Engine::new(dir.join("engine"), false, Box::new(|_, _| {}))
}

fn wait(e: &Engine, id: &str) -> crate::engine::types::HistoryEntry {
    let start = Instant::now();
    loop {
        if let Some(h) = e.history(20).into_iter().find(|h| h.id == id) {
            return h;
        }
        assert!(start.elapsed() < Duration::from_secs(120));
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("blob-tweaks-test-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&p).unwrap();
    p
}

#[test]
fn catalog_is_consistent() {
    let mut ids: Vec<_> = catalog::TWEAKS.iter().map(|t| t.id).collect();
    ids.sort();
    let n = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), n, "duplicate tweak ids");
    for t in catalog::TWEAKS {
        assert!(!t.on.is_empty() && !t.off.is_empty(), "{}", t.id);
        // Detection must never panic on a real system.
        let _ = t.state();
        // Nothing may touch Defender, the firewall or Windows Update services.
        for op in t.on {
            let d = op.describe().to_lowercase();
            for bad in ["windefend", "mpssvc", "wuauserv", "usosvc", "waasmedic", "spynet", "disableantispyware", "realtime"] {
                assert!(!d.contains(bad), "{} touches {bad}", t.id);
            }
        }
    }
}

static TEST_TWEAK: Tweak = Tweak {
    id: "test.tweak",
    title: "Test tweak",
    description: "",
    caveat: None,
    category: Category::Performance,
    group: "Test",
    on: &[Op::Set(CU, TEST_KEY, "Value", V::D(1)), Op::Set(CU, TEST_KEY, "Text", V::S("on"))],
    off: &[Op::Opt(CU, TEST_KEY, "Value", V::D(0)), Op::Del(CU, TEST_KEY, "Text")],
    recommend: Recommend::Apply,
    after: After::Nothing,
    min_build: 0,
    default_on: false,
    latency: false,
};

#[test]
fn apply_default_and_snapshot_restore_round_trip() {
    run(&registry::ps_delete_key(CU, TEST_KEY));
    assert_eq!(TEST_TWEAK.state(), State::Off, "absent value counts as default");

    run(&registry::ps_set(CU, TEST_KEY, "Value", &RegData::Dword(7)));
    assert_eq!(TEST_TWEAK.state(), State::Custom);
    let before = snapshot(&TEST_TWEAK);

    run(&TEST_TWEAK.on.iter().map(Op::script).collect::<String>());
    assert_eq!(TEST_TWEAK.state(), State::On);

    run(&restore_script(&before));
    assert_eq!(registry::read(CU, TEST_KEY, "Value"), Some(RegData::Dword(7)), "undo restores the exact previous value");
    assert_eq!(registry::read(CU, TEST_KEY, "Text"), None);

    run(&TEST_TWEAK.off.iter().map(Op::script).collect::<String>());
    assert_eq!(TEST_TWEAK.state(), State::Off);
    run(&registry::ps_delete_key(CU, TEST_KEY));
}

#[test]
fn engine_apply_saves_undo_snapshot() {
    // Uses a real catalogue entry that only touches HKCU and puts the user's value back afterwards.
    let t = catalog::find("perf.startup_delay").unwrap();
    let original = registry::read(CU, r"Software\Microsoft\Windows\CurrentVersion\Explorer\Serialize", "StartupDelayInMSec");
    let dir = temp_dir("apply");
    let tw = Tweaks::new(dir.clone());
    let e = engine(&dir);

    let job = e.submit(tw.apply(&[Change { id: t.id.into(), target: Target::On }]).unwrap()).unwrap();
    assert_eq!(wait(&e, &job.id).status, JobStatus::Succeeded);
    assert_eq!(t.state(), State::On);
    assert!(tw.list().iter().find(|i| i.id == t.id).unwrap().can_undo);

    let job = e.submit(tw.apply(&[Change { id: t.id.into(), target: Target::Undo }]).unwrap()).unwrap();
    assert_eq!(wait(&e, &job.id).status, JobStatus::Succeeded);
    let now = registry::read(CU, r"Software\Microsoft\Windows\CurrentVersion\Explorer\Serialize", "StartupDelayInMSec");
    assert_eq!(now, original, "undo restored the user's original value");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn read_only_queries_parse() {
    let info = system::info().expect("system info");
    assert!(info.plans.iter().any(|p| p.active), "{:?}", info.plans);
    assert!(info.pagefile.ram_mb > 0);
    let cleanup = system::cleanup_scan().expect("cleanup scan");
    assert_eq!(cleanup.len(), system::CLEANUP_IDS.len());
    let dir = temp_dir("svc");
    let services = services::Services::new(dir.clone()).list().expect("services");
    assert!(services.iter().any(|s| s.name == "WinDefend" && s.protected) || !services.iter().any(|s| s.name == "WinDefend"));
    assert!(services.iter().any(|s| s.name.eq_ignore_ascii_case("Spooler")));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn protected_services_are_refused() {
    let dir = temp_dir("svc2");
    let s = services::Services::new(dir.clone());
    for name in ["WinDefend", "mpssvc", "wuauserv", "BFE"] {
        assert!(s.set_start(name, StartType::Disabled).is_err(), "{name}");
    }
    assert!(s.set_start("../evil", StartType::Disabled).is_err());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn power_validation() {
    assert!(system::set_pagefile(false, Some(512), Some(2048)).is_err(), "too small");
    assert!(system::set_pagefile(false, Some(4096), Some(2048)).is_err(), "max < initial");
    assert!(system::set_timeouts(system::Timeouts { monitor_ac: 9999, sleep_ac: 0, monitor_dc: None, sleep_dc: None }).is_err());
    assert!(system::add_plan("turbo").is_err());
    assert!(system::cleanup(&["C:\\".into()]).is_err());
}

#[test]
fn startup_disable_remove_restore_round_trip() {
    let run_key = r"Software\Microsoft\Windows\CurrentVersion\Run";
    let name = "BlobOptimizersStartupTest";
    run(&registry::ps_set(CU, run_key, name, &RegData::String(r"C:\Windows\System32\notepad.exe /blobtest".into())));
    let dir = temp_dir("startup");
    let st = startup::Startup::new(dir.clone());
    let e = engine(&dir);
    let id = format!(r"registry|HKCU\Run|{name}");

    let entry = st.list().unwrap().entries.into_iter().find(|x| x.id == id).expect("listed");
    assert!(entry.enabled);
    assert_eq!(entry.publisher.as_deref(), Some("Microsoft Corporation"));

    let j = e.submit(st.set_enabled(&id, false).unwrap()).unwrap();
    assert_eq!(wait(&e, &j.id).status, JobStatus::Succeeded);
    assert!(!st.list().unwrap().entries.iter().find(|x| x.id == id).unwrap().enabled);

    let j = e.submit(st.remove(&id).unwrap()).unwrap();
    assert_eq!(wait(&e, &j.id).status, JobStatus::Succeeded);
    let after = st.list().unwrap();
    assert!(!after.entries.iter().any(|x| x.id == id));
    assert!(after.removed.iter().any(|r| r.name == name));

    let j = e.submit(st.restore(name, r"HKCU\Run").unwrap()).unwrap();
    assert_eq!(wait(&e, &j.id).status, JobStatus::Succeeded);
    assert!(registry::read(CU, run_key, name).is_some());

    run(&registry::ps_delete_value(CU, run_key, name));
    run(&format!(
        "Remove-ItemProperty -LiteralPath 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run' -Name '{name}' -ErrorAction SilentlyContinue"
    ));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn processes_are_listed_and_critical_ones_protected() {
    let pm = crate::system::processes::ProcessManager::new();
    std::thread::sleep(Duration::from_millis(500));
    let list = pm.list();
    assert!(list.len() > 20);
    assert!(list.iter().all(|p| (0.0..=100.0).contains(&p.cpu)));
    let total: f32 = list.iter().map(|p| p.cpu).sum();
    assert!(total <= 100.5, "per-process CPU adds up to {total}%");
    let lsass = list.iter().find(|p| p.name.eq_ignore_ascii_case("lsass.exe")).expect("lsass running");
    assert!(pm.end(lsass.pid, &lsass.name, lsass.start_time, false, false).is_err());
    // A stale PID / name pair is refused.
    assert!(pm.end(lsass.pid, "notepad.exe", lsass.start_time, false, false).is_err());
}

#[test]
fn command_checks_run_and_apply_scripts_parse() {
    use std::time::Instant;
    model::invalidate_cmd_cache();
    let started = Instant::now();
    refresh_states();
    let took = started.elapsed();
    let cmds: Vec<&model::Cmd> = catalog::TWEAKS.iter().flat_map(|t| t.cmds()).collect();
    assert!(!cmds.is_empty());
    for t in catalog::TWEAKS.iter().filter(|t| t.cmds().next().is_some()) {
        println!("{:<34} available={:<5} state={:?}", t.id, t.available(), t.state());
    }
    println!("checks took {took:?}");
    assert!(took < Duration::from_secs(30), "checks too slow: {took:?}");
    // The USB power setting exists on every Windows PC with a power plan.
    assert!(catalog::find("game.usb_suspend_off").unwrap().available(), "powercfg check failed");

    // Every apply script must at least be valid PowerShell (they are not run here: they change the system).
    let mut script = String::from("$errs = 0\n");
    for c in &cmds {
        let body = c.apply.replace('\'', "''");
        script.push_str(&format!(
            "$t = $null; $e = $null; [void][System.Management.Automation.Language.Parser]::ParseInput('{body}', [ref]$t, [ref]$e); if ($e.Count) {{ $errs++; Write-Output ('{}: ' + $e[0].Message) }}\n",
            c.key
        ));
    }
    script.push_str("if ($errs) { exit 1 }\n");
    let out = crate::util::powershell::run(&script, Duration::from_secs(60));
    assert!(out.is_ok(), "apply scripts have syntax errors: {out:?}");

    // Latency sections have content in both places.
    assert!(catalog::TWEAKS.iter().filter(|t| t.latency && t.category == Category::Gaming).count() >= 8);
    assert!(catalog::TWEAKS.iter().filter(|t| t.latency && t.category == Category::Network).count() >= 3);
}

#[test]
#[ignore = "manual: writes the live tweak list as JSON to $BLOB_DUMP for UI previews"]
fn dump_tweak_list() {
    let Ok(path) = std::env::var("BLOB_DUMP") else { return };
    let t = Tweaks::new(temp_dir("dump"));
    std::fs::write(path, serde_json::to_string(&t.list()).unwrap()).unwrap();
}
