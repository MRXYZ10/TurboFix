//! Real network probes. Every function takes an explicit timeout and a
//! cancellation token and returns a typed result; nothing is simulated.
//!
//! Probe methods:
//! * DNS: the operating system resolver (`getaddrinfo`) is timed, so results
//!   reflect what applications on this PC actually experience (including the
//!   resolver cache, which is noted in the docs).
//! * Latency: TCP connect time (SYN to SYN/ACK). ICMP echo normally needs
//!   privileges or platform-specific APIs, so it is deliberately not used.
//! * Gateway: TCP connect to common router ports. A refusal shows that a TCP
//!   reset was returned (potentially by an intermediary); a timeout is only
//!   *inconclusive* because routers often drop probes silently.
//! * HTTPS: a HEAD request with certificate validation.

use crate::error::{CoreError, Result};
use crate::stats::{summarize_outcomes, LatencyStats};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use tokio::net::{lookup_host, TcpStream};
use tokio_util::sync::CancellationToken;

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

// ---------------------------------------------------------------- interfaces

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InterfaceInfo {
    pub name: String,
    pub kind: String,
    pub is_up: bool,
    pub has_ipv4: bool,
    pub has_ipv6: bool,
    pub is_default_route: bool,
    pub link_speed_mbps: Option<u64>,
}

/// Interfaces as reported by the OS. Addresses and MAC addresses are
/// intentionally *not* copied into reports (privacy).
pub fn list_interfaces() -> Vec<InterfaceInfo> {
    let default_index = default_net::get_default_interface().ok().map(|i| i.index);
    default_net::get_interfaces()
        .into_iter()
        .filter(|i| !i.is_loopback())
        .map(|i| InterfaceInfo {
            name: i.friendly_name.clone().unwrap_or_else(|| i.name.clone()),
            kind: format!("{:?}", i.if_type),
            is_up: i.is_up(),
            has_ipv4: !i.ipv4.is_empty(),
            has_ipv6: !i.ipv6.is_empty(),
            is_default_route: Some(i.index) == default_index,
            link_speed_mbps: i.receive_speed.map(|b| b / 1_000_000).filter(|v| *v > 0),
        })
        .collect()
}

pub fn default_gateway_ip() -> Option<std::net::IpAddr> {
    default_net::get_default_gateway().ok().map(|g| g.ip_addr)
}

// ----------------------------------------------------------------------- TCP

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProbeStage {
    Resolution,
    Connection,
}

/// Public, serialized outcome for one TCP connect attempt.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum ProbeOutcome {
    Connected {
        latency_ms: f64,
    },
    /// A TCP reset was returned for the attempt. This commonly means the port
    /// is closed, though a firewall or intermediary can also generate a reset.
    ConnectionRefused {
        latency_ms: f64,
    },
    Timeout {
        stage: ProbeStage,
    },
    ResolutionFailed {
        message: String,
    },
    TransportError {
        message: String,
    },
}

async fn classify_connection<F>(future: F, timeout: Duration) -> ProbeOutcome
where
    F: Future<Output = io::Result<()>>,
{
    let start = Instant::now();
    match tokio::time::timeout(timeout, future).await {
        Err(_) => ProbeOutcome::Timeout {
            stage: ProbeStage::Connection,
        },
        Ok(Ok(())) => ProbeOutcome::Connected {
            latency_ms: ms(start.elapsed()),
        },
        Ok(Err(e)) if e.kind() == io::ErrorKind::ConnectionRefused => {
            ProbeOutcome::ConnectionRefused {
                latency_ms: ms(start.elapsed()),
            }
        }
        Ok(Err(e)) => ProbeOutcome::TransportError {
            message: e.to_string(),
        },
    }
}

async fn resolve_one(target: &str) -> std::io::Result<SocketAddr> {
    lookup_host(target)
        .await?
        .next()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no addresses"))
}

fn resolution_failure(error: io::Error) -> ProbeOutcome {
    ProbeOutcome::ResolutionFailed {
        message: error.to_string(),
    }
}

