//! Evidence-based optimizer recommendations and verification comparisons.
use crate::diagnosis::Confidence;
use crate::scan::{ScanReport, ScanStatus};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OptimizerAction {
    FlushDnsCache,
}

impl OptimizerAction {
    pub fn validate(value: &str) -> Option<Self> {
        match value {
            "flush_dns_cache" => Some(Self::FlushDnsCache),
            _ => None,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FlushDnsCache => "flush_dns_cache",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Recommendation {
    pub action: OptimizerAction,
    pub title: String,
    pub evidence: Vec<String>,
    pub expected_effect: String,
    pub risks: String,
    pub prerequisites: String,
    pub verified: bool,
    pub hypothesis: bool,
}

pub fn recommendations(scan: &ScanReport) -> Vec<Recommendation> {
    if scan.status != ScanStatus::Completed || !scan.platform.is_windows {
        return vec![];
    }
    let Some(diagnosis) = scan.diagnosis.as_ref() else {
        return vec![];
    };
    let dns_finding = diagnosis
        .findings
        .iter()
        .find(|f| matches!(f.id.as_str(), "dns_failure" | "dns_partial"));
    dns_finding.map(|finding| vec![Recommendation {
        action: OptimizerAction::FlushDnsCache,
        title: "Flush Windows DNS resolver cache".into(),
        evidence: finding.evidence.clone(),
        expected_effect: "May remove a stale cached DNS answer. It cannot repair an unreachable or misconfigured DNS server; no speed improvement is guaranteed.".into(),
        risks: "Low impact; cached names may need to be resolved again, briefly adding lookup time.".into(),
        prerequisites: "Windows 10/11 and a completed diagnostic scan showing DNS lookup failures.".into(),
        verified: finding.confidence == Confidence::High,
        hypothesis: true,
    }]).unwrap_or_default()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MeasurementComparison {
    pub metric: String,
    pub before: Option<f64>,
    pub after: Option<f64>,
    pub delta: Option<f64>,
    pub result: String,
}

pub fn compare(before: &ScanReport, after: &ScanReport) -> Vec<MeasurementComparison> {
    let mut comparisons = Vec::new();
    for (prefix, keys, before_values, after_values) in [
        (
            "dns",
            before
                .config
                .dns_hosts
                .iter()
                .chain(&after.config.dns_hosts)
                .cloned()
                .collect::<BTreeSet<_>>(),
            before
                .dns
                .iter()
                .map(|x| (x.host.as_str(), x.ok.then_some(x.time_ms).flatten()))
                .collect::<Vec<_>>(),
            after
                .dns
                .iter()
                .map(|x| (x.host.as_str(), x.ok.then_some(x.time_ms).flatten()))
                .collect::<Vec<_>>(),
        ),
        (
            "tcp_connect",
            before
                .config
                .latency_targets
                .iter()
                .chain(&after.config.latency_targets)
                .cloned()
                .collect::<BTreeSet<_>>(),
            before
                .latency
                .iter()
                .map(|x| (x.target.as_str(), x.stats.median_ms))
                .collect::<Vec<_>>(),
            after
                .latency
                .iter()
                .map(|x| (x.target.as_str(), x.stats.median_ms))
                .collect::<Vec<_>>(),
        ),
    ] {
        for key in keys {
            let find = |values: &[(&str, Option<f64>)]| {
                values
                    .iter()
                    .find(|(target, _)| *target == key)
                    .and_then(|(_, value)| *value)
                    .filter(|value| value.is_finite())
            };
            let b = find(&before_values);
            let a = find(&after_values);
            let metric = format!("{prefix}:{key}");
            let delta = b.zip(a).map(|(x, y)| y - x);
            let result = match delta {
                None => "measurement unavailable on one or both scans".into(),
                Some(d) if d.abs() < 1.0 => "no measurable improvement".into(),
                Some(d) if d < 0.0 => {
                    "lower after measurement; change alone does not establish cause".into()
                }
                Some(_) => "higher after measurement; change alone does not establish cause".into(),
            };
            comparisons.push(MeasurementComparison {
                metric,
                before: b,
                after: a,
                delta,
                result,
            });
        }
    }
    comparisons
}

pub fn action_supported(action: &str) -> bool {
    OptimizerAction::validate(action).is_some()
}

pub fn execution_state(succeeded: bool, verification_available: bool) -> &'static str {
    match (succeeded, verification_available) {
        (false, _) => "failed",
        (true, true) => "succeeded_and_verified",
        (true, false) => "succeeded_verification_failed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnosis::{Diagnosis, Finding, OverallStatus, Severity};
    use crate::scan::{ScanStatus, UnavailableMetric};
    use crate::{config::ScanConfig, platform::platform_info};

    fn scan(findings: Vec<Finding>, dns_ms: Option<f64>) -> ScanReport {
        let config = ScanConfig {
            dns_hosts: vec!["a.test".into()],
            ..ScanConfig::default()
        };
        ScanReport {
            id: Some(1),
            app_version: "test".into(),
            started_at: "2026-01-01T00:00:00Z".into(),
            duration_ms: 1,
            status: ScanStatus::Completed,
            error: None,
            platform: platform_info(),
            config,
            interfaces: vec![],
            gateway: None,
            dns: dns_ms
                .map(|ms| {
                    vec![crate::network::DnsResult {
                        host: "a.test".into(),
                        ok: true,
                        time_ms: Some(ms),
                        address_count: 1,
                        timed_out: false,
                        error: None,
                    }]
                })
                .unwrap_or_default(),
            https: vec![],
            latency: vec![],
            diagnosis: Some(Diagnosis {
                status: OverallStatus::Degraded,
                findings,
                limitations: vec![],
            }),
            performance: None,
            unavailable: vec![UnavailableMetric {
                name: "speed".into(),
                reason: "not measured".into(),
            }],
        }
    }

    #[test]
    fn action_allowlist_rejects_shell_text() {
        assert!(action_supported("flush_dns_cache"));
        assert!(!action_supported("ipconfig /flushdns && whoami"));
        assert!(!action_supported(""));
    }

    #[test]
    fn recommendations_require_measured_dns_findings_and_explain_hypothesis() {
        let none = scan(
            vec![Finding {
                id: "healthy".into(),
                severity: Severity::Info,
                confidence: Confidence::High,
                title: "Healthy".into(),
                detail: "ok".into(),
                evidence: vec![],
            }],
            Some(8.0),
        );
        assert!(recommendations(&none).is_empty());
        let dns = scan(
            vec![Finding {
                id: "dns_failure".into(),
                severity: Severity::Critical,
                confidence: Confidence::High,
                title: "DNS failed".into(),
                detail: "Direct IP worked".into(),
                evidence: vec!["0/2 DNS lookups succeeded".into()],
            }],
            None,
        );
        let recs = recommendations(&dns);
        assert_eq!(recs.len(), 1);
        assert!(recs[0].hypothesis);
        assert_eq!(recs[0].evidence, vec!["0/2 DNS lookups succeeded"]);
        assert!(recs[0].expected_effect.contains("cannot repair"));
    }

    #[test]
    fn comparisons_report_missing_and_no_measurable_improvement() {
        let before = scan(vec![], Some(20.0));
        let near = scan(vec![], Some(20.5));
        assert_eq!(
            compare(&before, &near)[0].result,
            "no measurable improvement"
        );
        let missing = scan(vec![], None);
        assert_eq!(
            compare(&before, &missing)[0].result,
            "measurement unavailable on one or both scans"
        );
    }

    #[test]
    fn comparisons_pair_the_same_host_instead_of_mixing_successful_targets() {
        let mut before = scan(vec![], Some(20.0));
        before.config.dns_hosts.push("b.test".into());
        before.dns.push(crate::network::DnsResult {
            host: "b.test".into(),
            ok: true,
            time_ms: Some(100.0),
            address_count: 1,
            timed_out: false,
            error: None,
        });
        let mut after = scan(vec![], Some(19.0));
        after.config.dns_hosts.push("b.test".into());
        after.dns.push(crate::network::DnsResult {
            host: "b.test".into(),
            ok: false,
            time_ms: None,
            address_count: 0,
            timed_out: true,
            error: Some("timeout".into()),
        });
        let values = compare(&before, &after);
        let failed = values
            .iter()
            .find(|value| value.metric == "dns:b.test")
            .unwrap();
        assert_eq!(failed.before, Some(100.0));
        assert_eq!(failed.after, None);
        assert!(failed.delta.is_none());
    }

    #[test]
    fn execution_failures_and_partial_success_have_distinct_states() {
        assert_eq!(execution_state(false, false), "failed");
        assert_eq!(
            execution_state(true, false),
            "succeeded_verification_failed"
        );
        assert_eq!(execution_state(true, true), "succeeded_and_verified");
    }
}
