//! Minimal hidden PowerShell runner used for read-only system queries (hardware detection etc.).
//! System-modifying operations go through the full command engine instead.

use std::io::Read;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use base64::Engine;

use crate::error::{AppError, AppResult};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

/// Do not create a console window for child processes.
#[cfg(windows)]
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Encodes a script for `-EncodedCommand` (UTF-16LE + base64), which sidesteps all quoting issues.
pub fn encode_script(script: &str) -> String {
    let prelude = "$ProgressPreference='SilentlyContinue';[Console]::OutputEncoding=[Text.Encoding]::UTF8;\n";
    encode_raw(&format!("{prelude}{script}"))
}

/// `-EncodedCommand` encoding without any prelude.
pub fn encode_raw(script: &str) -> String {
    let utf16: Vec<u8> = script.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    base64::engine::general_purpose::STANDARD.encode(utf16)
}

/// Builds a hidden, non-interactive `powershell.exe` command for the given script.
pub fn command(script: &str) -> Command {
    let mut cmd = Command::new("powershell.exe");
    cmd.args([
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-EncodedCommand",
        &encode_script(script),
    ]);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

/// Runs a script to completion and returns its stdout. Kills the process if it exceeds `timeout`.
pub fn run(script: &str, timeout: Duration) -> AppResult<String> {
    let mut child = command(script)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    // Drain pipes on separate threads so a chatty process can never deadlock on a full buffer.
    let mut stdout = child.stdout.take().expect("stdout piped");
    let mut stderr = child.stderr.take().expect("stderr piped");
    let out_handle = thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout.read_to_end(&mut buf);
        buf
    });
    let err_handle = thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stderr.read_to_end(&mut buf);
        buf
    });

    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if started.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(AppError::Timeout(timeout.as_secs()));
        }
        thread::sleep(Duration::from_millis(25));
    };

    let out = String::from_utf8_lossy(&out_handle.join().unwrap_or_default()).into_owned();
    let err = String::from_utf8_lossy(&err_handle.join().unwrap_or_default()).into_owned();

    if !status.success() {
        return Err(AppError::CommandFailed {
            code: status.code().unwrap_or(-1),
            stderr: err.trim().to_string(),
        });
    }
    Ok(out)
}

/// Runs a script whose stdout is a single JSON document and deserializes it.
pub fn run_json<T: serde::de::DeserializeOwned>(script: &str, timeout: Duration) -> AppResult<T> {
    let out = run(script, timeout)?;
    // Strip a UTF-8 BOM if PowerShell emitted one.
    let trimmed = out.trim_start_matches('\u{feff}').trim();
    Ok(serde_json::from_str(trimmed)?)
}
