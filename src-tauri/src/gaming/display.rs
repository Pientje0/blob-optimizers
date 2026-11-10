//! Displays: current mode and supported refresh rates, and changing the refresh rate.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::engine::types::{JobRequest, Shell};
use crate::error::{AppError, AppResult};
use crate::util::powershell;
use crate::util::serde_helpers::one_or_many;

const SCRIPT: &str = include_str!("../../scripts/display.ps1");

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Display {
    pub device: String,
    pub adapter: String,
    pub monitor: String,
    pub primary: bool,
    pub width: u32,
    pub height: u32,
    pub hz: u32,
    #[serde(deserialize_with = "one_or_many")]
    pub rates: Vec<u32>,
}

fn script(mode: &str, device: &str, hz: u32) -> String {
    SCRIPT.replace("__MODE__", mode).replace("__DEVICE__", &device.replace('\'', "''")).replace("__HZ__", &hz.to_string())
}

pub fn list() -> AppResult<Vec<Display>> {
    #[derive(Deserialize)]
    struct W(#[serde(deserialize_with = "one_or_many")] Vec<Display>);
    let W(d) = powershell::run_json(&script("list", "", 0), Duration::from_secs(30))?;
    Ok(d)
}

/// Only rates the display reports for its current resolution are accepted.
pub fn set_refresh_rate(device: &str, hz: u32) -> AppResult<JobRequest> {
    let d = list()?.into_iter().find(|d| d.device == device).ok_or_else(|| AppError::Other("That display is no longer connected.".into()))?;
    if !d.rates.contains(&hz) {
        return Err(AppError::Other(format!("{} does not support {hz} Hz at {}x{}.", d.monitor, d.width, d.height)));
    }
    let mut r = JobRequest::internal(format!("Set {} to {hz} Hz", d.monitor), Shell::PowerShell, script("set", device, hz));
    r.operation_id = Some("display.refresh_rate".into());
    r.timeout = Duration::from_secs(60);
    Ok(r)
}
