//! Evidence-based classification. Pure functions over measured results so the
//! logic is unit-testable without a network. Statements never go beyond what
//! the evidence supports; each finding carries a confidence level.

use crate::network::{DnsResult, GatewayStatus, HttpsResult, InterfaceInfo, LatencyResult};
use crate::stats::median;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OverallStatus {
    Online,
    Degraded,
    Offline,
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Finding {
    pub id: String,
    pub severity: Severity,
    pub confidence: Confidence,
    pub title: String,
    pub detail: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Diagnosis {
    pub status: OverallStatus,
    pub findings: Vec<Finding>,
    pub limitations: Vec<String>,
}

pub struct Evidence<'a> {
    pub interfaces: &'a [InterfaceInfo],
    pub gateway: &'a GatewayStatus,
    pub dns: &'a [DnsResult],
    pub https: &'a [HttpsResult],
    pub latency: &'a [LatencyResult],
}

pub const SLOW_DNS_MS: f64 = 300.0;
pub const HIGH_LATENCY_MS: f64 = 150.0;
pub const HIGH_JITTER_MS: f64 = 30.0;
pub const LOSS_WARN_PCT: f64 = 2.0;
pub const LOSS_CRIT_PCT: f64 = 10.0;

fn finding(
    id: &str,
    severity: Severity,
    confidence: Confidence,
    title: &str,
    detail: &str,
    evidence: Vec<String>,
) -> Finding {
    Finding {
        id: id.into(),
        severity,
        confidence,
        title: title.into(),
        detail: detail.into(),
        evidence,
    }
}

pub fn standard_limitations() -> Vec<String> {
    vec![
        "Latency is measured as TCP connect time, not ICMP ping, so values can differ from the ping command.".into(),
        "Packet loss is the share of failed TCP probes; a firewall that drops probes can look like loss.".into(),
        "DNS timings use the Windows/OS resolver and may be served from its cache.".into(),
        "Download/upload throughput is not measured by this connectivity scan; use the separate Speed Test page for a measured result.".into(),
        "This scan cannot identify problems inside your ISP's network; it can only show where measurements fail.".into(),
    ]
}

pub fn diagnose(e: &Evidence) -> Diagnosis {
    let mut f: Vec<Finding> = Vec::new();

    let any_up = e
        .interfaces
        .iter()
        .any(|i| i.is_up && (i.has_ipv4 || i.has_ipv6));
    let dns_ok = e.dns.iter().filter(|d| d.ok).count();
    let dns_total = e.dns.len();
    let https_ok = e.https.iter().filter(|h| h.ok).count();
    let https_total = e.https.len();
    let tcp_ok = e.latency.iter().filter(|l| l.stats.received > 0).count();
    let tcp_total = e.latency.len();
    let external_ok = tcp_ok > 0 || https_ok > 0;

    // 1. No usable interface
    if !any_up {
        f.push(finding(
            "no_interface",
            Severity::Critical,
            Confidence::High,
            "No active network interface",
            "Windows reports no connected adapter with an IP address. Check that Wi-Fi is on or the cable is plugged in.",
            vec![format!("{} non-loopback interface(s) listed, none up with an address", e.interfaces.len())],
        ));
        return Diagnosis {
            status: OverallStatus::Offline,
            findings: f,
            limitations: standard_limitations(),
        };
    }

    // 2. Complete failure of everything external
    let everything_failed = tcp_total > 0 && tcp_ok == 0 && https_ok == 0 && dns_ok == 0;
    if everything_failed {
        match e.gateway {
            GatewayStatus::Reachable { rtt_ms } => f.push(finding(
                "external_unreachable",
                Severity::Critical,
                Confidence::Medium,
                "Router answers, but nothing beyond it does",
                "The local gateway responded while DNS, TCP and HTTPS tests to the internet all failed. This points past your PC, to the router's uplink, modem or ISP, but cannot show which.",
                vec![format!("gateway reachable in {rtt_ms:.1} ms"), format!("0/{dns_total} DNS lookups, 0/{tcp_total} TCP targets succeeded")],
            )),
            GatewayStatus::Inconclusive => f.push(finding(
                "gateway_silent",
                Severity::Critical,
                Confidence::Low,
                "No internet and the local router did not answer",
                "All internet tests failed and the router did not answer test connections. Routers often ignore such probes, so this is not proof the router is down, but it is the first thing to check (restart it, check Wi-Fi association).",
                vec![format!("0/{dns_total} DNS lookups, 0/{tcp_total} TCP targets succeeded")],
            )),
            GatewayStatus::Unavailable { reason } => f.push(finding(
                "no_default_route",
                Severity::Critical,
                Confidence::Medium,
                "No default route to the internet",
                "The OS reports no default gateway and all internet tests failed. The adapter may be connected without a working DHCP lease or router.",
                vec![reason.clone()],
            )),
        }
        return Diagnosis {
            status: OverallStatus::Offline,
            findings: f,
            limitations: standard_limitations(),
        };
    }

    // 3. DNS failing while raw IP connectivity works
    if dns_total > 0 && dns_ok == 0 && tcp_ok > 0 {
        f.push(finding(
            "dns_failure",
            Severity::Critical,
            Confidence::High,
            "DNS lookups fail while direct IP connections work",
            "Name resolution is broken but the internet itself is reachable by IP address. Websites will fail to load by name. The configured DNS server is the likely cause.",
            vec![format!("0/{dns_total} DNS lookups succeeded"), format!("{tcp_ok}/{tcp_total} TCP targets reachable by IP")],
        ));
    } else if dns_total > 0 && dns_ok < dns_total {
        f.push(finding(
            "dns_partial",
            Severity::Warning,
            Confidence::Medium,
            "Some DNS lookups failed",
            "Part of the test hostnames could not be resolved, which can mean an unreliable resolver or blocked domains.",
            e.dns.iter().filter(|d| !d.ok).map(|d| format!("{}: {}", d.host, d.error.clone().unwrap_or_default())).collect(),
        ));
    }

    // 4. Slow DNS
    let dns_times: Vec<f64> = e.dns.iter().filter_map(|d| d.time_ms).collect();
    if let Some(m) = median(&dns_times) {
        if m > SLOW_DNS_MS {
            f.push(finding(
                "dns_slow",
                Severity::Warning,
                Confidence::Medium,
                "DNS lookups are slow",
                "Slow name resolution delays the start of page loads but does not reduce download speed. A different DNS resolver may help; TurboFix does not change DNS settings yet.",
                vec![format!("median DNS time {m:.0} ms (threshold {SLOW_DNS_MS:.0} ms)")],
            ));
        }
    }

    // 5. HTTPS failing while TCP works
    if https_total > 0 && https_ok == 0 && tcp_ok > 0 && dns_ok > 0 {
        f.push(finding(
            "https_blocked",
            Severity::Warning,
            Confidence::Low,
            "HTTPS requests failed although TCP connections work",
            "Possible causes include a captive portal (hotel/cafe Wi-Fi sign-in), a proxy or filtering software, a wrong system clock, or the test endpoints being blocked.",
            e.https.iter().map(|h| format!("{}: {}", h.url, h.error.clone().unwrap_or_default())).collect(),
        ));
    }

    // 6. Latency / jitter / loss per target
    for l in e.latency.iter().filter(|l| l.stats.received > 0) {
        let s = &l.stats;
        if s.loss_pct >= LOSS_CRIT_PCT {
            f.push(finding("packet_loss", Severity::Critical, Confidence::Medium, "High probe loss", "Many connection probes failed. On Wi-Fi this is commonly weak signal or interference; on any link it can indicate congestion.", vec![format!("{}: {:.0}% of {} probes failed", l.target, s.loss_pct, s.sent)]));
        } else if s.loss_pct >= LOSS_WARN_PCT {
            f.push(finding(
                "packet_loss",
                Severity::Warning,
                Confidence::Medium,
                "Some probe loss",
                "A few connection probes failed, which can cause stutter in calls and games.",
                vec![format!(
                    "{}: {:.0}% of {} probes failed",
                    l.target, s.loss_pct, s.sent
                )],
            ));
        }
        if let Some(m) = s.median_ms {
            if m > HIGH_LATENCY_MS {
                f.push(finding("high_latency", Severity::Warning, Confidence::Medium, "High latency", "Round trips are slow. Distance, congestion, VPNs and weak Wi-Fi all add latency.", vec![format!("{}: median {m:.0} ms (threshold {HIGH_LATENCY_MS:.0} ms)", l.target)]));
            }
        }
        if let Some(j) = s.jitter_ms {
            if j > HIGH_JITTER_MS {
                f.push(finding("high_jitter", Severity::Warning, Confidence::Medium, "Unstable latency (jitter)", "Latency varies a lot between probes, which hurts video calls and online games even when average speed is fine.", vec![format!("{}: jitter {j:.0} ms (threshold {HIGH_JITTER_MS:.0} ms)", l.target)]));
            }
        }
    }

    // 7. Reachability of some targets only
    if tcp_total > 1 && tcp_ok > 0 && tcp_ok < tcp_total {
        f.push(finding("partial_reach", Severity::Warning, Confidence::Low, "Some test targets are unreachable", "At least one probe target could not be reached while others could. The target itself or a route to it may be blocked.", e.latency.iter().filter(|l| l.stats.received == 0).map(|l| format!("{}: {}", l.target, l.last_error.clone().unwrap_or_default())).collect()));
    }

    let status = if !external_ok && tcp_total + https_total == 0 {
        OverallStatus::Unknown
    } else if f.iter().any(|x| x.severity != Severity::Info) {
        OverallStatus::Degraded
    } else {
        OverallStatus::Online
    };

    if f.is_empty() {
        f.push(finding("healthy", Severity::Info, Confidence::Medium, "No problems detected in these tests", "Connectivity, DNS, latency and HTTPS checks were within normal ranges. This does not measure download or upload speed.", vec![]));
    }

    Diagnosis {
        status,
        findings: f,
        limitations: standard_limitations(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats::summarize;

    fn iface(up: bool) -> InterfaceInfo {
        InterfaceInfo {
            name: "Wi-Fi".into(),
            kind: "Wireless80211".into(),
            is_up: up,
            has_ipv4: true,
            has_ipv6: false,
            is_default_route: true,
            link_speed_mbps: None,
        }
    }
    fn dns(ok: bool, t: f64) -> DnsResult {
        DnsResult {
            host: "h.example".into(),
            ok,
            time_ms: ok.then_some(t),
            address_count: ok as usize,
            timed_out: false,
            error: (!ok).then(|| "fail".into()),
        }
    }
    fn https(ok: bool) -> HttpsResult {
        HttpsResult {
            url: "https://x.example".into(),
            ok,
            status: ok.then_some(200),
            time_ms: ok.then_some(50.0),
            timed_out: false,
            error: (!ok).then(|| "connection failed".into()),
        }
    }
    fn lat(samples: Vec<Option<f64>>) -> LatencyResult {
        LatencyResult {
            target: "1.1.1.1:443".into(),
            stats: summarize(&samples),
            samples,
            last_error: Some("timed out".into()),
        }
    }
    fn run(
        i: &[InterfaceInfo],
        g: GatewayStatus,
        d: &[DnsResult],
        h: &[HttpsResult],
        l: &[LatencyResult],
    ) -> Diagnosis {
        diagnose(&Evidence {
            interfaces: i,
            gateway: &g,
            dns: d,
            https: h,
            latency: l,
        })
    }
    fn has(d: &Diagnosis, id: &str) -> bool {
        d.findings.iter().any(|f| f.id == id)
    }
    fn good_lat() -> LatencyResult {
        lat(vec![Some(20.0), Some(21.0), Some(20.5), Some(22.0)])
    }

    #[test]
    fn healthy() {
        let d = run(
            &[iface(true)],
            GatewayStatus::Reachable { rtt_ms: 2.0 },
            &[dns(true, 20.0)],
            &[https(true)],
            &[good_lat()],
        );
        assert_eq!(d.status, OverallStatus::Online);
        assert!(has(&d, "healthy"));
    }

    #[test]
    fn no_interface_is_offline() {
        let d = run(
            &[iface(false)],
            GatewayStatus::Unavailable { reason: "x".into() },
            &[],
            &[],
            &[],
        );
        assert_eq!(d.status, OverallStatus::Offline);
        assert!(has(&d, "no_interface"));
    }

    #[test]
    fn router_ok_but_internet_dead() {
        let d = run(
            &[iface(true)],
            GatewayStatus::Reachable { rtt_ms: 1.0 },
            &[dns(false, 0.0)],
            &[https(false)],
            &[lat(vec![None, None])],
        );
        assert_eq!(d.status, OverallStatus::Offline);
        assert!(has(&d, "external_unreachable"));
        assert_eq!(d.findings[0].confidence, Confidence::Medium); // never claims ISP cause with certainty
    }

    #[test]
    fn silent_gateway_is_low_confidence() {
        let d = run(
            &[iface(true)],
            GatewayStatus::Inconclusive,
            &[dns(false, 0.0)],
            &[],
            &[lat(vec![None])],
        );
        assert!(has(&d, "gateway_silent"));
        assert_eq!(d.findings[0].confidence, Confidence::Low);
    }

    #[test]
    fn dns_failure_with_ip_connectivity() {
        let d = run(
            &[iface(true)],
            GatewayStatus::Reachable { rtt_ms: 1.0 },
            &[dns(false, 0.0), dns(false, 0.0)],
            &[https(false)],
            &[good_lat()],
        );
        assert!(has(&d, "dns_failure"));
        assert_eq!(d.status, OverallStatus::Degraded);
    }

    #[test]
    fn slow_dns_flagged_and_does_not_claim_bandwidth() {
        let d = run(
            &[iface(true)],
            GatewayStatus::Reachable { rtt_ms: 1.0 },
            &[dns(true, 500.0)],
            &[https(true)],
            &[good_lat()],
        );
        let f = d.findings.iter().find(|f| f.id == "dns_slow").unwrap();
        assert!(f.detail.contains("does not reduce download speed"));
    }

    #[test]
    fn loss_jitter_latency_thresholds() {
        let l = lat(vec![
            Some(200.0),
            None,
            Some(260.0),
            Some(190.0),
            Some(300.0),
            Some(210.0),
            Some(205.0),
            Some(500.0),
            Some(220.0),
            Some(215.0),
        ]);
        let d = run(
            &[iface(true)],
            GatewayStatus::Reachable { rtt_ms: 1.0 },
            &[dns(true, 10.0)],
            &[https(true)],
            &[l],
        );
        assert!(has(&d, "packet_loss"));
        assert!(has(&d, "high_latency"));
        assert!(has(&d, "high_jitter"));
    }

    #[test]
    fn https_blocked_only() {
        let d = run(
            &[iface(true)],
            GatewayStatus::Reachable { rtt_ms: 1.0 },
            &[dns(true, 10.0)],
            &[https(false)],
            &[good_lat()],
        );
        assert!(has(&d, "https_blocked"));
        assert_eq!(
            d.findings
                .iter()
                .find(|f| f.id == "https_blocked")
                .unwrap()
                .confidence,
            Confidence::Low
        );
    }

    #[test]
    fn always_lists_limitations() {
        let d = run(
            &[iface(true)],
            GatewayStatus::Reachable { rtt_ms: 1.0 },
            &[dns(true, 10.0)],
            &[https(true)],
            &[good_lat()],
        );
        assert!(d.limitations.iter().any(|l| l.contains("not measured")));
    }
}
