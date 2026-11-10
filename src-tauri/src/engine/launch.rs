//! Process launching for the command engine: direct (inherits the app's token) and elevated (UAC).

use std::path::Path;
use std::process::{Child, Command, Stdio};

use base64::Engine as _;

use super::types::Shell;
use crate::util::powershell::{encode_raw, CREATE_NO_WINDOW};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

const WRAPPER: &str = include_str!("../../scripts/engine-wrapper.ps1");
const ELEVATED: &str = include_str!("../../scripts/engine-elevated.ps1");

/// Windows limits a command line to 32 767 characters; leave headroom for the program path.
const MAX_COMMAND_LINE: usize = 32_000;

/// A program plus its raw (already quoted) argument string.
#[derive(Debug, Clone)]
pub struct Launch {
    pub program: &'static str,
    pub args: String,
}

pub fn build(shell: Shell, command: &str) -> Result<Launch, String> {
    let launch = match shell {
        Shell::PowerShell => {
            let b64 = base64::engine::general_purpose::STANDARD.encode(command.as_bytes());
            let script = WRAPPER.replace("__BLOB_SCRIPT_B64__", &b64);
            Launch {
                program: "powershell.exe",
                args: format!(
                    "-NoProfile -NonInteractive -ExecutionPolicy Bypass -EncodedCommand {}",
                    encode_raw(&script)
                ),
            }
        }
        Shell::Cmd => {
            if command.contains(['\r', '\n']) {
                return Err("CMD commands must be a single line.".into());
            }
            // /s keeps the outer quotes intact; chcp switches output to UTF-8.
            Launch { program: "cmd.exe", args: format!("/d /s /c \"chcp 65001>nul & {command}\"") }
        }
    };
    if launch.args.len() > MAX_COMMAND_LINE {
        return Err("Command is too long to execute.".into());
    }
    Ok(launch)
}

pub fn spawn_direct(launch: &Launch) -> std::io::Result<Child> {
    let mut cmd = Command::new(launch.program);
    #[cfg(windows)]
    {
        cmd.raw_arg(&launch.args);
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()
}

/// Kills a process and all its children.
pub fn kill_tree(pid: u32) {
    let mut cmd = Command::new("taskkill.exe");
    cmd.args(["/PID", &pid.to_string(), "/T", "/F"]).stdout(Stdio::null()).stderr(Stdio::null());
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let _ = cmd.status();
}

fn ps_quote(s: &str) -> String {
    s.replace('\'', "''")
}

pub fn elevated_script(launch: &Launch, out: &Path, err: &Path, cancel: &Path, kill_tree: bool) -> String {
    ELEVATED
        .replace("__TREE__", if kill_tree { "/T" } else { "" })
        .replace("__OUT__", &ps_quote(&out.to_string_lossy()))
        .replace("__ERR__", &ps_quote(&err.to_string_lossy()))
        .replace("__CANCEL__", &ps_quote(&cancel.to_string_lossy()))
        .replace("__PROGRAM__", launch.program)
        .replace("__ARGS__", &ps_quote(&launch.args))
}

pub enum ElevateError {
    Declined,
    Failed(String),
}

#[cfg(windows)]
pub mod elevated {
    use super::*;
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, HANDLE, WAIT_OBJECT_0};
    use windows_sys::Win32::System::Com::CoInitializeEx;
    use windows_sys::Win32::System::Threading::{GetExitCodeProcess, TerminateProcess, WaitForSingleObject};
    use windows_sys::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};

    const ERROR_CANCELLED: u32 = 1223;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// Handle to a process started through UAC.
    pub struct ElevatedProcess {
        handle: HANDLE,
    }

    unsafe impl Send for ElevatedProcess {}

    /// Starts `program args` with the "runas" verb (shows the UAC prompt) and a hidden window.
    pub fn shell_execute_runas(program: &str, args: &str, wait_handle: bool) -> Result<Option<ElevatedProcess>, ElevateError> {
        unsafe {
            // ShellExecuteEx may use COM; initialising twice on a thread is harmless.
            CoInitializeEx(std::ptr::null(), 0x2 | 0x4);
            let verb = wide("runas");
            let file = wide(program);
            let params = wide(args);
            let mut info: SHELLEXECUTEINFOW = std::mem::zeroed();
            info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
            info.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC;
            info.lpVerb = verb.as_ptr();
            info.lpFile = file.as_ptr();
            info.lpParameters = params.as_ptr();
            // SW_HIDE for helper processes; SW_SHOWNORMAL when relaunching the app itself.
            info.nShow = if wait_handle { 0 } else { 1 };
            if ShellExecuteExW(&mut info) == 0 {
                let code = GetLastError();
                return Err(if code == ERROR_CANCELLED {
                    ElevateError::Declined
                } else {
                    ElevateError::Failed(format!("Could not start elevated process (Windows error {code})"))
                });
            }
            if info.hProcess.is_null() {
                return Ok(None);
            }
            if wait_handle {
                Ok(Some(ElevatedProcess { handle: info.hProcess }))
            } else {
                CloseHandle(info.hProcess);
                Ok(None)
            }
        }
    }

    impl ElevatedProcess {
        pub fn start(script: &str) -> Result<Self, ElevateError> {
            let args = format!(
                "-NoProfile -NonInteractive -ExecutionPolicy Bypass -WindowStyle Hidden -EncodedCommand {}",
                encode_raw(script)
            );
            if args.len() > MAX_COMMAND_LINE {
                return Err(ElevateError::Failed("Command is too long to execute elevated.".into()));
            }
            shell_execute_runas("powershell.exe", &args, true)?
                .ok_or_else(|| ElevateError::Failed("Elevated process handle unavailable".into()))
        }

        /// Exit code once the process has finished.
        pub fn try_exit_code(&self) -> Option<i32> {
            unsafe {
                if WaitForSingleObject(self.handle, 0) != WAIT_OBJECT_0 {
                    return None;
                }
                let mut code = 0u32;
                GetExitCodeProcess(self.handle, &mut code);
                Some(code as i32)
            }
        }

        /// Last resort when the wrapper does not respond to the cancel file.
        pub fn terminate(&self) {
            unsafe {
                TerminateProcess(self.handle, 1);
            }
        }
    }

    impl Drop for ElevatedProcess {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.handle);
            }
        }
    }
}
