//! FPS measurement from Windows' own graphics events (ETW), the same source PresentMon uses.
//!
//! A real-time ETW session needs administrator rights, so the app runs a copy of itself with
//! `--fps-helper` (elevated through UAC when needed). The helper writes one JSON line per second to a
//! file the app reads; it exits when asked (stop file) or when the app is gone.
//!
//! Counted: DXGI Present (DirectX 10/11/12) and Direct3D 9 Present calls per process. Games that
//! present only through Vulkan or OpenGL without DXGI interop are not counted.

use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FpsProcess {
    pub pid: u32,
    pub name: String,
    pub fps: f32,
    /// Average frame time in milliseconds.
    pub frame_ms: f32,
    /// FPS equivalent of the 99th-percentile frame time ("1% low").
    pub low1_fps: f32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FpsSample {
    pub t: u64,
    pub processes: Vec<FpsProcess>,
    pub error: Option<String>,
    /// The error is a permissions problem that an elevated helper would solve.
    #[serde(default)]
    pub needs_admin: bool,
}

/// Turns present timestamps (milliseconds) collected over `window_ms` into stats.
pub fn summarize(times_ms: &mut [f64], window_ms: f64) -> Option<(f32, f32, f32)> {
    if times_ms.len() < 2 || window_ms <= 0.0 {
        return None;
    }
    times_ms.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut intervals: Vec<f64> = times_ms.windows(2).map(|w| w[1] - w[0]).filter(|d| *d > 0.0).collect();
    if intervals.is_empty() {
        return None;
    }
    let fps = (times_ms.len() as f64 * 1000.0 / window_ms) as f32;
    let avg = intervals.iter().sum::<f64>() / intervals.len() as f64;
    intervals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let idx = ((intervals.len() as f64) * 0.99).ceil() as usize;
    let p99 = intervals[idx.saturating_sub(1).min(intervals.len() - 1)];
    Some((fps, avg as f32, (1000.0 / p99) as f32))
}

// ------------------------------------------------------------------------------------------------
// App side
// ------------------------------------------------------------------------------------------------

struct Session {
    out: PathBuf,
    stop: PathBuf,
    offset: u64,
    partial: String,
    latest: Option<FpsSample>,
    started: Instant,
}

pub struct FpsMonitor {
    dir: PathBuf,
    session: Mutex<Option<Session>>,
}

impl FpsMonitor {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir, session: Mutex::new(None) }
    }

    /// Starts the helper. Without `elevate` it runs with the app's own rights, which is enough for
    /// administrators and members of "Performance Log Users"; otherwise it goes through UAC.
    pub fn start(&self, app_elevated: bool, elevate: bool) -> AppResult<()> {
        let mut s = self.session.lock().map_err(|_| AppError::Other("fps lock".into()))?;
        if let Some(old) = s.take() {
            let _ = std::fs::write(&old.stop, b"");
        }
        let _ = std::fs::create_dir_all(&self.dir);
        let stamp = crate::engine::types::now_ms();
        let out = self.dir.join(format!("fps-{stamp}.jsonl"));
        let stop = self.dir.join(format!("fps-{stamp}.stop"));
        std::fs::write(&out, b"")?;
        let exe = std::env::current_exe()?;
        let args = format!("--fps-helper \"{}\" \"{}\" {}", out.display(), stop.display(), std::process::id());
        if app_elevated || !elevate {
            let mut cmd = std::process::Command::new(&exe);
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd.raw_arg(&args).creation_flags(crate::util::powershell::CREATE_NO_WINDOW);
            }
            cmd.spawn()?;
        } else {
            #[cfg(windows)]
            crate::engine::launch_elevated_with_args(&exe.to_string_lossy(), &args)?;
        }
        *s = Some(Session { out, stop, offset: 0, partial: String::new(), latest: None, started: Instant::now() });
        Ok(())
    }

    pub fn stop(&self) {
        if let Ok(mut s) = self.session.lock() {
            if let Some(sess) = s.take() {
                let _ = std::fs::write(&sess.stop, b"");
                // The helper deletes its output when it exits; remove leftovers later if it didn't.
                let out = sess.out.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_secs(5));
                    let _ = std::fs::remove_file(&out);
                    let _ = std::fs::remove_file(sess.stop);
                });
            }
        }
    }

    /// Latest sample, or None while waiting for the helper. `running` is false when no session exists.
    pub fn poll(&self) -> (bool, Option<FpsSample>) {
        let Ok(mut guard) = self.session.lock() else { return (false, None) };
        let Some(s) = guard.as_mut() else { return (false, None) };
        if let Ok(mut f) = std::fs::File::open(&s.out) {
            if f.seek(SeekFrom::Start(s.offset)).is_ok() {
                let mut buf = String::new();
                if let Ok(n) = f.read_to_string(&mut buf) {
                    s.offset += n as u64;
                    s.partial.push_str(&buf);
                    while let Some(i) = s.partial.find('\n') {
                        let line: String = s.partial.drain(..=i).collect();
                        if let Ok(sample) = serde_json::from_str::<FpsSample>(line.trim()) {
                            s.latest = Some(sample);
                        }
                    }
                }
            }
        }
        // The helper never started (UAC declined or failed): report it instead of waiting forever.
        if s.latest.is_none() && s.started.elapsed() > Duration::from_secs(30) {
            return (true, Some(FpsSample { error: Some("The FPS helper did not start. Was the administrator prompt declined?".into()), ..Default::default() }));
        }
        (true, s.latest.clone())
    }
}

