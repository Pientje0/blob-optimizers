//! CPU and GPU utilisation via Windows Performance Data Helper (PDH) counters - the same sources
//! Task Manager uses. One in-process query is far cheaper than spawning PowerShell every poll.

use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct PerfSample {
    /// Total CPU utility, 0-100 (Task Manager's "Utilization").
    pub cpu_total: Option<f32>,
    /// Per logical processor utility, ordered by processor index.
    pub per_core: Vec<f32>,
    /// Current performance relative to base clock, in percent (e.g. 124 = 24% above base).
    pub cpu_performance: Option<f32>,
    /// Busiest engine type on the busiest GPU, 0-100.
    pub gpu_usage: Option<f32>,
    /// Dedicated VRAM in use on the busiest GPU, bytes.
    pub gpu_dedicated_used: Option<u64>,
}

#[cfg(windows)]
mod imp {
    use super::*;
    use windows_sys::Win32::System::Performance::{
        PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterArrayW,
        PdhGetFormattedCounterValue, PdhOpenQueryW, PDH_FMT_COUNTERVALUE,
        PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE,
    };

    const PDH_MORE_DATA: u32 = 0x8000_07D2;
    const PDH_FMT_NOCAP100: u32 = 0x0000_8000;
    const FORMAT: u32 = PDH_FMT_DOUBLE | PDH_FMT_NOCAP100;

    /// Counter handle; 0 means the counter is unavailable on this system.
    type Counter = isize;

    pub struct PerfCounters {
        query: isize,
        cpu_utility: Counter,
        cpu_performance: Counter,
        gpu_engine: Counter,
        gpu_memory: Counter,
    }

    // PDH handles are opaque and may be used from any thread; access is serialised by the
    // Mutex that owns the monitor.
    unsafe impl Send for PerfCounters {}

    impl PerfCounters {
        pub fn new() -> Option<Self> {
            unsafe {
                let mut query = 0isize;
                if PdhOpenQueryW(std::ptr::null(), 0, &mut query) != 0 {
                    return None;
                }
                let add = |path: &str| -> Counter {
                    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
                    let mut counter = 0isize;
                    if PdhAddEnglishCounterW(query, wide.as_ptr(), 0, &mut counter) != 0 {
                        0
                    } else {
                        counter
                    }
                };
                let me = Self {
                    query,
                    cpu_utility: add(r"\Processor Information(*)\% Processor Utility"),
                    cpu_performance: add(r"\Processor Information(_Total)\% Processor Performance"),
                    gpu_engine: add(r"\GPU Engine(*)\Utilization Percentage"),
                    gpu_memory: add(r"\GPU Adapter Memory(*)\Dedicated Usage"),
                };
                // Rate counters need a first sample before they report values.
                PdhCollectQueryData(query);
                Some(me)
            }
        }

        unsafe fn read_single(&self, counter: Counter) -> Option<f64> {
            if counter == 0 {
                return None;
            }
            let mut value: PDH_FMT_COUNTERVALUE = std::mem::zeroed();
            let mut ty = 0u32;
            if PdhGetFormattedCounterValue(counter, FORMAT, &mut ty, &mut value) != 0 || value.CStatus > 1 {
                return None;
            }
            Some(value.Anonymous.doubleValue)
        }

