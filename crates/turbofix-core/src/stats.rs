//! Pure measurement calculations (no I/O). Definitions match docs/MEASUREMENTS.md:
//! * TCP failure % = unsuccessful TCP connections / total connection attempts x 100
//! * median: mean of the two middle values for even counts
//! * p95: nearest-rank
//! * jitter: mean absolute difference between consecutive successful probes
//! * throughput: decimal megabits per second

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LatencyStats {
    pub sent: u32,
    /// Successful TCP handshakes (legacy field name retained for API compatibility).
    pub received: u32,
    /// Deprecated legacy field. TCP connect probes do not measure IP packet loss.
    #[serde(default)]
    pub loss_pct: Option<f64>,
    #[serde(default)]
    pub connected: u32,
    #[serde(default)]
    pub refused: u32,
    #[serde(default)]
    pub timed_out: u32,
    #[serde(default)]
    pub resolution_timeouts: u32,
    #[serde(default)]
    pub connection_timeouts: u32,
    #[serde(default)]
    pub resolution_failed: u32,
    #[serde(default)]
    pub transport_errors: u32,
    #[serde(default)]
    pub tcp_failure_pct: Option<f64>,
    pub min_ms: Option<f64>,
    pub median_ms: Option<f64>,
    pub p95_ms: Option<f64>,
    pub max_ms: Option<f64>,
    pub mean_ms: Option<f64>,
    pub jitter_ms: Option<f64>,
}

fn finite_sorted(values: &[f64]) -> Option<Vec<f64>> {
    if values.is_empty() || values.iter().any(|v| !v.is_finite()) {
        return None;
    }
    let mut v = values.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
    Some(v)
}

pub fn mean(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        None
    } else {
        Some(values.iter().sum::<f64>() / values.len() as f64)
    }
}

pub fn median(values: &[f64]) -> Option<f64> {
    let v = finite_sorted(values)?;
    let n = v.len();
    Some(if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    })
}

/// Nearest-rank percentile, `p` in (0, 100].
pub fn percentile_nearest_rank(values: &[f64], p: f64) -> Option<f64> {
    if !p.is_finite() || p <= 0.0 || p > 100.0 {
        return None;
    }
    let v = finite_sorted(values)?;
    let rank = (p / 100.0 * v.len() as f64).ceil() as usize;
    Some(v[rank.clamp(1, v.len()) - 1])
}

pub fn jitter_ms(samples_in_order: &[f64]) -> Option<f64> {
    if samples_in_order.len() < 2 {
        return None;
    }
    let sum: f64 = samples_in_order
        .windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .sum();
    Some(sum / (samples_in_order.len() - 1) as f64)
}

pub fn tcp_failure_pct(sent: u32, connected: u32) -> f64 {
    if sent == 0 {
        return 0.0;
    }
    sent.saturating_sub(connected) as f64 / sent as f64 * 100.0
}

/// Compatibility summarizer for the original success-or-failed sample format.
pub fn summarize(samples: &[Option<f64>]) -> LatencyStats {
    let ok: Vec<f64> = samples.iter().flatten().copied().collect();
    let sent = samples.len() as u32;
    let received = ok.len() as u32;
    LatencyStats {
        sent,
        received,
        loss_pct: None,
        connected: received,
        refused: 0,
        timed_out: sent.saturating_sub(received),
        resolution_timeouts: 0,
        connection_timeouts: sent.saturating_sub(received),
        resolution_failed: 0,
        transport_errors: 0,
        tcp_failure_pct: (sent > 0).then(|| tcp_failure_pct(sent, received)),
        min_ms: ok.iter().copied().reduce(f64::min),
        median_ms: median(&ok),
        p95_ms: percentile_nearest_rank(&ok, 95.0),
        max_ms: ok.iter().copied().reduce(f64::max),
        mean_ms: mean(&ok),
        jitter_ms: jitter_ms(&ok),
    }
}

/// Summarize distinct TCP connection outcomes. Only completed handshakes
/// contribute latency; failures remain categorized and are never called packet loss.
pub fn summarize_outcomes(outcomes: &[crate::network::ProbeOutcome]) -> LatencyStats {
    use crate::network::ProbeOutcome;
    let mut stats = summarize(&[]);
    let mut values = Vec::new();
    for outcome in outcomes {
        match outcome {
            ProbeOutcome::Connected { latency_ms } => {
                stats.connected += 1;
                values.push(*latency_ms);
            }
            ProbeOutcome::ConnectionRefused { .. } => stats.refused += 1,
            ProbeOutcome::Timeout { .. } => stats.timed_out += 1,
            ProbeOutcome::ResolutionFailed { .. } => stats.resolution_failed += 1,
            ProbeOutcome::TransportError { .. } => stats.transport_errors += 1,
        }
    }
    stats.resolution_timeouts = outcomes
        .iter()
        .filter(|o| {
            matches!(
                o,
                ProbeOutcome::Timeout {
                    stage: crate::network::ProbeStage::Resolution
                }
            )
        })
        .count() as u32;
    stats.connection_timeouts = stats.timed_out - stats.resolution_timeouts;
    stats.sent = outcomes.len() as u32;
    stats.received = stats.connected;
    stats.tcp_failure_pct = (stats.sent > 0).then(|| tcp_failure_pct(stats.sent, stats.connected));
    stats.min_ms = values.iter().copied().reduce(f64::min);
    stats.median_ms = median(&values);
    stats.p95_ms = percentile_nearest_rank(&values, 95.0);
    stats.max_ms = values.iter().copied().reduce(f64::max);
    stats.mean_ms = mean(&values);
    stats.jitter_ms = jitter_ms(&values);
    stats
}

