//! Real network probes. Every function takes an explicit timeout and a
//! cancellation token and returns a typed result; nothing is simulated.
//!
//! Probe methods:
//! * DNS: the operating system resolver (`getaddrinfo`) is timed, so results
//!   reflect what applications on this PC actually experience (including the
//!   resolver cache, which is noted in the docs).
//! * Latency: TCP connect time (SYN to SYN/ACK). ICMP echo normally needs
//!   privileges or platform-specific APIs, so it is deliberately not used.
//! * Gateway: TCP connect to common router ports. A refusal (RST) proves the
//!   host answered; a timeout is only *inconclusive* because routers often drop
//!   packets silently.
//! * HTTPS: a HEAD request with certificate validation.

use crate::error::{CoreError, Result};
use crate::stats::{summarize, LatencyStats};
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Clone, PartialEq)]
pub enum TcpOutcome {
    Connected(f64),
    /// The peer answered with a reset: the host is reachable, nothing listens.
    Refused(f64),
    Timeout,
    Error(String),
}

async fn resolve_one(target: &str) -> std::io::Result<SocketAddr> {
    lookup_host(target)
        .await?
        .next()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no addresses"))
}

/// Time a single TCP connect. Name resolution (if any) happens first and is
/// *not* included in the measured time.
pub async fn tcp_connect(
    target: &str,
    timeout: Duration,
    cancel: &CancellationToken,
) -> Result<TcpOutcome> {
    let addr = tokio::select! {
        biased;
        _ = cancel.cancelled() => return Err(CoreError::Cancelled),
        r = tokio::time::timeout(timeout, resolve_one(target)) => match r {
            Err(_) => return Ok(TcpOutcome::Timeout),
            Ok(Err(e)) => return Ok(TcpOutcome::Error(format!("resolve failed: {e}"))),
            Ok(Ok(a)) => a,
        },
    };
    let start = Instant::now();
    let res = tokio::select! {
        biased;
        _ = cancel.cancelled() => return Err(CoreError::Cancelled),
        r = tokio::time::timeout(timeout, TcpStream::connect(addr)) => r,
    };
    Ok(match res {
        Err(_) => TcpOutcome::Timeout,
        Ok(Ok(_stream)) => TcpOutcome::Connected(ms(start.elapsed())),
        Ok(Err(e)) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
            TcpOutcome::Refused(ms(start.elapsed()))
        }
        Ok(Err(e)) => TcpOutcome::Error(e.to_string()),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LatencyResult {
    pub target: String,
    pub stats: LatencyStats,
    /// Per-probe RTT in ms in probe order; `None` = failed probe.
    pub samples: Vec<Option<f64>>,
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
    let mut last_error = None;
    for i in 0..count {
        if i > 0 {
            tokio::select! {
            biased;
                    _ = cancel.cancelled() => return Err(CoreError::Cancelled),
                    _ = tokio::time::sleep(interval) => {}
                }
        }
        match tcp_connect(target, timeout, cancel).await? {
            TcpOutcome::Connected(m) => samples.push(Some(m)),
            TcpOutcome::Refused(_) => {
                last_error = Some("connection refused".to_string());
                samples.push(None);
            }
            TcpOutcome::Timeout => {
                last_error = Some("timed out".to_string());
                samples.push(None);
            }
            TcpOutcome::Error(e) => {
                last_error = Some(e);
                samples.push(None);
            }
        }
    }
    Ok(LatencyResult {
        target: target.to_string(),
        stats: summarize(&samples),
        samples,
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
            TcpOutcome::Connected(m) | TcpOutcome::Refused(m) => {
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

    const T: Duration = Duration::from_millis(1500);

    #[tokio::test]
    async fn tcp_connect_to_local_listener_is_connected() {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap();
        let tok = CancellationToken::new();
        match tcp_connect(&addr.to_string(), T, &tok).await.unwrap() {
            TcpOutcome::Connected(m) => assert!((0.0..1500.0).contains(&m)),
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
            !matches!(out, TcpOutcome::Connected(_)),
            "a closed port must never be considered connected: {out:?}"
        );
    }

    #[tokio::test]
    async fn latency_against_dead_port_reports_full_loss_and_no_fake_rtt() {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap();
        drop(l);
        let tok = CancellationToken::new();
        let r = measure_latency(&addr.to_string(), 3, Duration::from_millis(10), T, &tok)
            .await
            .unwrap();
        assert_eq!(r.stats.sent, 3);
        assert_eq!(r.stats.received, 0);
        assert_eq!(r.stats.loss_pct, 100.0);
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
        assert_eq!(r.stats.loss_pct, 0.0);
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
    async fn dns_failure_is_reported_with_no_time() {
        let tok = CancellationToken::new();
        let r = time_dns("does-not-exist.invalid", T, &tok).await.unwrap();
        assert!(!r.ok);
        assert_eq!(r.time_ms, None);
        assert!(r.error.is_some());
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
