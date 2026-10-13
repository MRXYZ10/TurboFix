//! Scan orchestration: bounded concurrency, timeouts, cancellation, progress.

use crate::config::ScanConfig;
use crate::diagnosis::{diagnose, Diagnosis, Evidence};
use crate::error::{CoreError, Result};
use crate::network::*;
use crate::performance::PerfSnapshot;
use crate::platform::{platform_info, PlatformInfo};
use futures_util::future::join_all;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScanStatus {
    Completed,
    Cancelled,
    Failed,
}

impl ScanStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            ScanStatus::Completed => "completed",
            ScanStatus::Cancelled => "cancelled",
            ScanStatus::Failed => "failed",
        }
    }
    pub fn parse(s: &str) -> Self {
        match s {
            "completed" => ScanStatus::Completed,
            "cancelled" => ScanStatus::Cancelled,
            _ => ScanStatus::Failed,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UnavailableMetric {
    pub name: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScanReport {
    pub id: Option<i64>,
    pub app_version: String,
    pub started_at: String,
    pub duration_ms: u64,
    pub status: ScanStatus,
    pub error: Option<String>,
    pub platform: PlatformInfo,
    pub config: ScanConfig,
    pub interfaces: Vec<InterfaceInfo>,
    pub gateway: Option<GatewayStatus>,
    pub dns: Vec<DnsResult>,
    pub https: Vec<HttpsResult>,
    pub latency: Vec<LatencyResult>,
    pub diagnosis: Option<Diagnosis>,
    pub performance: Option<PerfSnapshot>,
    pub unavailable: Vec<UnavailableMetric>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScanProgress {
    pub stage: &'static str,
    pub step: u32,
    pub total_steps: u32,
}

pub fn unavailable_metrics() -> Vec<UnavailableMetric> {
    vec![
        UnavailableMetric {
            name: "Download / upload speed".into(),
            reason: "Throughput is measured separately in the Speed Test page; this diagnostic scan does not run a bandwidth test."
                .into(),
        },
        UnavailableMetric {
            name: "Wi-Fi signal and link rate".into(),
            reason: "Wi-Fi data is measured separately in Wi-Fi Analysis on Windows; it is not part of this scan report.".into(),
        },
        UnavailableMetric {
            name: "ICMP ping".into(),
            reason: "Latency uses TCP connect timing, which needs no administrator rights.".into(),
        },
    ]
}

const TOTAL_STEPS: u32 = 5;

/// Runs all diagnostics. Cancellation yields `Ok` with `status = Cancelled`
/// and whatever was measured so far, so partial data is never lost or faked.
pub async fn run_scan<F>(
    config: ScanConfig,
    cancel: CancellationToken,
    progress: F,
) -> Result<ScanReport>
where
    F: Fn(ScanProgress) + Send + Sync,
{
    config.validate()?;
    let started = Instant::now();
    let mut report = ScanReport {
        id: None,
        app_version: APP_VERSION.into(),
        started_at: chrono::Utc::now().to_rfc3339(),
        duration_ms: 0,
        status: ScanStatus::Completed,
        error: None,
        platform: platform_info(),
        config: config.clone(),
        interfaces: vec![],
        gateway: None,
        dns: vec![],
        https: vec![],
        latency: vec![],
        diagnosis: None,
        performance: None,
        unavailable: unavailable_metrics(),
    };

    let outcome = run_stages(&config, &cancel, &progress, &mut report).await;
    match outcome {
        Ok(()) => {
            report.diagnosis = Some(diagnose(&Evidence {
                interfaces: &report.interfaces,
                gateway: report
                    .gateway
                    .as_ref()
                    .unwrap_or(&GatewayStatus::Unavailable {
                        reason: "not measured".into(),
                    }),
                dns: &report.dns,
                https: &report.https,
                latency: &report.latency,
            }));
        }
        Err(CoreError::Cancelled) => report.status = ScanStatus::Cancelled,
        Err(e) => {
            report.status = ScanStatus::Failed;
            report.error = Some(e.to_string());
        }
    }
    report.duration_ms = started.elapsed().as_millis() as u64;
    if let Some(d) = &report.diagnosis {
        tracing::info!(status = ?d.status, findings = d.findings.len(), "scan finished");
    }
    Ok(report)
}

async fn run_stages<F>(
    config: &ScanConfig,
    cancel: &CancellationToken,
    progress: &F,
    report: &mut ScanReport,
) -> Result<()>
where
    F: Fn(ScanProgress) + Send + Sync,
{
    let step = |stage: &'static str, n: u32| {
        progress(ScanProgress {
            stage,
            step: n,
            total_steps: TOTAL_STEPS,
        })
    };

    step("Checking network adapters", 1);
    report.interfaces = tokio::task::spawn_blocking(list_interfaces)
        .await
        .map_err(|e| CoreError::Internal(e.to_string()))?;
    if cancel.is_cancelled() {
        return Err(CoreError::Cancelled);
    }

    step("Checking local gateway", 2);
    report.gateway = Some(
        check_gateway(
            Duration::from_millis(config.probe_timeout_ms.min(1500)),
            cancel,
        )
        .await?,
    );

    step("Timing DNS lookups", 3);
    let dns_to = Duration::from_millis(config.dns_timeout_ms);
    let results = join_all(config.dns_hosts.iter().map(|h| time_dns(h, dns_to, cancel))).await;
    report.dns = results.into_iter().collect::<Result<Vec<_>>>()?;

    step("Measuring latency, jitter and loss", 4);
    let interval = Duration::from_millis(config.probe_interval_ms);
    let probe_to = Duration::from_millis(config.probe_timeout_ms);
    let results = join_all(
        config
            .latency_targets
            .iter()
            .map(|t| measure_latency(t, config.probe_count, interval, probe_to, cancel)),
    )
    .await;
    report.latency = results.into_iter().collect::<Result<Vec<_>>>()?;

    step("Checking HTTPS reachability", 5);
    if !config.https_endpoints.is_empty() {
        let client = build_http_client(Duration::from_millis(config.https_timeout_ms))?;
        let results = join_all(
            config
                .https_endpoints
                .iter()
                .map(|u| check_https(&client, u, cancel)),
        )
        .await;
        report.https = results.into_iter().collect::<Result<Vec<_>>>()?;
    }
    Ok(())
}