/// Decimal megabits per second; `None` for a zero/invalid duration.
pub fn throughput_mbps(bytes: u64, millis: f64) -> Option<f64> {
    if !millis.is_finite() || millis <= 0.0 {
        return None;
    }
    Some(bytes as f64 * 8.0 / (millis / 1000.0) / 1_000_000.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "{a} != {b}");
    }

    #[test]
    fn median_odd_even_and_edges() {
        approx(median(&[30.0, 10.0, 20.0]).unwrap(), 20.0);
        approx(median(&[10.0, 20.0, 30.0, 40.0]).unwrap(), 25.0);
        approx(median(&[5.0]).unwrap(), 5.0);
        assert!(median(&[]).is_none());
        assert!(median(&[1.0, f64::NAN]).is_none());
    }

    #[test]
    fn p95_is_nearest_rank() {
        let v: Vec<f64> = (1..=10).map(|x| x as f64).collect();
        approx(percentile_nearest_rank(&v, 95.0).unwrap(), 10.0); // ceil(9.5)=10th
        approx(percentile_nearest_rank(&v, 50.0).unwrap(), 5.0);
        approx(percentile_nearest_rank(&v, 100.0).unwrap(), 10.0);
        approx(percentile_nearest_rank(&[7.0], 95.0).unwrap(), 7.0);
        assert!(percentile_nearest_rank(&v, 0.0).is_none());
        assert!(percentile_nearest_rank(&v, 101.0).is_none());
        assert!(percentile_nearest_rank(&[], 95.0).is_none());
    }

    #[test]
    fn jitter_is_mean_abs_consecutive_difference() {
        approx(jitter_ms(&[10.0, 12.0, 9.0, 9.0]).unwrap(), 5.0 / 3.0);
        approx(jitter_ms(&[7.0, 7.0, 7.0]).unwrap(), 0.0);
        assert!(jitter_ms(&[10.0]).is_none());
        assert!(jitter_ms(&[]).is_none());
    }

    #[test]
    fn tcp_failure_percentage_cases() {
        approx(tcp_failure_pct(10, 10), 0.0);
        approx(tcp_failure_pct(10, 7), 30.0);
        approx(tcp_failure_pct(4, 0), 100.0);
        approx(tcp_failure_pct(0, 0), 0.0);
        approx(tcp_failure_pct(3, 9), 0.0);
    }

    #[test]
    fn compatibility_summary_never_populates_packet_loss() {
        let s = summarize(&[Some(10.0), None, Some(20.0), None, Some(30.0)]);
        assert_eq!((s.sent, s.received), (5, 3));
        approx(s.tcp_failure_pct.unwrap(), 40.0);
        assert_eq!(s.loss_pct, None);
        approx(s.min_ms.unwrap(), 10.0);
        approx(s.max_ms.unwrap(), 30.0);
        approx(s.median_ms.unwrap(), 20.0);
        approx(s.mean_ms.unwrap(), 20.0);
        approx(s.jitter_ms.unwrap(), 10.0);
    }

    #[test]
    fn outcome_summary_keeps_connection_failure_categories_separate() {
        use crate::network::{ProbeOutcome as O, ProbeStage as S};
        let s = summarize_outcomes(&[
            O::Connected { latency_ms: 5.0 },
            O::ConnectionRefused { latency_ms: 1.0 },
            O::Timeout {
                stage: S::Resolution,
            },
            O::Timeout {
                stage: S::Connection,
            },
            O::ResolutionFailed {
                message: "NXDOMAIN".into(),
            },
            O::TransportError {
                message: "reset".into(),
            },
        ]);
        assert_eq!(s.connected, 1);
        assert_eq!(s.refused, 1);
        assert_eq!(s.resolution_timeouts, 1);
        assert_eq!(s.connection_timeouts, 1);
        assert_eq!(s.resolution_failed, 1);
        assert_eq!(s.transport_errors, 1);
        approx(s.tcp_failure_pct.unwrap(), 100.0 * 5.0 / 6.0);
        assert_eq!(s.loss_pct, None);
        assert_eq!(s.median_ms, Some(5.0));
    }

    #[test]
    fn all_failed_never_invents_numbers() {
        let s = summarize(&[None, None, None]);
        assert_eq!(s.received, 0);
        approx(s.tcp_failure_pct.unwrap(), 100.0);
        assert_eq!(s.loss_pct, None);
        assert!(
            s.median_ms.is_none()
                && s.mean_ms.is_none()
                && s.p95_ms.is_none()
                && s.jitter_ms.is_none()
        );
        let e = summarize(&[]);
        assert_eq!(e.sent, 0);
        assert!(e.median_ms.is_none());
    }

    #[test]
    fn throughput_uses_decimal_megabits() {
        approx(throughput_mbps(12_500_000, 1000.0).unwrap(), 100.0);
        approx(throughput_mbps(1_000_000, 500.0).unwrap(), 16.0);
        approx(throughput_mbps(0, 1000.0).unwrap(), 0.0);
        assert!(throughput_mbps(1, 0.0).is_none());
        assert!(throughput_mbps(1, f64::NAN).is_none());
    }
}
