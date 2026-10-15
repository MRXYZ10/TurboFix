//! Continuous, bounded TCP-connect monitoring sessions.
//!
//! The scheduler waits for each bounded probe round to finish before beginning
//! the configured interval delay, so rounds cannot overlap. Only summaries are
//! intended for persistence; per-probe history remains a short in-memory ring.

use crate::config::validate_tcp_target;
use crate::error::{CoreError, Result};
use crate::network::{tcp_connect, ProbeOutcome, ProbeStage};
use futures_util::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

pub const MAX_TARGETS: usize = 8;
pub const MAX_CONCURRENCY: u8 = 4;
pub const RECENT_RESULT_LIMIT: usize = 30;
pub const LATENCY_HISTORY_LIMIT: usize = 60;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MonitorConfig {
    pub targets: Vec<String>,
    pub interval_ms: u64,
    pub timeout_ms: u64,
    pub max_concurrency: u8,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            targets: vec!["1.1.1.1:443".into(), "8.8.8.8:443".into()],
            interval_ms: 1_000,
            timeout_ms: 2_000,
            max_concurrency: 2,
        }
    }
}

impl MonitorConfig {
    pub fn validate(&self) -> Result<()> {
        if self.targets.is_empty() || self.targets.len() > MAX_TARGETS {
            return Err(CoreError::InvalidConfig(format!(
                "monitor target count must be between 1 and {MAX_TARGETS}"
            )));
        }
        if !(500..=60_000).contains(&self.interval_ms) {
            return Err(CoreError::InvalidConfig(
                "monitor interval must be between 500 and 60000 ms".into(),
            ));
        }
        if !(200..=10_000).contains(&self.timeout_ms) {
            return Err(CoreError::InvalidConfig(
                "monitor timeout must be between 200 and 10000 ms".into(),
            ));
        }
        if !(1..=MAX_CONCURRENCY).contains(&self.max_concurrency) {
            return Err(CoreError::InvalidConfig(format!(
                "maximum concurrency must be between 1 and {MAX_CONCURRENCY}"
            )));
        }
        let mut unique = std::collections::HashSet::new();
        for target in &self.targets {
            if target.len() > 255 {
                return Err(CoreError::InvalidConfig(
                    "monitor targets must be at most 255 characters".into(),
                ));
            }
            validate_tcp_target(target)?;
            if !unique.insert(target.to_ascii_lowercase()) {
                return Err(CoreError::InvalidConfig(format!(
                    "duplicate monitor target '{target}'"
                )));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LatencyPoint {
    pub at: String,
    pub latency_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MonitorProbe {
    pub at: String,
    pub outcome: ProbeOutcome,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TargetMonitorSnapshot {
    pub target: String,
    pub attempts: u64,
    pub successful_attempts: u64,
    pub failed_attempts: u64,
    pub connection_refusals: u64,
    pub resolution_failures: u64,
    pub resolution_timeouts: u64,
    pub connection_timeouts: u64,
    pub transport_errors: u64,
    pub latest_success_latency_ms: Option<f64>,
    pub latest_success_at: Option<String>,
    pub jitter_ms: Option<f64>,
    pub last_result: Option<MonitorProbe>,
    pub recent_results: Vec<MonitorProbe>,
    pub latency_history: Vec<LatencyPoint>,
    #[serde(skip)]
    jitter_sum_ms: f64,
    #[serde(skip)]
    jitter_pairs: u64,
    #[serde(skip)]
    previous_success_ms: Option<f64>,
}

impl TargetMonitorSnapshot {
    pub fn empty(target: String) -> Self {
        Self {
            target,
            attempts: 0,
            successful_attempts: 0,
            failed_attempts: 0,
            connection_refusals: 0,
            resolution_failures: 0,
            resolution_timeouts: 0,
            connection_timeouts: 0,
            transport_errors: 0,
            latest_success_latency_ms: None,
            latest_success_at: None,
            jitter_ms: None,
            last_result: None,
            recent_results: Vec::new(),
            latency_history: Vec::new(),
            jitter_sum_ms: 0.0,
            jitter_pairs: 0,
            previous_success_ms: None,
        }
    }

    pub fn record(&mut self, at: String, outcome: ProbeOutcome) {
        self.attempts += 1;
        match &outcome {
            ProbeOutcome::Connected { latency_ms } => {
                self.successful_attempts += 1;
                if let Some(previous) = self.previous_success_ms {
                    self.jitter_sum_ms += (latency_ms - previous).abs();
                    self.jitter_pairs += 1;
                    self.jitter_ms = Some(self.jitter_sum_ms / self.jitter_pairs as f64);
                }
                self.previous_success_ms = Some(*latency_ms);
                self.latest_success_latency_ms = Some(*latency_ms);
                self.latest_success_at = Some(at.clone());
                self.latency_history.push(LatencyPoint {
                    at: at.clone(),
                    latency_ms: *latency_ms,
                });
                trim_front(&mut self.latency_history, LATENCY_HISTORY_LIMIT);
            }
            ProbeOutcome::ConnectionRefused { .. } => {
                self.failed_attempts += 1;
                self.connection_refusals += 1;
            }
            ProbeOutcome::Timeout {
                stage: ProbeStage::Resolution,
            } => {
                self.failed_attempts += 1;
                self.resolution_timeouts += 1;
            }
            ProbeOutcome::Timeout {
                stage: ProbeStage::Connection,
            } => {
                self.failed_attempts += 1;
                self.connection_timeouts += 1;
            }
            ProbeOutcome::ResolutionFailed { .. } => {
                self.failed_attempts += 1;
                self.resolution_failures += 1;
            }
            ProbeOutcome::TransportError { .. } => {
                self.failed_attempts += 1;
                self.transport_errors += 1;
            }
        }
        let probe = MonitorProbe { at, outcome };
        self.last_result = Some(probe.clone());
        self.recent_results.push(probe);
        trim_front(&mut self.recent_results, RECENT_RESULT_LIMIT);
    }
}

fn trim_front<T>(items: &mut Vec<T>, limit: usize) {
    if items.len() > limit {
        let excess = items.len() - limit;
        items.drain(0..excess);
    }
}

fn duration_ms_ceil(duration: Duration) -> u64 {
    let fractional_ms = if duration.subsec_nanos() % 1_000_000 == 0 {
        0
    } else {
        1
    };
    duration
        .as_millis()
        .saturating_add(fractional_ms)
        .min(u64::MAX as u128) as u64
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MonitorSnapshot {
    pub id: Option<i64>,
    pub config: MonitorConfig,
    pub started_at: String,
    pub elapsed_ms: u64,
    pub running: bool,
    pub total_attempts: u64,
    pub successful_attempts: u64,
    pub failed_attempts: u64,
    pub targets: Vec<TargetMonitorSnapshot>,
    pub last_updated_at: String,
}

impl MonitorSnapshot {
    pub fn new(config: MonitorConfig) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            id: None,
            targets: config
                .targets
                .iter()
                .cloned()
                .map(TargetMonitorSnapshot::empty)
                .collect(),
            config,
            started_at: now.clone(),
            elapsed_ms: 0,
            running: true,
            total_attempts: 0,
            successful_attempts: 0,
            failed_attempts: 0,
            last_updated_at: now,
        }
    }

    fn refresh_totals(&mut self) {
        self.total_attempts = self.targets.iter().map(|t| t.attempts).sum();
        self.successful_attempts = self.targets.iter().map(|t| t.successful_attempts).sum();
        self.failed_attempts = self.targets.iter().map(|t| t.failed_attempts).sum();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TargetMonitorSummary {
    pub target: String,
    pub attempts: u64,
    pub successful_attempts: u64,
    pub failed_attempts: u64,
    pub connection_refusals: u64,
    pub resolution_failures: u64,
    pub resolution_timeouts: u64,
    pub connection_timeouts: u64,
    pub transport_errors: u64,
    pub latest_success_latency_ms: Option<f64>,
    pub jitter_ms: Option<f64>,
    pub last_result: Option<MonitorProbe>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MonitorSessionSummary {
    pub id: Option<i64>,
    pub started_at: String,
    pub ended_at: String,
    pub duration_ms: u64,
    pub interval_ms: u64,
    pub timeout_ms: u64,
    pub total_attempts: u64,
    pub successful_attempts: u64,
    pub failed_attempts: u64,
    pub targets: Vec<TargetMonitorSummary>,
}

impl MonitorSessionSummary {
    fn from_snapshot(snapshot: &MonitorSnapshot, ended_at: String) -> Self {
        Self {
            id: None,
            started_at: snapshot.started_at.clone(),
            ended_at,
            duration_ms: snapshot.elapsed_ms,
            interval_ms: snapshot.config.interval_ms,
            timeout_ms: snapshot.config.timeout_ms,
            total_attempts: snapshot.total_attempts,
            successful_attempts: snapshot.successful_attempts,
            failed_attempts: snapshot.failed_attempts,
            targets: snapshot
                .targets
                .iter()
                .map(|t| TargetMonitorSummary {
                    target: t.target.clone(),
                    attempts: t.attempts,
                    successful_attempts: t.successful_attempts,
                    failed_attempts: t.failed_attempts,
                    connection_refusals: t.connection_refusals,
                    resolution_failures: t.resolution_failures,
                    resolution_timeouts: t.resolution_timeouts,
                    connection_timeouts: t.connection_timeouts,
                    transport_errors: t.transport_errors,
                    latest_success_latency_ms: t.latest_success_latency_ms,
                    jitter_ms: t.jitter_ms,
                    last_result: t.last_result.clone(),
                })
                .collect(),
        }
    }
}

pub async fn run_monitor_session<F>(
    config: MonitorConfig,
    cancel: CancellationToken,
    on_update: F,
) -> Result<MonitorSessionSummary>
where
    F: Fn(&MonitorSnapshot) + Send + Sync,
{
    run_monitor_with(config, cancel, on_update, |target, timeout, token| {
        Box::pin(async move { tcp_connect(&target, timeout, &token).await })
    })
    .await
}

async fn run_monitor_with<F, Fut>(
    config: MonitorConfig,
    cancel: CancellationToken,
    on_update: impl Fn(&MonitorSnapshot) + Send + Sync,
    probe: F,
) -> Result<MonitorSessionSummary>
where
    F: Fn(String, Duration, CancellationToken) -> Fut + Clone + Send + Sync,
    Fut: Future<Output = Result<ProbeOutcome>> + Send,
{
    config.validate()?;
    let started = Instant::now();
    let timeout = Duration::from_millis(config.timeout_ms);
    let mut snapshot = MonitorSnapshot::new(config.clone());
    on_update(&snapshot);

    while !cancel.is_cancelled() {
        let targets = config.targets.clone();
        let round_cancel = cancel.clone();
        let probe_fn = probe.clone();
        let outcomes = tokio::select! {
            biased;
            _ = cancel.cancelled() => break,
            values = stream::iter(targets.into_iter().map(|target| {
                let token = round_cancel.clone();
                let f = probe_fn.clone();
                async move { (target.clone(), f(target, timeout, token).await) }
            })).buffer_unordered(config.max_concurrency as usize).collect::<Vec<_>>() => values,
        };

        for (target, result) in outcomes {
            let outcome = result.unwrap_or_else(|error| ProbeOutcome::TransportError {
                message: error.to_string(),
            });
            if let Some(item) = snapshot.targets.iter_mut().find(|t| t.target == target) {
                item.record(chrono::Utc::now().to_rfc3339(), outcome);
            }
        }
        snapshot.elapsed_ms = duration_ms_ceil(started.elapsed());
        snapshot.last_updated_at = chrono::Utc::now().to_rfc3339();
        snapshot.refresh_totals();
        on_update(&snapshot);

        tokio::select! {
            biased;
            _ = cancel.cancelled() => break,
            _ = tokio::time::sleep(Duration::from_millis(config.interval_ms)) => {}
        }
    }

    snapshot.elapsed_ms = duration_ms_ceil(started.elapsed());
    snapshot.running = false;
    snapshot.last_updated_at = chrono::Utc::now().to_rfc3339();
    snapshot.refresh_totals();
    on_update(&snapshot);
    Ok(MonitorSessionSummary::from_snapshot(
        &snapshot,
        chrono::Utc::now().to_rfc3339(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::ProbeStage;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    fn config() -> MonitorConfig {
        MonitorConfig {
            targets: vec!["127.0.0.1:80".into()],
            interval_ms: 500,
            timeout_ms: 1000,
            max_concurrency: 1,
        }
    }

    #[test]
    fn elapsed_duration_rounds_up_without_erasing_submillisecond_sessions() {
        assert_eq!(duration_ms_ceil(Duration::from_nanos(1)), 1);
        assert_eq!(duration_ms_ceil(Duration::from_micros(999)), 1);
        assert_eq!(duration_ms_ceil(Duration::from_millis(1)), 1);
        assert_eq!(duration_ms_ceil(Duration::from_micros(1_001)), 2);
    }

    #[test]
    fn validates_targets_limits_and_duplicate_entries() {
        let mut c = config();
        c.validate().unwrap();
        c.targets.push("bad target:3".into());
        assert!(c.validate().is_err());
        c.targets = vec!["127.0.0.1:80".into(), "127.0.0.1:80".into()];
        assert!(c.validate().is_err());
        c.targets = (0..=MAX_TARGETS)
            .map(|n| format!("127.0.0.1:{}", 80 + n))
            .collect();
        assert!(c.validate().is_err());
        c = config();
        c.interval_ms = 100;
        assert!(c.validate().is_err());
        c.interval_ms = 500;
        c.timeout_ms = 10_001;
        assert!(c.validate().is_err());
        c.timeout_ms = 1000;
        c.max_concurrency = MAX_CONCURRENCY + 1;
        assert!(c.validate().is_err());
    }

    #[test]
    fn aggregates_outcomes_and_jitter_from_successes_only() {
        let mut target = TargetMonitorSnapshot::empty("localhost:80".into());
        target.record("1".into(), ProbeOutcome::Connected { latency_ms: 20.0 });
        target.record(
            "2".into(),
            ProbeOutcome::Timeout {
                stage: ProbeStage::Connection,
            },
        );
        target.record("3".into(), ProbeOutcome::Connected { latency_ms: 24.0 });
        target.record(
            "4".into(),
            ProbeOutcome::ConnectionRefused { latency_ms: 1.0 },
        );
        target.record("5".into(), ProbeOutcome::Connected { latency_ms: 30.0 });
        target.record(
            "6".into(),
            ProbeOutcome::ResolutionFailed {
                message: "resolver failure".into(),
            },
        );
        target.record(
            "7".into(),
            ProbeOutcome::Timeout {
                stage: ProbeStage::Resolution,
            },
        );
        target.record(
            "8".into(),
            ProbeOutcome::TransportError {
                message: "route unavailable".into(),
            },
        );
        assert_eq!(target.attempts, 8);
        assert_eq!(target.successful_attempts, 3);
        assert_eq!(target.failed_attempts, 5);
        assert_eq!(target.connection_refusals, 1);
        assert_eq!(target.connection_timeouts, 1);
        assert_eq!(target.resolution_failures, 1);
        assert_eq!(target.resolution_timeouts, 1);
        assert_eq!(target.transport_errors, 1);
        assert_eq!(target.jitter_ms, Some(5.0));
        assert_eq!(target.latency_history.len(), 3);
    }

    #[tokio::test]
    async fn scheduler_honors_concurrency_limit_and_cancellation() {
        let mut c = config();
        c.targets = (0..6).map(|n| format!("127.0.0.1:{}", 80 + n)).collect();
        c.max_concurrency = 2;
        c.interval_ms = 60_000;
        let cancel = CancellationToken::new();
        let cancel_after_round = cancel.clone();
        let active = Arc::new(AtomicUsize::new(0));
        let maximum = Arc::new(AtomicUsize::new(0));
        let updates = Arc::new(AtomicUsize::new(0));
        let active_probe = active.clone();
        let max_probe = maximum.clone();
        let update_count = updates.clone();
        let summary = run_monitor_with(
            c,
            cancel,
            move |snapshot| {
                if snapshot.total_attempts >= 6 {
                    cancel_after_round.cancel();
                }
            },
            move |_target, _timeout, _token| {
                let active = active_probe.clone();
                let maximum = max_probe.clone();
                let update_count = update_count.clone();
                Box::pin(async move {
                    let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                    maximum.fetch_max(now, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(2)).await;
                    active.fetch_sub(1, Ordering::SeqCst);
                    update_count.fetch_add(1, Ordering::SeqCst);
                    Ok(ProbeOutcome::Connected { latency_ms: 10.0 })
                })
            },
        )
        .await
        .unwrap();
        assert_eq!(maximum.load(Ordering::SeqCst), 2);
        assert_eq!(summary.total_attempts, 6);
        assert_eq!(summary.successful_attempts, 6);
    }

    #[tokio::test]
    async fn cancellation_drops_inflight_probes_without_waiting_for_timeout() {
        let c = config();
        let cancel = CancellationToken::new();
        let cancel_run = cancel.clone();
        let task = tokio::spawn(async move {
            run_monitor_with(
                c,
                cancel_run,
                |_| {},
                |_target, _timeout, _token| {
                    Box::pin(std::future::pending::<Result<ProbeOutcome>>())
                },
            )
            .await
        });
        tokio::time::sleep(Duration::from_millis(10)).await;
        cancel.cancel();
        let summary = tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .expect("cancellation should promptly drop pending probes")
            .unwrap()
            .unwrap();
        assert_eq!(summary.total_attempts, 0);
    }
}
