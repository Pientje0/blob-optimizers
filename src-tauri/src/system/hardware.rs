//! Static hardware & Windows configuration, collected by `scripts/hardware.ps1`.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::AppResult;
use crate::util::powershell;
use crate::util::serde_helpers::one_or_many;

const SCRIPT: &str = include_str!("../../scripts/hardware.ps1");
const TIMEOUT: Duration = Duration::from_secs(45);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HardwareInfo {
    pub windows: WindowsInfo,
    pub cpu: CpuInfo,
    #[serde(deserialize_with = "one_or_many")]
    pub gpus: Vec<GpuInfo>,
    pub memory: MemoryInfo,
    #[serde(deserialize_with = "one_or_many")]
    pub disks: Vec<DiskInfo>,
    pub board: BoardInfo,
    #[serde(deserialize_with = "one_or_many")]
    pub network: Vec<NetAdapterInfo>,
    #[serde(deserialize_with = "one_or_many")]
    pub monitors: Vec<MonitorInfo>,
    pub config: PerfConfig,
    pub collected_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowsInfo {
    pub product_name: String,
    pub display_version: String,
    pub build: String,
    pub edition: String,
    pub architecture: String,
    pub install_date: Option<String>,
    pub computer_name: String,
    pub last_boot: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CpuInfo {
    pub name: String,
    pub manufacturer: String,
    pub cores: u32,
    pub threads: u32,
    pub max_clock_mhz: u32,
    pub l2_cache_kb: u32,
    pub l3_cache_kb: u32,
    pub socket: String,
    pub virtualization: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuInfo {
    pub name: String,
    pub manufacturer: String,
    pub vram_bytes: Option<u64>,
    pub driver_version: String,
    pub driver_date: Option<String>,
    pub resolution: Option<String>,
    pub refresh_rate: Option<u32>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryInfo {
    pub total_bytes: u64,
    #[serde(deserialize_with = "one_or_many")]
    pub modules: Vec<MemoryModule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryModule {
    pub capacity_bytes: u64,
    pub speed_mhz: u32,
    pub configured_mhz: u32,
    pub manufacturer: String,
    pub part_number: String,
    pub slot: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskInfo {
    pub number: u32,
    pub model: String,
    pub media_type: String,
    pub bus_type: String,
    pub size_bytes: u64,
    pub health: String,
    pub temperature: Option<i32>,
    pub wear_percent: Option<i32>,
    #[serde(deserialize_with = "one_or_many")]
    pub volumes: Vec<VolumeInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeInfo {
    pub letter: String,
    pub label: String,
    pub file_system: String,
    pub size_bytes: u64,
    pub free_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardInfo {
    pub manufacturer: String,
    pub product: String,
    pub system_model: String,
    pub bios_vendor: String,
    pub bios_version: String,
    pub bios_date: Option<String>,
    pub firmware: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetAdapterInfo {
    pub name: String,
    pub description: String,
    pub status: String,
    pub link_speed: String,
    pub mac: String,
    pub media_type: String,
    pub ipv4: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorInfo {
    pub name: Option<String>,
    pub manufacturer: Option<String>,
    pub serial: Option<String>,
    pub size_inches: Option<f64>,
    pub year: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerfConfig {
    pub power_plan: Option<PowerPlan>,
    pub game_mode_enabled: bool,
    pub hags_enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PowerPlan {
    pub guid: String,
    pub name: String,
}

pub fn detect() -> AppResult<HardwareInfo> {
    powershell::run_json(SCRIPT, TIMEOUT)
}
