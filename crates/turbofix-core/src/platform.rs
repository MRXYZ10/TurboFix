//! Platform facts and an honest feature inventory. Anything Windows-specific
//! that has no verified implementation is listed as planned/disabled here;
//! nothing is simulated on other operating systems.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlatformInfo {
    pub os: String,
    pub arch: String,
    pub is_windows: bool,
}

pub fn platform_info() -> PlatformInfo {
    PlatformInfo {
        os: std::env::consts::OS.into(),
        arch: std::env::consts::ARCH.into(),
        is_windows: cfg!(windows),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityStatus {
    Available,
    Planned,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Capability {
    pub id: String,
    pub name: String,
    pub status: CapabilityStatus,
    pub note: String,
}

fn cap(id: &str, name: &str, status: CapabilityStatus, note: &str) -> Capability {
    Capability {
        id: id.into(),
        name: name.into(),
        status,
        note: note.into(),
    }
}

pub fn capabilities() -> Vec<Capability> {
    use CapabilityStatus::*;
    let wifi_status = if cfg!(windows) { Available } else { Planned };
    let windows_fix_status = if cfg!(windows) { Available } else { Planned };
    vec![
        cap("connectivity", "Connectivity diagnostics", Available, "Adapters, gateway (TCP probe), DNS timing, TCP latency/jitter/loss, HTTPS reachability, with evidence-based findings."),
        cap("pc_monitor", "CPU, memory, disk space and process monitoring", Available, "Read-only, via the sysinfo crate. Disk I/O activity is not measured."),
        cap("history", "Scan and speed-test history", Available, "History is stored locally in SQLite; report export is user-initiated. The separate speed test sends synthetic bytes to Cloudflare only when started."),
        cap("speed_test", "Download / upload speed test", Available, "Explicitly runs a bounded test against Cloudflare's public speed-test edge; a small amount of synthetic data is transferred and result saved locally."),
        cap("wifi", "Wi-Fi signal, channel, link rate and nearby networks", wifi_status, "Read-only Windows netsh Wi-Fi diagnostics. Signal and link rate are not the same as internet throughput."),
        cap("dns_flush", "Flush DNS cache", if cfg!(windows) { Available } else { Planned }, "Runs the fixed Windows ipconfig /flushdns command after explicit user action; no adapter settings are changed."),
        cap("startup", "Startup entry review", Planned, "Read-only enumeration not implemented yet."),
        cap("dns_change", "DNS server change and rollback", Disabled, "Disabled until adapter-specific apply, verification and rollback are implemented and tested on Windows."),
        cap("cleanup", "Preview-first temporary-file cleanup", if cfg!(windows) { Available } else { Planned }, "Lists up to 100 large regular files in the user's temp folder that are at least 7 days old. Only explicitly selected preview IDs can be deleted; no recursion or symlink following."),
        cap("optimizer", "Safe quick fixes", windows_fix_status, "Only the explicitly requested DNS-cache flush is implemented. Risky TCP, registry, service and adapter tweaks are not applied."),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn privileged_features_are_never_marked_available() {
        for id in ["dns_change", "startup"] {
            let c = capabilities().into_iter().find(|c| c.id == id).unwrap();
            assert_ne!(
                c.status,
                CapabilityStatus::Available,
                "{id} must stay off until verified"
            );
        }
        assert_eq!(
            capabilities()
                .into_iter()
                .find(|c| c.id == "speed_test")
                .unwrap()
                .status,
            CapabilityStatus::Available
        );
        assert_eq!(
            capabilities()
                .into_iter()
                .find(|c| c.id == "cleanup")
                .unwrap()
                .status,
            if cfg!(windows) {
                CapabilityStatus::Available
            } else {
                CapabilityStatus::Planned
            }
        );
        assert_eq!(
            capabilities()
                .into_iter()
                .find(|c| c.id == "optimizer")
                .unwrap()
                .status,
            if cfg!(windows) {
                CapabilityStatus::Available
            } else {
                CapabilityStatus::Planned
            }
        );
        assert_eq!(
            capabilities()
                .into_iter()
                .find(|c| c.id == "wifi")
                .unwrap()
                .status,
            if cfg!(windows) {
                CapabilityStatus::Available
            } else {
                CapabilityStatus::Planned
            }
        );
    }

    #[test]
    fn ids_unique_and_platform_consistent() {
        let caps = capabilities();
        let mut ids: Vec<_> = caps.iter().map(|c| c.id.clone()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), caps.len());
        assert_eq!(platform_info().is_windows, cfg!(windows));
    }
}
