//! Real system monitoring via `sysinfo` (works on Windows and Linux).
//!
//! Notes surfaced in the UI/docs:
//! * Process CPU % is relative to one core (can exceed 100 on multi-core).
//! * Disk *activity* (throughput/queue length) is not measured in the MVP;
//!   only capacity is reported.

use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use sysinfo::{Disks, Networks, ProcessesToUpdate, System, MINIMUM_CPU_UPDATE_INTERVAL};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProcInfo {
    pub pid: u32,
    pub name: String,
    pub cpu_pct: f32,
    pub memory_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiskInfo {
    pub name: String,
    pub mount: String,
    pub file_system: String,
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub removable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PerfSnapshot {
    pub timestamp: String,
    pub os_name: Option<String>,
    pub os_version: Option<String>,
    pub cpu_count: usize,
    /// Overall CPU utilisation 0-100 since the previous sample.
    pub cpu_pct: f32,
    pub memory_total_bytes: u64,
    pub memory_used_bytes: u64,
    pub memory_available_bytes: u64,
    pub swap_total_bytes: u64,
    pub swap_used_bytes: u64,
    pub disks: Vec<DiskInfo>,
    pub process_count: usize,
    pub top_cpu: Vec<ProcInfo>,
    pub top_memory: Vec<ProcInfo>,
    /// Receive/transmit rate over all interfaces since previous sample; `None` on first sample.
    pub net_rx_bytes_per_s: Option<f64>,
    pub net_tx_bytes_per_s: Option<f64>,
}

pub struct PerfMonitor {
    sys: System,
    disks: Disks,
    networks: Networks,
    last_net: Option<(Instant, u64, u64)>,
    last_cpu_refresh: Instant,
}

const TOP_N: usize = 8;

impl Default for PerfMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl PerfMonitor {
    pub fn new() -> Self {
        let mut sys = System::new();
        sys.refresh_cpu_usage();
        sys.refresh_memory();
        sys.refresh_processes(ProcessesToUpdate::All, true);
        Self {
            sys,
            disks: Disks::new_with_refreshed_list(),
            networks: Networks::new_with_refreshed_list(),
            last_net: None,
            last_cpu_refresh: Instant::now(),
        }
    }

    /// Blocking; call from `spawn_blocking`. Sleeps at most ~200 ms on the
    /// first call so CPU figures are valid.
    pub fn sample(&mut self) -> PerfSnapshot {
        let since = self.last_cpu_refresh.elapsed();
        if since < MINIMUM_CPU_UPDATE_INTERVAL {
            std::thread::sleep(MINIMUM_CPU_UPDATE_INTERVAL - since);
        }
        self.sys.refresh_cpu_usage();
        self.last_cpu_refresh = Instant::now();
        self.sys.refresh_memory();
        self.sys.refresh_processes(ProcessesToUpdate::All, true);
        self.disks.refresh(true);
        self.networks.refresh(true);

        let (rx, tx) = self.networks.iter().fold((0u64, 0u64), |(r, t), (_, d)| {
            (r + d.total_received(), t + d.total_transmitted())
        });
        let now = Instant::now();
        let (rx_rate, tx_rate) = match self.last_net {
            Some((t0, r0, x0)) => {
                let dt = now.duration_since(t0).as_secs_f64();
                if dt > 0.0 {
                    (
                        Some(rx.saturating_sub(r0) as f64 / dt),
                        Some(tx.saturating_sub(x0) as f64 / dt),
                    )
                } else {
                    (None, None)
                }
            }
            None => (None, None),
        };
        self.last_net = Some((now, rx, tx));

        let procs: Vec<ProcInfo> = self
            .sys
            .processes()
            .iter()
            .map(|(pid, p)| ProcInfo {
                pid: pid.as_u32(),
                name: p.name().to_string_lossy().into_owned(),
                cpu_pct: p.cpu_usage(),
                memory_bytes: p.memory(),
            })
            .collect();
        let mut by_cpu = procs.clone();
        by_cpu.sort_by(|a, b| b.cpu_pct.total_cmp(&a.cpu_pct));
        by_cpu.truncate(TOP_N);
        let mut by_mem = procs.clone();
        by_mem.sort_by_key(|a| std::cmp::Reverse(a.memory_bytes));
        by_mem.truncate(TOP_N);

        PerfSnapshot {
            timestamp: chrono::Utc::now().to_rfc3339(),
            os_name: System::name(),
            os_version: System::os_version(),
            cpu_count: self.sys.cpus().len(),
            cpu_pct: self.sys.global_cpu_usage(),
            memory_total_bytes: self.sys.total_memory(),
            memory_used_bytes: self.sys.used_memory(),
            memory_available_bytes: self.sys.available_memory(),
            swap_total_bytes: self.sys.total_swap(),
            swap_used_bytes: self.sys.used_swap(),
            disks: self
                .disks
                .list()
                .iter()
                .map(|d| DiskInfo {
                    name: d.name().to_string_lossy().into_owned(),
                    mount: d.mount_point().to_string_lossy().into_owned(),
                    file_system: d.file_system().to_string_lossy().into_owned(),
                    total_bytes: d.total_space(),
                    available_bytes: d.available_space(),
                    removable: d.is_removable(),
                })
                .filter(|d| d.total_bytes > 0)
                .collect(),
            process_count: procs.len(),
            top_cpu: by_cpu,
            top_memory: by_mem,
            net_rx_bytes_per_s: rx_rate,
            net_tx_bytes_per_s: tx_rate,
        }
    }
}

/// Minimum between-sample gap callers should respect.
pub const MIN_SAMPLE_INTERVAL: Duration = MINIMUM_CPU_UPDATE_INTERVAL;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_reports_real_values() {
        let mut m = PerfMonitor::new();
        let s = m.sample();
        assert!(s.memory_total_bytes > 0);
        assert!(s.memory_used_bytes <= s.memory_total_bytes);
        assert!(s.cpu_count >= 1);
        assert!((0.0..=100.0).contains(&s.cpu_pct), "cpu {}", s.cpu_pct);
        assert!(s.process_count >= 1);
        assert!(s.top_memory.len() <= TOP_N);
        // first sample has no rate yet: unavailable, not zero
        assert_eq!(s.net_rx_bytes_per_s, None);
        let s2 = m.sample();
        assert!(s2.net_rx_bytes_per_s.is_some());
    }

    #[test]
    fn top_lists_are_sorted() {
        let mut m = PerfMonitor::new();
        let s = m.sample();
        assert!(s
            .top_memory
            .windows(2)
            .all(|w| w[0].memory_bytes >= w[1].memory_bytes));
        assert!(s.top_cpu.windows(2).all(|w| w[0].cpu_pct >= w[1].cpu_pct));
    }
}
