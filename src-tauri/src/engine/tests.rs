//! End-to-end tests that run real (harmless) processes through the engine.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::types::*;
use super::Engine;

type Events = Arc<Mutex<Vec<(String, serde_json::Value)>>>;

fn engine() -> (Engine, Events, tempdir::Dir) {
    let dir = tempdir::Dir::new();
    let events: Events = Arc::default();
    let sink = events.clone();
    let e = Engine::new(
        dir.0.clone(),
        false,
        Box::new(move |ev, v| sink.lock().unwrap().push((ev.to_string(), v))),
    );
    (e, events, dir)
}

/// Minimal self-cleaning temp dir (avoids a dev-dependency).
mod tempdir {
    pub struct Dir(pub std::path::PathBuf);
    impl Dir {
        pub fn new() -> Self {
            static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let p = std::env::temp_dir().join(format!("blob-engine-test-{}-{n}", std::process::id()));
            std::fs::create_dir_all(&p).unwrap();
            Dir(p)
        }
    }
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

fn user(shell: Shell, cmd: &str, timeout: Duration) -> JobRequest {
    JobRequest {
        title: cmd.into(),
        source: Source::Internal,
        operation_id: None,
        shell,
        command: cmd.into(),
        requires_admin: false,
        modifies_system: false,
        cancellable: true,
        timeout,
        progress_pattern: Some(r"(\d+)%"),
        kill_tree: true,
        on_complete: None,
    }
}

fn wait(e: &Engine, id: &str) -> HistoryEntry {
    let start = Instant::now();
    loop {
        if let Some(h) = e.history(50).into_iter().find(|h| h.id == id) {
            return h;
        }
        assert!(start.elapsed() < Duration::from_secs(60), "job {id} did not finish");
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn out_text(e: &Engine, id: &str) -> Vec<String> {
    e.output(id).unwrap().into_iter().filter(|l| l.stream == Stream::Out).map(|l| l.text).collect()
}

#[test]
fn powershell_output_errors_and_exit_codes() {
    let (e, events, _d) = engine();
    let ok = e.submit(user(Shell::PowerShell, "'line one'; Write-Output 'ünïcødé'; '##PROGRESS 50 Halfway'", Duration::from_secs(60))).unwrap();
    let h = wait(&e, &ok.id);
    assert_eq!(h.status, JobStatus::Succeeded, "{h:?}");
    assert_eq!(h.progress.as_ref().map(|p| p.percent), Some(100.0));
    let out = out_text(&e, &ok.id);
    assert_eq!(out, vec!["line one", "ünïcødé"]);

    let bad = e.submit(user(Shell::PowerShell, "Get-Item C:\\definitely\\missing", Duration::from_secs(60))).unwrap();
    let h = wait(&e, &bad.id);
    assert_eq!(h.status, JobStatus::Failed);
    let err: Vec<_> = e.output(&bad.id).unwrap().into_iter().filter(|l| l.stream == Stream::Err).collect();
    assert!(err.iter().any(|l| l.text.contains("Cannot find path")), "{err:?}");

    let syntax = e.submit(user(Shell::PowerShell, "if ( {", Duration::from_secs(60))).unwrap();
    assert_eq!(wait(&e, &syntax.id).status, JobStatus::Failed);

    let ev = events.lock().unwrap();
    assert!(ev.iter().any(|(n, _)| n == "engine:output"));
    assert!(ev.iter().any(|(n, v)| n == "engine:job" && v["status"] == "succeeded"));
}

#[test]
fn cmd_exit_code_and_streaming() {
    let (e, _ev, _d) = engine();
    let j = e.submit(user(Shell::Cmd, "echo hello & echo oops 1>&2 & exit /b 7", Duration::from_secs(30))).unwrap();
    let h = wait(&e, &j.id);
    assert_eq!(h.status, JobStatus::Failed);
    assert_eq!(h.exit_code, Some(7));
    assert_eq!(out_text(&e, &j.id), vec!["hello"]);
}

#[test]
fn cancel_and_timeout_kill_the_process_tree() {
    let (e, _ev, _d) = engine();
    let j = e.submit(user(Shell::Cmd, "ping -n 30 127.0.0.1", Duration::from_secs(60))).unwrap();
    std::thread::sleep(Duration::from_millis(1500));
    e.cancel(&j.id).unwrap();
    let started = Instant::now();
    let h = wait(&e, &j.id);
    assert_eq!(h.status, JobStatus::Cancelled);
    assert!(started.elapsed() < Duration::from_secs(10));

    let t = e.submit(user(Shell::PowerShell, "Start-Sleep -Seconds 30", Duration::from_secs(2))).unwrap();
    let h = wait(&e, &t.id);
    assert_eq!(h.status, JobStatus::TimedOut);
}

#[test]
fn system_changes_run_one_at_a_time() {
    let (e, _ev, _d) = engine();
    let mut a = user(Shell::Cmd, "ping -n 5 127.0.0.1", Duration::from_secs(30));
    a.modifies_system = true;
    let first = e.submit(a.clone()).unwrap();
    assert!(e.submit(a).is_err(), "second exclusive job must be rejected");
    // Read-only jobs may still run alongside.
    let ro = e.submit(user(Shell::Cmd, "echo ok", Duration::from_secs(30))).unwrap();
    assert_eq!(wait(&e, &ro.id).status, JobStatus::Succeeded);
    e.cancel(&first.id).unwrap();
    wait(&e, &first.id);

    // Non-cancellable jobs refuse cancellation.
    let mut nc = user(Shell::Cmd, "ping -n 2 127.0.0.1", Duration::from_secs(30));
    nc.cancellable = false;
    let j = e.submit(nc).unwrap();
    assert!(e.cancel(&j.id).is_err());
    assert_eq!(wait(&e, &j.id).status, JobStatus::Succeeded);
}

#[test]
fn history_persists_and_clears() {
    let (e, _ev, d) = engine();
    let j = e.submit(user(Shell::Cmd, "echo persisted", Duration::from_secs(30))).unwrap();
    wait(&e, &j.id);
    // A fresh engine on the same directory sees the entry and its output.
    let e2 = Engine::new(d.0.clone(), false, Box::new(|_, _| {}));
    assert!(e2.history(10).iter().any(|h| h.id == j.id));
    assert_eq!(out_text(&e2, &j.id), vec!["persisted"]);
    e2.clear_history().unwrap();
    assert!(e2.history(10).is_empty());
    assert!(e2.output(&j.id).is_err());
}

/// Exercises the elevated wrapper's file-tailing and cancel-file protocol without UAC by running it
/// in a normal PowerShell.
#[test]
fn elevated_wrapper_protocol() {
    use super::launch;
    let d = tempdir::Dir::new();
    let (out, err, cancel) = (d.0.join("x.out"), d.0.join("x.err"), d.0.join("x.cancel"));
    let l = launch::build(Shell::Cmd, "echo from-wrapper & exit /b 5").unwrap();
    let script = launch::elevated_script(&l, &out, &err, &cancel, true);
    let r = crate::util::powershell::run(&script, Duration::from_secs(30));
    assert!(matches!(r, Err(crate::error::AppError::CommandFailed { code: 5, .. })), "{r:?}");
    assert!(std::fs::read_to_string(&out).unwrap().contains("from-wrapper"));

    let l = launch::build(Shell::Cmd, "ping -n 30 127.0.0.1").unwrap();
    let script = launch::elevated_script(&l, &out, &err, &cancel, true);
    let c2 = cancel.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(1500));
        std::fs::write(c2, b"").unwrap();
    });
    let started = Instant::now();
    let r = crate::util::powershell::run(&script, Duration::from_secs(30));
    assert!(matches!(r, Err(crate::error::AppError::CommandFailed { code: 1223, .. })), "{r:?}");
    assert!(started.elapsed() < Duration::from_secs(10));
}
