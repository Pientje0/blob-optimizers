//! Read-only native registry access (fast state detection). All writes go through the command engine.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub enum RegData {
    Dword(u32),
    Qword(u64),
    String(String),
    ExpandString(String),
    MultiString(Vec<String>),
    Binary(Vec<u8>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Hive {
    #[serde(rename = "HKCU")]
    CurrentUser,
    #[serde(rename = "HKLM")]
    LocalMachine,
}

impl Hive {
    /// PowerShell provider prefix.
    pub fn ps(self) -> &'static str {
        match self {
            Hive::CurrentUser => "HKCU:",
            Hive::LocalMachine => "HKLM:",
        }
    }
    /// reg.exe prefix.
    pub fn reg(self) -> &'static str {
        match self {
            Hive::CurrentUser => "HKCU",
            Hive::LocalMachine => "HKLM",
        }
    }
}

#[cfg(windows)]
mod imp {
    use super::*;
    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegGetValueW, RegOpenKeyExW, HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, REG_BINARY,
        REG_DWORD, REG_EXPAND_SZ, REG_MULTI_SZ, REG_QWORD, REG_SZ, RRF_NOEXPAND, RRF_RT_ANY,
    };

    fn root(h: Hive) -> HKEY {
        match h {
            Hive::CurrentUser => HKEY_CURRENT_USER,
            Hive::LocalMachine => HKEY_LOCAL_MACHINE,
        }
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub fn key_exists(h: Hive, path: &str) -> bool {
        unsafe {
            let p = wide(path);
            let mut key: HKEY = std::mem::zeroed();
            if RegOpenKeyExW(root(h), p.as_ptr(), 0, KEY_READ, &mut key) == ERROR_SUCCESS {
                RegCloseKey(key);
                true
            } else {
                false
            }
        }
    }

    /// Reads a value; `name` "" is the key's default value.
    pub fn read(h: Hive, path: &str, name: &str) -> Option<RegData> {
        unsafe {
            let p = wide(path);
            let n = wide(name);
            let mut ty = 0u32;
            let mut size = 0u32;
            let flags = RRF_RT_ANY | RRF_NOEXPAND;
            if RegGetValueW(root(h), p.as_ptr(), n.as_ptr(), flags, &mut ty, std::ptr::null_mut(), &mut size) != ERROR_SUCCESS {
                return None;
            }
            let mut buf = vec![0u8; size as usize + 2];
            if RegGetValueW(root(h), p.as_ptr(), n.as_ptr(), flags, &mut ty, buf.as_mut_ptr().cast(), &mut size) != ERROR_SUCCESS {
                return None;
            }
            buf.truncate(size as usize);
            let utf16 = |b: &[u8]| -> Vec<u16> { b.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes([c[0], c[1]])).collect() };
            let text = |b: &[u8]| String::from_utf16_lossy(&utf16(b)).trim_end_matches('\0').to_string();
            Some(match ty {
                REG_DWORD if buf.len() >= 4 => RegData::Dword(u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]])),
                REG_QWORD if buf.len() >= 8 => RegData::Qword(u64::from_le_bytes(buf[..8].try_into().ok()?)),
                REG_SZ => RegData::String(text(&buf)),
                REG_EXPAND_SZ => RegData::ExpandString(text(&buf)),
                REG_MULTI_SZ => RegData::MultiString(
                    String::from_utf16_lossy(&utf16(&buf)).split('\0').filter(|s| !s.is_empty()).map(String::from).collect(),
                ),
                REG_BINARY => RegData::Binary(buf),
                _ => RegData::Binary(buf),
            })
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    pub fn key_exists(_h: Hive, _path: &str) -> bool {
        false
    }
    pub fn read(_h: Hive, _path: &str, _name: &str) -> Option<RegData> {
        None
    }
}

pub use imp::{key_exists, read};

/// PowerShell statement that writes `data` to `path\name` (creating the key), as used by engine scripts.
pub fn ps_set(h: Hive, path: &str, name: &str, data: &RegData) -> String {
    let q = |s: &str| s.replace('\'', "''");
    let key = format!("{}\\{}", h.ps(), q(path));
    let (ty, value) = match data {
        RegData::Dword(v) => ("DWord", v.to_string()),
        RegData::Qword(v) => ("QWord", v.to_string()),
        RegData::String(s) => ("String", format!("'{}'", q(s))),
        RegData::ExpandString(s) => ("ExpandString", format!("'{}'", q(s))),
        RegData::MultiString(v) => ("MultiString", format!("@({})", v.iter().map(|s| format!("'{}'", q(s))).collect::<Vec<_>>().join(","))),
        RegData::Binary(b) => ("Binary", format!("([byte[]]@({}))", b.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(","))),
    };
    if name.is_empty() {
        // The default value: Set-Item writes it as REG_SZ.
        return format!("if (-not (Test-Path -LiteralPath '{key}')) {{ New-Item -Path '{key}' -Force | Out-Null }}\nSet-Item -LiteralPath '{key}' -Value {value}\n");
    }
    format!(
        "if (-not (Test-Path -LiteralPath '{key}')) {{ New-Item -Path '{key}' -Force | Out-Null }}\nNew-ItemProperty -LiteralPath '{key}' -Name '{}' -PropertyType {ty} -Value {value} -Force | Out-Null\n",
        q(name)
    )
}

pub fn ps_delete_value(h: Hive, path: &str, name: &str) -> String {
    let q = |s: &str| s.replace('\'', "''");
    format!("Remove-ItemProperty -LiteralPath '{}\\{}' -Name '{}' -ErrorAction SilentlyContinue\n", h.ps(), q(path), q(name))
}

pub fn ps_delete_key(h: Hive, path: &str) -> String {
    let q = |s: &str| s.replace('\'', "''");
    format!("Remove-Item -LiteralPath '{}\\{}' -Recurse -Force -ErrorAction SilentlyContinue\n", h.ps(), q(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_known_values() {
        let v = read(Hive::LocalMachine, r"SOFTWARE\Microsoft\Windows NT\CurrentVersion", "CurrentBuildNumber");
        assert!(matches!(v, Some(RegData::String(ref s)) if s.parse::<u32>().is_ok()), "{v:?}");
        assert!(key_exists(Hive::CurrentUser, r"Software\Microsoft"));
        assert!(!key_exists(Hive::CurrentUser, r"Software\Blob\Definitely\Missing"));
        assert_eq!(read(Hive::CurrentUser, r"Software\Microsoft", "definitely-missing"), None);
    }
}
