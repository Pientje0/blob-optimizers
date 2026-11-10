//! Executes one job: launches the process, streams output, handles progress, cancel and timeout.

use std::fs::File;
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use regex::Regex;
use serde::Serialize;

use super::decoder::{DecodedLine, LineDecoder};
use super::launch::{self, Launch};
use super::types::*;
use super::{Inner, Job, MEMORY_LINES};

const TICK: Duration = Duration::from_millis(60);
const FLUSH_EVERY: Duration = Duration::from_millis(90);
const PROGRESS_PREFIX: &str = "##PROGRESS ";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OutputEvent<'a> {
    job_id: &'a str,
    lines: &'a [OutputLine],
}

enum Msg {
    Line(Stream, DecodedLine),
}

/// How the process ended, before mapping to a JobStatus.
enum End {
    Exited(i32),
    Cancelled,
    TimedOut,
    Declined,
    LaunchFailed(String),
}

struct Session {
    inner: Arc<Inner>,
    job: Arc<Job>,
    id: String,
    log: Option<BufWriter<File>>,
    pending: Vec<OutputLine>,
    last_flush: Instant,
    line_count: usize,
    progress_re: Option<Regex>,
    last_progress: Option<f32>,
}

impl Session {
    fn push(&mut self, stream: Stream, text: String, transient: bool) {
        // Scripts can report structured progress with "##PROGRESS <percent> <message>"; not shown as output.
        if stream == Stream::Out {
            if let Some(rest) = text.strip_prefix(PROGRESS_PREFIX) {
                let (pct, msg) = rest.split_once(' ').unwrap_or((rest, ""));
                if let Ok(p) = pct.trim().parse::<f32>() {
                    self.set_progress(p, (!msg.trim().is_empty()).then(|| msg.trim().to_string()));
                    return;
                }
            }
            if let Some(re) = &self.progress_re {
                if let Some(p) = re.captures(&text).and_then(|c| c.get(1)).and_then(|m| m.as_str().parse::<f32>().ok()) {
                    self.set_progress(p, None);
                }
            }
        }

        let line = OutputLine { stream, text, transient };
        if let Some(log) = self.log.as_mut() {
            if let Ok(json) = serde_json::to_string(&line) {
                let _ = writeln!(log, "{json}");
            }
        }
        if let Ok(mut lines) = self.job.lines.lock() {
            // Collapse consecutive in-place updates in memory, like a real console would.
            if lines.back().is_some_and(|l| l.transient) {
                lines.pop_back();
            }
            lines.push_back(line.clone());
            while lines.len() > MEMORY_LINES {
                lines.pop_front();
            }
        }
        self.line_count += 1;
        self.pending.push(line);
        if self.pending.len() >= 400 {
            self.flush();
        }
    }

    fn info(&mut self, text: impl Into<String>) {
        self.push(Stream::Info, text.into(), false);
    }

    fn set_progress(&mut self, percent: f32, message: Option<String>) {
        let percent = percent.clamp(0.0, 100.0);
        // Only notify the UI on visible changes.
        if message.is_none() && self.last_progress.is_some_and(|p| (p - percent).abs() < 0.5) {
            return;
        }
        self.last_progress = Some(percent);
        let entry = {
            let mut e = self.job.entry.lock().expect("entry lock");
            e.progress = Some(Progress {
                percent,
                message: message.or_else(|| e.progress.as_ref().and_then(|p| p.message.clone())),
            });
            e.clone()
        };
        self.flush();
        self.inner.emit("engine:job", &entry);
    }

    fn flush(&mut self) {
        if !self.pending.is_empty() {
            self.inner.emit("engine:output", &OutputEvent { job_id: &self.id, lines: &self.pending });
            self.pending.clear();
        }
        self.last_flush = Instant::now();
    }

    fn drain(&mut self, rx: &Receiver<Msg>, wait: Duration) {
        match rx.recv_timeout(wait) {
            Ok(Msg::Line(s, l)) => self.push(s, l.text, l.transient),
            Err(RecvTimeoutError::Timeout) | Err(RecvTimeoutError::Disconnected) => {}
        }
        while let Ok(Msg::Line(s, l)) = rx.try_recv() {
            self.push(s, l.text, l.transient);
        }
        if self.last_flush.elapsed() >= FLUSH_EVERY {
            self.flush();
        }
    }
}

