//! Live usage sampling (CPU, memory, GPU, network, disks, processes).
//! The frontend polls `get_live_stats`; each call refreshes only what it reports.

use std::time::Instant;

use serde::Serialize;
use sysinfo::{
    Components, Disks, MemoryRefreshKind, Networks, ProcessRefreshKind, ProcessesToUpdate,
    System,
};

use super::perf::PerfCounters;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveStats {
    pub cpu_usage: f32,
    pub per_core: Vec<f32>,
    pub cpu_freq_mhz: u64,
    pub cpu_temp_c: Option<f32>,
    pub mem_used: u64,
    pub mem_total: u64,
    pub swap_used: u64,
    pub swap_total: u64,
    pub gpu_usage: Option<f32>,
    pub gpu_mem_used: Option<u64>,
    pub net_rx_per_sec: u64,
    pub net_tx_per_sec: u64,
    /// Per-interface throughput (interface alias as shown in Windows, e.g. "Wi-Fi").
    pub interfaces: Vec<InterfaceRate>,
    pub disks: Vec<DiskUsage>,
    pub process_count: usize,
    pub top_processes: Vec<ProcessSummary>,
    pub uptime_secs: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InterfaceRate {
    pub name: String,
    pub rx_per_sec: u64,
    pub tx_per_sec: u64,
    pub total_rx: u64,
    pub total_tx: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskUsage {
    pub mount: String,
    pub name: String,
    pub kind: String,
    pub total: u64,
    pub available: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessSummary {
    pub name: String,
    pub instances: u32,
    pub cpu: f32,
    pub memory: u64,
}

pub struct Monitor {
    sys: System,
    networks: Networks,
    disks: Disks,
    components: Option<Components>,
    perf: Option<PerfCounters>,
    base_clock_mhz: u64,
    last_net: Instant,
    last_disk_refresh: Instant,
}

impl Monitor {
    pub fn new() -> Self {
        let mut sys = System::new();
        sys.refresh_cpu_all();
        sys.refresh_memory_specifics(MemoryRefreshKind::everything());
        // On Windows sysinfo reports the nominal (base) clock, which PDH's performance % scales.
        let base_clock_mhz = sys.cpus().iter().map(|c| c.frequency()).max().unwrap_or(0);
        // Temperature sensors via ACPI WMI are only exposed on some boards; skip entirely if none.
        let components = Components::new_with_refreshed_list();
        let components = (!components.is_empty()).then_some(components);
        Self {
            sys,
            networks: Networks::new_with_refreshed_list(),
            disks: Disks::new_with_refreshed_list(),
            components,
            perf: PerfCounters::new(),
            base_clock_mhz,
            last_net: Instant::now(),
            last_disk_refresh: Instant::now(),
        }
    }

    pub fn sample(&mut self) -> LiveStats {
        let perf = self.perf.as_mut().map(|p| p.sample()).unwrap_or_default();
        let sys = &mut self.sys;
        // sysinfo's CPU usage is only a fallback: on recent Windows builds it can report 100% on
        // every core, so PDH "% Processor Utility" (Task Manager's metric) is preferred.
        if perf.cpu_total.is_none() {
            sys.refresh_cpu_usage();
        }
        sys.refresh_memory_specifics(MemoryRefreshKind::everything());
        sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::new().with_cpu().with_memory(),
        );

        let elapsed = self.last_net.elapsed().as_secs_f64().max(0.001);
        self.networks.refresh();
        self.last_net = Instant::now();
        let (rx, tx) = self
            .networks
            .iter()
            .fold((0u64, 0u64), |(r, t), (_, n)| (r + n.received(), t + n.transmitted()));
        let interfaces = self
            .networks
            .iter()
            .filter(|(_, n)| n.total_received() + n.total_transmitted() > 0)
            .map(|(name, n)| InterfaceRate {
                name: name.clone(),
                rx_per_sec: (n.received() as f64 / elapsed) as u64,
                tx_per_sec: (n.transmitted() as f64 / elapsed) as u64,
                total_rx: n.total_received(),
                total_tx: n.total_transmitted(),
            })
            .collect();

        // Free space changes slowly; refresh at most every 10 s.
        if self.last_disk_refresh.elapsed().as_secs() >= 10 {
            self.disks.refresh_list();
            self.last_disk_refresh = Instant::now();
        }

        let cpu_temp_c = self.components.as_mut().and_then(|c| {
            c.refresh();
            let temps: Vec<f32> = c
                .iter()
                .map(|x| x.temperature())
                .filter(|t| t.is_finite() && *t > 0.0 && *t < 125.0)
                .collect();
            temps.into_iter().reduce(f32::max)
        });

        let cpu_count = sys.cpus().len().max(1) as f32;
        let mut grouped: std::collections::HashMap<String, ProcessSummary> =
            std::collections::HashMap::new();
        for p in sys.processes().values() {
            let name = p.name().to_string_lossy().into_owned();
            if name.is_empty() || name == "System Idle Process" {
                continue;
            }
            let e = grouped.entry(name.clone()).or_insert(ProcessSummary {
                name,
                instances: 0,
                cpu: 0.0,
                memory: 0,
            });
            e.instances += 1;
            e.cpu += p.cpu_usage() / cpu_count;
            e.memory += p.memory();
        }
        let mut top: Vec<ProcessSummary> = grouped.into_values().collect();
        top.sort_by(|a, b| {
            b.cpu
                .partial_cmp(&a.cpu)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(b.memory.cmp(&a.memory))
        });
        top.truncate(6);

        LiveStats {
            cpu_usage: perf.cpu_total.unwrap_or_else(|| sys.global_cpu_usage()),
            per_core: if perf.per_core.is_empty() {
                sys.cpus().iter().map(|c| c.cpu_usage()).collect()
            } else {
                perf.per_core
            },
            cpu_freq_mhz: match perf.cpu_performance {
                Some(p) if p > 0.0 => (self.base_clock_mhz as f32 * p / 100.0) as u64,
                _ => self.base_clock_mhz,
            },
            cpu_temp_c,
            mem_used: sys.used_memory(),
            mem_total: sys.total_memory(),
            swap_used: sys.used_swap(),
            swap_total: sys.total_swap(),
            gpu_usage: perf.gpu_usage,
            gpu_mem_used: perf.gpu_dedicated_used,
            net_rx_per_sec: (rx as f64 / elapsed) as u64,
            net_tx_per_sec: (tx as f64 / elapsed) as u64,
            interfaces,
            disks: self
                .disks
                .iter()
                .map(|d| DiskUsage {
                    mount: d.mount_point().to_string_lossy().trim_end_matches('\\').to_string(),
                    name: d.name().to_string_lossy().into_owned(),
                    kind: format!("{:?}", d.kind()),
                    total: d.total_space(),
                    available: d.available_space(),
                })
                .collect(),
            process_count: sys.processes().len(),
            top_processes: top,
            uptime_secs: System::uptime(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_are_sane() {
        let mut m = Monitor::new();
        m.sample();
        std::thread::sleep(std::time::Duration::from_millis(500));
        let s = m.sample();
        println!(
            "cpu={:.1}% gpu={:?} gpu_mem={:?} mem={}/{} procs={} temp={:?}",
            s.cpu_usage, s.gpu_usage, s.gpu_mem_used, s.mem_used, s.mem_total, s.process_count, s.cpu_temp_c
        );
        assert!(s.mem_total > 0 && s.mem_used <= s.mem_total);
        assert!((0.0..=100.0).contains(&s.cpu_usage));
        assert!(!s.per_core.is_empty());
        assert!(s.process_count > 10);
        assert!(!s.disks.is_empty());
        assert!(s.gpu_usage.map_or(true, |g| (0.0..=100.0).contains(&g)));
    }

    #[test]
    #[ignore = "manual: prints CPU readings over time for comparison with Task Manager"]
    fn cpu_trace() {
        let mut m = Monitor::new();
        for _ in 0..6 {
            std::thread::sleep(std::time::Duration::from_millis(1500));
            let s = m.sample();
            println!("cpu={:.1}% cores={:?}", s.cpu_usage, s.per_core.iter().map(|c| *c as u32).collect::<Vec<_>>());
        }
    }

    #[test]
    fn hardware_detection_parses() {
        let hw = crate::system::hardware::detect().expect("hardware script");
        assert!(!hw.cpu.name.is_empty());
        assert!(hw.cpu.threads >= hw.cpu.cores && hw.cpu.cores > 0);
        assert!(hw.memory.total_bytes > 0);
        assert!(!hw.disks.is_empty());
    }
}