        /// Reads every instance of a wildcard counter as (instance name, value).
        unsafe fn read_array(&self, counter: Counter) -> Vec<(String, f64)> {
            if counter == 0 {
                return Vec::new();
            }
            let mut size = 0u32;
            let mut count = 0u32;
            let status =
                PdhGetFormattedCounterArrayW(counter, FORMAT, &mut size, &mut count, std::ptr::null_mut());
            if status != PDH_MORE_DATA || size == 0 {
                return Vec::new();
            }
            // The buffer holds the item structs followed by the instance-name strings; allocate it
            // as items so it is correctly aligned for them.
            let item_size = std::mem::size_of::<PDH_FMT_COUNTERVALUE_ITEM_W>();
            let mut buf: Vec<PDH_FMT_COUNTERVALUE_ITEM_W> =
                Vec::with_capacity((size as usize).div_ceil(item_size));
            if PdhGetFormattedCounterArrayW(counter, FORMAT, &mut size, &mut count, buf.as_mut_ptr()) != 0 {
                return Vec::new();
            }
            buf.set_len(count as usize);
            buf.iter()
                .filter(|it| it.FmtValue.CStatus <= 1)
                .map(|it| {
                    let name = if it.szName.is_null() {
                        String::new()
                    } else {
                        let mut len = 0;
                        while *it.szName.add(len) != 0 {
                            len += 1;
                        }
                        String::from_utf16_lossy(std::slice::from_raw_parts(it.szName, len))
                    };
                    (name, it.FmtValue.Anonymous.doubleValue)
                })
                .collect()
        }

        pub fn sample(&mut self) -> PerfSample {
            unsafe {
                if PdhCollectQueryData(self.query) != 0 {
                    return PerfSample::default();
                }
                let mut out = PerfSample::default();

                // CPU instances look like "0,5" (group,index), "0,_Total" and "_Total".
                let mut cores: Vec<(u32, f32)> = Vec::new();
                for (name, v) in self.read_array(self.cpu_utility) {
                    let v = v.clamp(0.0, 100.0) as f32;
                    if name == "_Total" {
                        out.cpu_total = Some(v);
                    } else if let Some((group, idx)) = name.split_once(',') {
                        if let (Ok(g), Ok(i)) = (group.parse::<u32>(), idx.parse::<u32>()) {
                            cores.push((g * 64 + i, v));
                        }
                    }
                }
                cores.sort_by_key(|(i, _)| *i);
                out.per_core = cores.into_iter().map(|(_, v)| v).collect();
                out.cpu_performance = self.read_single(self.cpu_performance).map(|v| v as f32);

                // GPU engine instances: pid_1234_luid_0x0000_0x1234_phys_0_eng_3_engtype_3D.
                // Sum per (adapter, engine type) across processes, then take the busiest.
                let mut per_engine: HashMap<(String, String), f64> = HashMap::new();
                for (name, value) in self.read_array(self.gpu_engine) {
                    let luid = extract(&name, "luid_", "_phys").unwrap_or_default();
                    let engtype = name.rsplit("engtype_").next().unwrap_or("").to_string();
                    *per_engine.entry((luid, engtype)).or_default() += value;
                }
                if self.gpu_engine != 0 {
                    let (luid, usage) = per_engine
                        .into_iter()
                        .map(|((luid, _), v)| (luid, v))
                        .fold((String::new(), 0.0f64), |acc, x| if x.1 > acc.1 { x } else { acc });
                    out.gpu_usage = Some(usage.clamp(0.0, 100.0) as f32);

                    let mem = self.read_array(self.gpu_memory);
                    out.gpu_dedicated_used = mem
                        .iter()
                        .find(|(n, _)| !luid.is_empty() && n.contains(&luid))
                        .map(|(_, v)| *v as u64)
                        .or_else(|| mem.iter().map(|(_, v)| *v as u64).max())
                        .filter(|v| *v > 0);
                }
                out
            }
        }
    }

    fn extract(s: &str, start: &str, end: &str) -> Option<String> {
        let a = s.find(start)? + start.len();
        let b = s[a..].find(end)? + a;
        Some(s[a..b].to_string())
    }

    impl Drop for PerfCounters {
        fn drop(&mut self) {
            unsafe {
                PdhCloseQuery(self.query);
            }
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    pub struct PerfCounters;
    impl PerfCounters {
        pub fn new() -> Option<Self> {
            None
        }
        pub fn sample(&mut self) -> PerfSample {
            let _ = HashMap::<(), ()>::new();
            PerfSample::default()
        }
    }
}

pub use imp::PerfCounters;