pub fn run(inner: Arc<Inner>, job: Arc<Job>, req: JobRequest) {
    let id = job.entry.lock().map(|e| e.id.clone()).unwrap_or_default();
    let log = File::create(inner.log_path(&id)).ok().map(BufWriter::new);
    let mut s = Session {
        inner: inner.clone(),
        job: job.clone(),
        id: id.clone(),
        log,
        pending: Vec::new(),
        last_flush: Instant::now(),
        line_count: 0,
        progress_re: req.progress_pattern.and_then(|p| Regex::new(p).ok()),
        last_progress: None,
    };

    let needs_uac = req.requires_admin && !inner.app_elevated;
    let started = Instant::now();
    let end = match launch::build(req.shell, &req.command) {
        Err(e) => End::LaunchFailed(e),
        Ok(launch) if needs_uac => {
            s.info("Requesting administrator permission…");
            s.flush();
            run_elevated(&mut s, &launch, req.timeout, req.kill_tree)
        }
        Ok(launch) => run_direct(&mut s, &launch, req.timeout, req.kill_tree),
    };
    let elapsed = started.elapsed().as_secs_f32();

    let (status, exit_code, error) = match end {
        End::Exited(0) => (JobStatus::Succeeded, Some(0), None),
        End::Exited(code) => (JobStatus::Failed, Some(code), Some(format!("Exited with code {code}"))),
        End::Cancelled => (JobStatus::Cancelled, None, Some("Cancelled by user".to_string())),
        End::TimedOut => (
            JobStatus::TimedOut,
            None,
            Some(format!("Stopped after exceeding the {} s time limit", req.timeout.as_secs())),
        ),
        End::Declined => (JobStatus::Declined, None, Some("Administrator permission was declined".to_string())),
        End::LaunchFailed(e) => (JobStatus::Failed, None, Some(e)),
    };

    let summary = match status {
        JobStatus::Succeeded => format!("✔ Completed in {elapsed:.1} s"),
        _ => format!("✖ {} ({elapsed:.1} s)", error.clone().unwrap_or_default()),
    };
    s.push(if status == JobStatus::Succeeded { Stream::Info } else { Stream::Err }, summary, false);
    s.flush();
    if let Some(mut log) = s.log.take() {
        let _ = log.flush();
    }

    let entry = {
        let mut e = job.entry.lock().expect("entry lock");
        e.finished_at = Some(now_ms());
        e.status = status;
        e.exit_code = exit_code;
        e.error = error;
        e.line_count = s.line_count;
        if status == JobStatus::Succeeded {
            if let Some(p) = e.progress.as_mut() {
                p.percent = 100.0;
            }
        }
        e.clone()
    };
    inner.history.append(&entry);
    if let Some(cb) = &req.on_complete {
        (cb.0)(&entry);
    }
    if let Ok(mut jobs) = inner.jobs.lock() {
        jobs.remove(&id);
    }
    inner.emit("engine:job", &entry);
}

fn spawn_reader<R: Read + Send + 'static>(mut r: R, stream: Stream, tx: Sender<Msg>) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut dec = LineDecoder::new();
        let mut buf = [0u8; 8192];
        loop {
            match r.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    for l in dec.push(&buf[..n]) {
                        let _ = tx.send(Msg::Line(stream, l));
                    }
                }
            }
        }
        for l in dec.finish() {
            let _ = tx.send(Msg::Line(stream, l));
        }
    })
}