/// Time a single TCP connect. Name resolution (if any) happens first and is
/// *not* included in the measured time.
pub async fn tcp_connect(
    target: &str,
    timeout: Duration,
    cancel: &CancellationToken,
) -> Result<ProbeOutcome> {
    let addr = tokio::select! {
        biased;
        _ = cancel.cancelled() => return Err(CoreError::Cancelled),
        r = tokio::time::timeout(timeout, resolve_one(target)) => match r {
            Err(_) => return Ok(ProbeOutcome::Timeout { stage: ProbeStage::Resolution }),
            Ok(Err(e)) => return Ok(resolution_failure(e)),
            Ok(Ok(a)) => a,
        },
    };
    let connect = async { TcpStream::connect(addr).await.map(|_| ()) };
    let res = tokio::select! {
        biased;
        _ = cancel.cancelled() => return Err(CoreError::Cancelled),
        outcome = classify_connection(connect, timeout) => outcome,
    };
    Ok(res)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LatencyResult {
    pub target: String,
    pub stats: LatencyStats,
    /// Per-probe RTT in ms in probe order; `None` = failed probe.
    pub samples: Vec<Option<f64>>,
    /// Added in API v2. Empty on older stored reports; `samples` remains for
    /// compatibility and contains only successful TCP-connect timings.
    #[serde(default)]
    pub outcomes: Vec<ProbeOutcome>,
    pub last_error: Option<String>,
}

pub async fn measure_latency(
    target: &str,
    count: u32,
    interval: Duration,
    timeout: Duration,
    cancel: &CancellationToken,
) -> Result<LatencyResult> {
    let mut samples = Vec::with_capacity(count as usize);
    let mut outcomes = Vec::with_capacity(count as usize);
    let mut last_error = None;
    for i in 0..count {
        if i > 0 {
            tokio::select! {
            biased;
                    _ = cancel.cancelled() => return Err(CoreError::Cancelled),
                    _ = tokio::time::sleep(interval) => {}
                }
        }
        let outcome = tcp_connect(target, timeout, cancel).await?;
        match &outcome {
            ProbeOutcome::Connected { latency_ms } => samples.push(Some(*latency_ms)),
            ProbeOutcome::ConnectionRefused { .. } => {
                last_error = Some("connection refused".into());
                samples.push(None);
            }
            ProbeOutcome::Timeout { stage } => {
                last_error = Some(format!("{stage:?} timed out"));
                samples.push(None);
            }
            ProbeOutcome::ResolutionFailed { message } => {
                last_error = Some(format!("resolution failed: {message}"));
                samples.push(None);
            }
            ProbeOutcome::TransportError { message } => {
                last_error = Some(message.clone());
                samples.push(None);
            }
        }
        outcomes.push(outcome);
    }
    Ok(LatencyResult {
        target: target.to_string(),
        stats: summarize_outcomes(&outcomes),
        samples,
        outcomes,
        last_error,
    })
}

// ------------------------------------------------------------------- gateway

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum GatewayStatus {
    /// Gateway answered (accepted or actively refused a connection).
    Reachable { rtt_ms: f64 },
    /// No answer on probed ports; many routers silently drop such packets.
    Inconclusive,
    /// No default gateway known (or the platform cannot report one).
    Unavailable { reason: String },
}

pub async fn check_gateway(timeout: Duration, cancel: &CancellationToken) -> Result<GatewayStatus> {
    let Some(ip) = default_gateway_ip() else {
        return Ok(GatewayStatus::Unavailable {
            reason: "no default gateway reported by the OS".into(),
        });
    };
    for port in [443u16, 80, 53] {
        let target = match ip {
            std::net::IpAddr::V4(v4) => format!("{v4}:{port}"),
            std::net::IpAddr::V6(v6) => format!("[{v6}]:{port}"),
        };
        match tcp_connect(&target, timeout, cancel).await? {
            ProbeOutcome::Connected { latency_ms: m }
            | ProbeOutcome::ConnectionRefused { latency_ms: m } => {
                return Ok(GatewayStatus::Reachable { rtt_ms: m })
            }
            _ => {}
        }
    }
    Ok(GatewayStatus::Inconclusive)
}

// ----------------------------------------------------------------------- DNS

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DnsResult {
    pub host: String,
    pub ok: bool,
    pub time_ms: Option<f64>,
    pub address_count: usize,
    pub timed_out: bool,
    pub error: Option<String>,
}

pub async fn time_dns(
    host: &str,
    timeout: Duration,
    cancel: &CancellationToken,
) -> Result<DnsResult> {
    let start = Instant::now();
    let res = tokio::select! {
        biased;
        _ = cancel.cancelled() => return Err(CoreError::Cancelled),
        r = tokio::time::timeout(timeout, lookup_host((host, 443))) => r,
    };
    let elapsed = ms(start.elapsed());
    Ok(match res {
        Err(_) => DnsResult {
            host: host.into(),
            ok: false,
            time_ms: None,
            address_count: 0,
            timed_out: true,
            error: Some("timed out".into()),
        },
        Ok(Err(e)) => DnsResult {
            host: host.into(),
            ok: false,
            time_ms: None,
            address_count: 0,
            timed_out: false,
            error: Some(e.to_string()),
        },
        Ok(Ok(addrs)) => {
            let n = addrs.count();
            if n == 0 {
                DnsResult {
                    host: host.into(),
                    ok: false,
                    time_ms: None,
                    address_count: 0,
                    timed_out: false,
                    error: Some("no addresses returned".into()),
                }
            } else {
                DnsResult {
                    host: host.into(),
                    ok: true,
                    time_ms: Some(elapsed),
                    address_count: n,
                    timed_out: false,
                    error: None,
                }
            }
        }
    })
}

// --------------------------------------------------------------------- HTTPS

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HttpsResult {
    pub url: String,
    pub ok: bool,
    pub status: Option<u16>,
    pub time_ms: Option<f64>,
    pub timed_out: bool,
    pub error: Option<String>,
}

pub fn build_http_client(timeout: Duration) -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(timeout)
        .connect_timeout(timeout)
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("TurboFix/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| CoreError::Internal(format!("http client: {e}")))
}

/// Any HTTP response (even 4xx/5xx or a redirect) proves DNS + TCP + TLS worked.
pub async fn check_https(
    client: &reqwest::Client,
    url: &str,
    cancel: &CancellationToken,
) -> Result<HttpsResult> {
    let start = Instant::now();
    let res = tokio::select! {
        biased;
        _ = cancel.cancelled() => return Err(CoreError::Cancelled),
        r = client.head(url).send() => r,
    };
    Ok(match res {
        Ok(resp) => HttpsResult {
            url: url.into(),
            ok: true,
            status: Some(resp.status().as_u16()),
            time_ms: Some(ms(start.elapsed())),
            timed_out: false,
            error: None,
        },
        Err(e) => HttpsResult {
            url: url.into(),
            ok: false,
            status: None,
            time_ms: None,
            timed_out: e.is_timeout(),
            error: Some(if e.is_timeout() {
                "timed out".into()
            } else if e.is_connect() {
                "connection failed".into()
            } else {
                "request failed".into()
            }),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    const T: Duration = Duration::from_secs(10);

    #[tokio::test]
    async fn tcp_connect_to_local_listener_is_connected() {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap();
        let tok = CancellationToken::new();
        match tcp_connect(&addr.to_string(), T, &tok).await.unwrap() {
            ProbeOutcome::Connected { latency_ms: m } => assert!((0.0..1500.0).contains(&m)),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[tokio::test]
    async fn closed_port_is_refused_not_connected() {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap();
        drop(l);
        let tok = CancellationToken::new();
        let out = tcp_connect(&addr.to_string(), T, &tok).await.unwrap();
        assert!(
            matches!(out, ProbeOutcome::ConnectionRefused { .. }),
            "closed loopback port should refuse promptly: {out:?}"
        );
    }

    #[tokio::test]
    async fn zero_duration_connect_timeout_is_classified_at_connection_stage() {
        let out = classify_connection(std::future::pending(), Duration::from_millis(1)).await;
        assert_eq!(
            out,
            ProbeOutcome::Timeout {
                stage: ProbeStage::Connection
            }
        );
    }

    #[test]
    fn resolver_errors_are_classified_without_platform_specific_text() {
        for kind in [io::ErrorKind::NotFound, io::ErrorKind::Other] {
            assert!(matches!(
                resolution_failure(io::Error::new(kind, "platform detail")),
                ProbeOutcome::ResolutionFailed { .. }
            ));
        }
    }

    #[test]
    fn probe_outcomes_serialize_distinct_categories() {
        let values = [
            ProbeOutcome::Connected { latency_ms: 2.5 },
            ProbeOutcome::ConnectionRefused { latency_ms: 1.2 },
            ProbeOutcome::Timeout {
                stage: ProbeStage::Connection,
            },
            ProbeOutcome::Timeout {
                stage: ProbeStage::Resolution,
            },
            ProbeOutcome::ResolutionFailed {
                message: "NXDOMAIN".into(),
            },
            ProbeOutcome::TransportError {
                message: "network down".into(),
            },
        ];
        for value in values {
            let json = serde_json::to_string(&value).unwrap();
            let decoded: ProbeOutcome = serde_json::from_str(&json).unwrap();
            assert_eq!(decoded, value);
            assert!(json.contains("\"outcome\""));
        }
    }

    #[test]
    fn legacy_latency_json_remains_readable_without_claiming_packet_loss() {
        let old = r#"{"target":"host:443","stats":{"sent":2,"received":1,"loss_pct":50.0,"min_ms":10.0,"median_ms":10.0,"p95_ms":10.0,"max_ms":10.0,"mean_ms":10.0,"jitter_ms":null},"samples":[10.0,null],"last_error":"timed out"}"#;
        let result: LatencyResult = serde_json::from_str(old).unwrap();
        assert_eq!(result.stats.loss_pct, Some(50.0)); // retained only for old-report compatibility
        assert_eq!(result.outcomes, Vec::<ProbeOutcome>::new());
    }

    #[tokio::test]
    async fn latency_against_refused_port_reports_refusal_not_packet_loss() {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap();
        drop(l);
        let tok = CancellationToken::new();
        let r = measure_latency(&addr.to_string(), 3, Duration::from_millis(10), T, &tok)
            .await
            .unwrap();
        assert_eq!(r.stats.sent, 3);
        assert_eq!(r.stats.received, 0);
        assert_eq!(r.stats.loss_pct, None);
        assert_eq!(r.stats.refused, 3);
        assert_eq!(r.stats.tcp_failure_pct, Some(100.0));
        assert_eq!(r.stats.median_ms, None);
        assert!(r.last_error.is_some());
    }

    #[tokio::test]
    async fn latency_against_live_listener_succeeds() {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap();
        let tok = CancellationToken::new();
        let r = measure_latency(&addr.to_string(), 4, Duration::from_millis(10), T, &tok)
            .await
            .unwrap();
        assert_eq!(r.stats.received, 4);
        assert_eq!(r.stats.loss_pct, None);
        assert_eq!(r.stats.connected, 4);
        assert_eq!(r.stats.tcp_failure_pct, Some(0.0));
        assert!(r.stats.median_ms.is_some());
        assert!(r.stats.jitter_ms.is_some());
        drop(l);
    }

    #[tokio::test]
    async fn cancellation_is_honoured() {
        let tok = CancellationToken::new();
        tok.cancel();
        let err = measure_latency("127.0.0.1:9", 5, Duration::from_millis(10), T, &tok)
            .await
            .unwrap_err();
        assert!(matches!(err, CoreError::Cancelled));
        let err = time_dns("localhost", T, &tok).await.unwrap_err();
        assert!(matches!(err, CoreError::Cancelled));
    }

    #[tokio::test]
    async fn dns_localhost_resolves() {
        let tok = CancellationToken::new();
        let r = time_dns("localhost", T, &tok).await.unwrap();
        assert!(r.ok, "{r:?}");
        assert!(r.time_ms.is_some());
        assert!(r.address_count >= 1);
    }

    #[tokio::test]
    async fn https_to_closed_local_port_fails_cleanly() {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = l.local_addr().unwrap().port();
        drop(l);
        let client = build_http_client(T).unwrap();
        let tok = CancellationToken::new();
        let r = check_https(&client, &format!("https://127.0.0.1:{port}/"), &tok)
            .await
            .unwrap();
        assert!(!r.ok);
        assert_eq!(r.status, None);
        assert_eq!(r.time_ms, None);
    }

    #[tokio::test]
    async fn gateway_check_never_panics() {
        let tok = CancellationToken::new();
        let _ = check_gateway(Duration::from_millis(300), &tok)
            .await
            .unwrap();
    }
}
