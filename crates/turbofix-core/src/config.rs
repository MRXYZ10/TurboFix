//! Scan configuration. Every value that reaches the engine is validated here.

use crate::error::{CoreError, Result};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScanConfig {
    pub dns_hosts: Vec<String>,
    /// `host:port` TCP-connect targets (an IP literal keeps DNS out of the latency numbers).
    pub latency_targets: Vec<String>,
    pub https_endpoints: Vec<String>,
    pub probe_count: u32,
    pub probe_interval_ms: u64,
    pub probe_timeout_ms: u64,
    pub dns_timeout_ms: u64,
    pub https_timeout_ms: u64,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            dns_hosts: vec![
                "www.cloudflare.com".into(),
                "www.google.com".into(),
                "www.microsoft.com".into(),
            ],
            latency_targets: vec!["1.1.1.1:443".into(), "8.8.8.8:443".into()],
            https_endpoints: vec![
                "https://www.cloudflare.com/".into(),
                "https://www.microsoft.com/".into(),
            ],
            probe_count: 10,
            probe_interval_ms: 200,
            probe_timeout_ms: 2000,
            dns_timeout_ms: 3000,
            https_timeout_ms: 5000,
        }
    }
}

fn bad<T>(msg: impl Into<String>) -> Result<T> {
    Err(CoreError::InvalidConfig(msg.into()))
}

fn range(name: &str, v: u64, lo: u64, hi: u64) -> Result<()> {
    if v < lo || v > hi {
        return bad(format!("{name} must be between {lo} and {hi} (got {v})"));
    }
    Ok(())
}

/// Conservative hostname check: ASCII letters, digits, `-`, dots.
pub fn validate_hostname(host: &str) -> Result<()> {
    if host.is_empty()
        || host.len() > 253
        || host.starts_with('.')
        || host.ends_with('.')
        || host.contains("..")
    {
        return bad(format!("malformed hostname '{host}'"));
    }
    for label in host.split('.') {
        let ok = !label.is_empty()
            && label.len() <= 63
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
        if !ok {
            return bad(format!("malformed hostname '{host}'"));
        }
    }
    Ok(())
}

fn validate_target(t: &str) -> Result<()> {
    if let Ok(addr) = t.parse::<SocketAddr>() {
        return if addr.port() == 0 {
            bad("target port must not be 0")
        } else {
            Ok(())
        };
    }
    let (host, port) = t.rsplit_once(':').ok_or_else(|| {
        CoreError::InvalidConfig(format!("target '{t}' must look like host:port"))
    })?;
    validate_hostname(host)?;
    match port.parse::<u16>() {
        Ok(p) if p != 0 => Ok(()),
        _ => bad(format!("target '{t}' has an invalid port")),
    }
}

fn validate_https(raw: &str) -> Result<()> {
    let url = url::Url::parse(raw)
        .map_err(|e| CoreError::InvalidConfig(format!("invalid URL '{raw}': {e}")))?;
    if url.scheme() != "https" {
        return bad(format!("endpoint '{raw}' must use https"));
    }
    if url.host_str().is_none() || !url.username().is_empty() || url.password().is_some() {
        return bad(format!(
            "endpoint '{raw}' must have a host and no credentials"
        ));
    }
    Ok(())
}

impl ScanConfig {
    pub fn validate(&self) -> Result<()> {
        range("probe_count", self.probe_count as u64, 1, 100)?;
        range("probe_interval_ms", self.probe_interval_ms, 0, 5000)?;
        range("probe_timeout_ms", self.probe_timeout_ms, 200, 10_000)?;
        range("dns_timeout_ms", self.dns_timeout_ms, 200, 15_000)?;
        range("https_timeout_ms", self.https_timeout_ms, 500, 30_000)?;
        range("dns_hosts count", self.dns_hosts.len() as u64, 1, 8)?;
        range(
            "latency_targets count",
            self.latency_targets.len() as u64,
            1,
            8,
        )?;
        range(
            "https_endpoints count",
            self.https_endpoints.len() as u64,
            0,
            8,
        )?;
        self.dns_hosts
            .iter()
            .try_for_each(|h| validate_hostname(h))?;
        self.latency_targets
            .iter()
            .try_for_each(|t| validate_target(t))?;
        self.https_endpoints
            .iter()
            .try_for_each(|u| validate_https(u))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid() {
        ScanConfig::default().validate().unwrap();
    }

    #[test]
    fn numeric_ranges_enforced() {
        for mutate in [
            (|c: &mut ScanConfig| c.probe_count = 0) as fn(&mut ScanConfig),
            |c| c.probe_count = 101,
            |c| c.probe_timeout_ms = 10,
            |c| c.probe_interval_ms = 60_000,
            |c| c.dns_timeout_ms = 999_999,
            |c| c.https_timeout_ms = 1,
            |c| c.latency_targets.clear(),
            |c| c.dns_hosts.clear(),
        ] {
            let mut c = ScanConfig::default();
            mutate(&mut c);
            assert_eq!(c.validate().unwrap_err().code(), "invalid_config");
        }
    }

    #[test]
    fn hostnames_validated() {
        for h in [
            "",
            "-a.com",
            "a..com",
            "a b.com",
            "exa$mple.com",
            ".x.com",
            "x.com.",
            "a_b.com",
            "http://x.com",
        ] {
            assert!(validate_hostname(h).is_err(), "{h:?}");
        }
        for h in [
            "example.com",
            "a-b.example.org",
            "localhost",
            "1.1.1.1",
            "does-not-exist.invalid",
        ] {
            validate_hostname(h).unwrap();
        }
    }

    #[test]
    fn targets_accept_ip_or_host_with_valid_port() {
        for t in [
            "1.1.1.1:443",
            "[2606:4700:4700::1111]:443",
            "example.com:80",
            "127.0.0.1:9",
        ] {
            validate_target(t).unwrap();
        }
        for t in [
            "1.1.1.1",
            "1.1.1.1:0",
            "example.com:99999",
            "bad host:80",
            ":443",
            "x:y",
        ] {
            assert!(validate_target(t).is_err(), "{t:?}");
        }
    }

    #[test]
    fn https_endpoints_must_be_https_without_credentials() {
        let mut c = ScanConfig::default();
        for u in [
            "http://example.com",
            "ftp://example.com",
            "https://user:pw@example.com",
            "nonsense",
        ] {
            c.https_endpoints = vec![u.into()];
            assert!(c.validate().is_err(), "{u}");
        }
        c.https_endpoints = vec![];
        c.validate().unwrap();
    }

    #[test]
    fn json_roundtrip_and_partial_json_uses_defaults() {
        let c = ScanConfig::default();
        assert_eq!(
            serde_json::from_str::<ScanConfig>(&serde_json::to_string(&c).unwrap()).unwrap(),
            c
        );
        let p: ScanConfig = serde_json::from_str(r#"{"probe_count": 4}"#).unwrap();
        assert_eq!(p.probe_count, 4);
        assert_eq!(p.dns_hosts, ScanConfig::default().dns_hosts);
    }
}