fn run_direct(s: &mut Session, launch: &Launch, timeout: Duration, kill_tree: bool) -> End {
    let stop = |pid: u32, child: &mut std::process::Child| {
        if kill_tree {
            launch::kill_tree(pid);
        }
        let _ = child.kill();
    };
    let mut child = match launch::spawn_direct(launch) {
        Ok(c) => c,
        Err(e) => return End::LaunchFailed(format!("Could not start {}: {e}", launch.program)),
    };
    let pid = child.id();
    let (tx, rx) = mpsc::channel();
    let readers = [
        spawn_reader(child.stdout.take().expect("piped"), Stream::Out, tx.clone()),
        spawn_reader(child.stderr.take().expect("piped"), Stream::Err, tx),
    ];
    let started = Instant::now();
    let mut forced: Option<End> = None;

    let code = loop {
        s.drain(&rx, TICK);
        match child.try_wait() {
            Ok(Some(status)) => break status.code().unwrap_or(-1),
            Ok(None) => {}
            Err(e) => {
                forced = Some(End::LaunchFailed(e.to_string()));
                break -1;
            }
        }
        if forced.is_none() && s.job.cancel.load(Ordering::SeqCst) {
            s.info(if kill_tree { "Cancelling…" } else { "Stopped waiting. Programs this operation opened keep running." });
            stop(pid, &mut child);
            forced = Some(End::Cancelled);
        } else if forced.is_none() && started.elapsed() > timeout {
            stop(pid, &mut child);
            forced = Some(End::TimedOut);
        }
    };
    // Readers finish at EOF; a grandchild holding the pipe open must not hang us forever.
    let deadline = Instant::now() + Duration::from_secs(3);
    for r in readers {
        while !r.is_finished() && Instant::now() < deadline {
            s.drain(&rx, TICK);
        }
    }
    s.drain(&rx, Duration::ZERO);
    forced.unwrap_or(End::Exited(code))
}

/// Incrementally reads a file another process is still writing.
struct Tail {
    path: PathBuf,
    offset: u64,
    decoder: LineDecoder,
    stream: Stream,
}

impl Tail {
    fn new(path: PathBuf, stream: Stream) -> Self {
        Self { path, offset: 0, decoder: LineDecoder::new(), stream }
    }

    fn poll(&mut self, s: &mut Session) {
        let Ok(mut f) = File::open(&self.path) else { return };
        if f.seek(SeekFrom::Start(self.offset)).is_err() {
            return;
        }
        let mut buf = Vec::new();
        if let Ok(n) = f.read_to_end(&mut buf) {
            self.offset += n as u64;
            for l in self.decoder.push(&buf) {
                s.push(self.stream, l.text, l.transient);
            }
        }
    }

    fn finish(&mut self, s: &mut Session) {
        self.poll(s);
        for l in self.decoder.finish() {
            s.push(self.stream, l.text, l.transient);
        }
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(windows)]
fn run_elevated(s: &mut Session, launch: &Launch, timeout: Duration, kill_tree: bool) -> End {
    use super::launch::elevated::ElevatedProcess;
    use super::launch::ElevateError;

    let dir = s.inner.run_dir();
    let out = dir.join(format!("{}.out", s.id));
    let err = dir.join(format!("{}.err", s.id));
    let cancel = dir.join(format!("{}.cancel", s.id));
    let script = launch::elevated_script(launch, &out, &err, &cancel, kill_tree);

    let proc = match ElevatedProcess::start(&script) {
        Ok(p) => p,
        Err(ElevateError::Declined) => return End::Declined,
        Err(ElevateError::Failed(e)) => return End::LaunchFailed(e),
    };
    s.info("Running with administrator rights.");

    let mut tails = [Tail::new(out, Stream::Out), Tail::new(err, Stream::Err)];
    let started = Instant::now();
    let mut forced: Option<(End, Instant)> = None;

    let code = loop {
        for t in tails.iter_mut() {
            t.poll(s);
        }
        if s.last_flush.elapsed() >= FLUSH_EVERY {
            s.flush();
        }
        if let Some(code) = proc.try_exit_code() {
            break code;
        }
        match &forced {
            None if s.job.cancel.load(Ordering::SeqCst) => {
                s.info("Cancelling…");
                let _ = File::create(&cancel);
                forced = Some((End::Cancelled, Instant::now()));
            }
            None if started.elapsed() > timeout => {
                let _ = File::create(&cancel);
                forced = Some((End::TimedOut, Instant::now()));
            }
            // The wrapper polls the cancel file every 150 ms; terminate it if it does not respond.
            Some((_, at)) if at.elapsed() > Duration::from_secs(10) => proc.terminate(),
            _ => {}
        }
        thread::sleep(Duration::from_millis(100));
    };

    for t in tails.iter_mut() {
        t.finish(s);
    }
    let _ = std::fs::remove_file(&cancel);
    match (forced, code) {
        (Some((end, _)), _) => end,
        (None, 9001) => End::LaunchFailed("The elevated process could not start the command.".into()),
        (None, code) => End::Exited(code),
    }
}

#[cfg(not(windows))]
fn run_elevated(_s: &mut Session, _launch: &Launch, _timeout: Duration, _kill_tree: bool) -> End {
    End::LaunchFailed("Elevation is only supported on Windows.".into())
}