// ------------------------------------------------------------------------------------------------
// Helper side (elevated process)
// ------------------------------------------------------------------------------------------------

#[cfg(windows)]
mod etw {
    use super::*;
    use std::sync::atomic::{AtomicI64, Ordering};
    use windows_sys::core::GUID;
    use windows_sys::Win32::System::Diagnostics::Etw::*;
    use windows_sys::Win32::System::Performance::QueryPerformanceFrequency;

    const SESSION_NAME: &str = "BlobOptimizersFps";
    const DXGI: GUID = GUID::from_u128(0xCA11C036_0102_4A2D_A6AD_F03CFED5D3C9);
    const D3D9: GUID = GUID::from_u128(0x783ACA0A_790E_4D7F_8451_AA850511C6B9);
    const DXGI_PRESENT_START: u16 = 42;
    const D3D9_PRESENT_START: u16 = 1;

    pub static PRESENTS: Mutex<Option<HashMap<u32, Vec<f64>>>> = Mutex::new(None);
    static QPC_FREQ: AtomicI64 = AtomicI64::new(1);

    fn same(a: &GUID, b: &GUID) -> bool {
        a.data1 == b.data1 && a.data2 == b.data2 && a.data3 == b.data3 && a.data4 == b.data4
    }

    unsafe extern "system" fn on_event(rec: *mut EVENT_RECORD) {
        let h = &(*rec).EventHeader;
        let id = h.EventDescriptor.Id;
        let hit = (same(&h.ProviderId, &DXGI) && id == DXGI_PRESENT_START) || (same(&h.ProviderId, &D3D9) && id == D3D9_PRESENT_START);
        if !hit {
            return;
        }
        let ms = h.TimeStamp as f64 * 1000.0 / QPC_FREQ.load(Ordering::Relaxed) as f64;
        if let Ok(mut g) = PRESENTS.lock() {
            g.get_or_insert_with(HashMap::new).entry(h.ProcessId).or_default().push(ms);
        }
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub struct Trace {
        control: u64,
        name: Vec<u16>,
        props: Vec<u8>,
    }

    fn props_buffer(name: &[u16]) -> Vec<u8> {
        let size = std::mem::size_of::<EVENT_TRACE_PROPERTIES>() + name.len() * 2;
        let mut buf = vec![0u8; size];
        unsafe {
            let p = buf.as_mut_ptr() as *mut EVENT_TRACE_PROPERTIES;
            (*p).Wnode.BufferSize = size as u32;
            (*p).Wnode.Flags = 0x0002_0000; // WNODE_FLAG_TRACED_GUID
            (*p).Wnode.ClientContext = 1; // QPC timestamps
            (*p).LogFileMode = 0x0000_0100; // EVENT_TRACE_REAL_TIME_MODE
            (*p).LoggerNameOffset = std::mem::size_of::<EVENT_TRACE_PROPERTIES>() as u32;
        }
        buf
    }

    pub fn start() -> Result<Trace, String> {
        unsafe {
            let mut f = 0i64;
            QueryPerformanceFrequency(&mut f);
            QPC_FREQ.store(f.max(1), Ordering::Relaxed);

            let name = wide(SESSION_NAME);
            let mut props = props_buffer(&name);
            let mut control = std::mem::zeroed();
            let mut r = StartTraceW(&mut control, name.as_ptr(), props.as_mut_ptr() as *mut _);
            if r == 183 {
                // ERROR_ALREADY_EXISTS: a previous helper didn't clean up. Stop it and retry.
                let mut stop = props_buffer(&name);
                ControlTraceW(std::mem::zeroed(), name.as_ptr(), stop.as_mut_ptr() as *mut _, 1);
                props = props_buffer(&name);
                r = StartTraceW(&mut control, name.as_ptr(), props.as_mut_ptr() as *mut _);
            }
            if r != 0 {
                return Err(if r == 5 {
                    "Administrator rights are required to read graphics events.".into()
                } else {
                    format!("Could not start the event trace (Windows error {r}).")
                });
            }
            for guid in [DXGI, D3D9] {
                EnableTraceEx2(control, &guid, 1, 4, 0, 0, 0, std::ptr::null());
            }
            let trace = Trace { control: control.Value, name, props };

            let mut logfile: EVENT_TRACE_LOGFILEW = std::mem::zeroed();
            logfile.LoggerName = trace.name.as_ptr() as *mut u16;
            logfile.Anonymous1.ProcessTraceMode = 0x0000_0100 | 0x1000_0000; // REAL_TIME | EVENT_RECORD
            logfile.Anonymous2.EventRecordCallback = Some(on_event);
            let handle = OpenTraceW(&mut logfile);
            if handle.Value == u64::MAX {
                trace.stop();
                return Err("Could not open the event trace.".into());
            }
            let h = handle.Value;
            std::thread::spawn(move || {
                let handles = [PROCESSTRACE_HANDLE { Value: h }];
                // Blocks until the session stops.
                ProcessTrace(handles.as_ptr(), 1, std::ptr::null(), std::ptr::null());
                CloseTrace(handles[0]);
            });
            Ok(trace)
        }
    }

    impl Trace {
        pub fn stop(&self) {
            unsafe {
                let mut props = props_buffer(&self.name);
                let _ = &self.props;
                ControlTraceW(CONTROLTRACE_HANDLE { Value: self.control }, self.name.as_ptr(), props.as_mut_ptr() as *mut _, 1);
            }
        }
    }
}

/// Entry point for `--fps-helper <out> <stop> <parent pid>`. Returns the process exit code.
#[cfg(windows)]
pub fn run_helper(args: &[String]) -> i32 {
    let (Some(out), Some(stop), Some(ppid)) = (args.first(), args.get(1), args.get(2).and_then(|p| p.parse::<u32>().ok())) else {
        return 2;
    };
    let mut file = match std::fs::OpenOptions::new().append(true).create(true).open(out) {
        Ok(f) => f,
        Err(_) => return 3,
    };
    let mut write = |s: &FpsSample| {
        if let Ok(line) = serde_json::to_string(s) {
            let _ = writeln!(file, "{line}");
            let _ = file.flush();
        }
    };
    let trace = match etw::start() {
        Ok(t) => t,
        Err(e) => {
            let needs_admin = e.contains("Administrator");
            write(&FpsSample { t: crate::engine::types::now_ms(), processes: vec![], error: Some(e), needs_admin });
            return 1;
        }
    };

    let mut sys = sysinfo::System::new();
    let mut window_start = Instant::now();
    loop {
        std::thread::sleep(Duration::from_millis(1000));
        let window_ms = window_start.elapsed().as_secs_f64() * 1000.0;
        window_start = Instant::now();
        let batch = etw::PRESENTS.lock().ok().and_then(|mut g| g.take()).unwrap_or_default();

        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        if sys.process(sysinfo::Pid::from_u32(ppid)).is_none() || std::path::Path::new(stop).exists() {
            break;
        }
        // Blob Optimizers' own window (WebView2 helpers) renders too; never report it as a game.
        let is_ours = |pid: u32| {
            let mut cur = Some(sysinfo::Pid::from_u32(pid));
            for _ in 0..6 {
                match cur {
                    Some(p) if p.as_u32() == ppid => return true,
                    Some(p) => cur = sys.process(p).and_then(|x| x.parent()),
                    None => return false,
                }
            }
            false
        };

        let mut processes: Vec<FpsProcess> = batch
            .into_iter()
            .filter(|(pid, _)| !is_ours(*pid))
            .filter_map(|(pid, mut times)| {
                let (fps, frame_ms, low1_fps) = summarize(&mut times, window_ms)?;
                let name = sys.process(sysinfo::Pid::from_u32(pid)).map(|p| p.name().to_string_lossy().into_owned()).unwrap_or_else(|| format!("PID {pid}"));
                // Desktop composition helpers present a few times a second; keep real render loops.
                (fps >= 5.0).then_some(FpsProcess { pid, name, fps, frame_ms, low1_fps })
            })
            .collect();
        processes.sort_by(|a, b| b.fps.partial_cmp(&a.fps).unwrap_or(std::cmp::Ordering::Equal));
        write(&FpsSample { t: crate::engine::types::now_ms(), processes, error: None, needs_admin: false });
    }
    trace.stop();
    let _ = std::fs::remove_file(stop);
    let _ = std::fs::remove_file(out);
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarize_steady_and_stuttering() {
        // 60 frames evenly spaced over one second.
        let mut t: Vec<f64> = (0..60).map(|i| i as f64 * 1000.0 / 60.0).collect();
        let (fps, ms, low) = summarize(&mut t, 1000.0).unwrap();
        assert!((fps - 60.0).abs() < 0.5 && (ms - 16.67).abs() < 0.1 && (low - 60.0).abs() < 1.0, "{fps} {ms} {low}");
        // One 100 ms hitch drags the 1% low down while the average barely moves.
        let mut t: Vec<f64> = (0..60).map(|i| i as f64 * 15.0 + if i >= 30 { 85.0 } else { 0.0 }).collect();
        let (_, _, low) = summarize(&mut t, 1000.0).unwrap();
        assert!(low < 11.0, "{low}");
        assert!(summarize(&mut [1.0], 1000.0).is_none());
    }

    /// Either collects present events, or fails with a readable message (no crash, session cleaned up).
    #[cfg(windows)]
    #[test]
    fn etw_session_starts_or_explains() {
        match etw::start() {
            Ok(trace) => {
                std::thread::sleep(Duration::from_secs(3));
                let counts: Vec<(u32, usize)> = etw::PRESENTS
                    .lock()
                    .unwrap()
                    .take()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(p, v)| (p, v.len()))
                    .collect();
                println!("present events per pid over 3 s: {counts:?}");
                trace.stop();
            }
            Err(e) => {
                println!("etw start failed: {e}");
                assert!(!e.is_empty());
            }
        }
    }
}
