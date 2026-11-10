use std::sync::Mutex;

use crate::system::hardware::HardwareInfo;
use crate::system::live::Monitor;

/// Shared backend state managed by Tauri.
pub struct AppState {
    pub monitor: Mutex<Monitor>,
    pub hardware: Mutex<Option<HardwareInfo>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            monitor: Mutex::new(Monitor::new()),
            hardware: Mutex::new(None),
        }
    }
}
