//! Read-only Wi-Fi details for Windows using fixed, non-interactive `netsh` queries.
//! No user-provided input is ever passed to the OS process.

#[cfg(windows)]
use crate::error::CoreError;
use crate::error::Result;
use serde::{Deserialize, Serialize};
#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NearbyNetwork {
    pub ssid: String,
    pub signal_percent: Option<u8>,
    pub channel: Option<u16>,
    pub radio_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WifiSnapshot {
    pub supported: bool,
    pub connected: bool,
    pub ssid: Option<String>,
    pub signal_percent: Option<u8>,
    pub channel: Option<u16>,
    pub receive_mbps: Option<f64>,
    pub transmit_mbps: Option<f64>,
    pub radio_type: Option<String>,
    pub band_hint: Option<String>,
    pub nearby_networks: Vec<NearbyNetwork>,
    pub checked_at: String,
    pub note: String,
}

#[cfg(windows)]
fn value<'a>(text: &'a str, label: &str) -> Option<&'a str> {
    text.lines().find_map(|line| {
        let (key, val) = line.split_once(':')?;
        key.trim().eq_ignore_ascii_case(label).then_some(val.trim())
    })
}

#[cfg(any(windows, test))]
fn numeric_prefix<T: std::str::FromStr>(value: &str) -> Option<T> {
    let trimmed = value.trim().trim_end_matches('%').trim();
    let end = trimmed
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(trimmed.len());
    trimmed[..end].parse().ok()
}

#[cfg(any(windows, test))]
fn band_hint(channel: Option<u16>, radio_type: Option<&str>) -> Option<String> {
    let channel = channel?;
    let radio = radio_type.unwrap_or("").to_ascii_lowercase();
    if channel <= 14 {
        if radio.contains("ax") || radio.contains("be") {
            Some("2.4 or 6 GHz (channel alone cannot distinguish)".into())
        } else {
            Some("2.4 GHz (channel-based estimate)".into())
        }
    } else if (32..=177).contains(&channel) {
        Some("5 GHz (channel-based estimate)".into())
    } else {
        Some("Band not determined from driver output".into())
    }
}

#[cfg(windows)]
fn run_netsh(args: &[&str]) -> Result<String> {
    let root = std::env::var_os("SystemRoot").ok_or_else(|| {
        CoreError::Internal("Windows SystemRoot environment variable is unavailable".into())
    })?;
    let executable = std::path::PathBuf::from(root)
        .join("System32")
        .join("netsh.exe");
    let output = Command::new(executable)
        .creation_flags(0x08000000)
        .args(args)
        .output()
        .map_err(|e| CoreError::Internal(format!("could not run Windows Wi-Fi query: {e}")))?;
    if !output.status.success() {
        return Err(CoreError::Internal(format!(
            "Windows Wi-Fi query failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    // The fields we parse are normally ASCII. If the SSID uses characters outside UTF-8,
    // the UI may display replacement characters; measurement fields remain usable.
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(any(windows, test))]
fn parse_nearby(text: &str) -> Vec<NearbyNetwork> {
    let mut items: Vec<NearbyNetwork> = Vec::new();
    let mut current: Option<NearbyNetwork> = None;
    let flush = |current: &mut Option<NearbyNetwork>, items: &mut Vec<NearbyNetwork>| {
        if let Some(network) = current.take() {
            if !items.iter().any(|n| n.ssid == network.ssid) {
                items.push(network);
            } else if let Some(existing) = items.iter_mut().find(|n| n.ssid == network.ssid) {
                if network.signal_percent.unwrap_or(0) > existing.signal_percent.unwrap_or(0) {
                    existing.signal_percent = network.signal_percent;
                    existing.channel = network.channel;
                    existing.radio_type = network.radio_type;
                }
            }
        }
    };
    for line in text.lines() {
        let Some((key, raw)) = line.trim().split_once(':') else {
            continue;
        };
        let key = key.trim();
        let raw = raw.trim();
        if let Some(rest) = key.strip_prefix("SSID ") {
            if rest.trim().parse::<u32>().is_ok() {
                flush(&mut current, &mut items);
                current = Some(NearbyNetwork {
                    ssid: if raw.is_empty() {
                        "(Hidden network)".into()
                    } else {
                        raw.into()
                    },
                    signal_percent: None,
                    channel: None,
                    radio_type: None,
                });
                continue;
            }
        }
        if let Some(network) = current.as_mut() {
            if key.eq_ignore_ascii_case("Signal") {
                network.signal_percent = numeric_prefix::<u8>(raw).map(|v| v.min(100));
            } else if key.eq_ignore_ascii_case("Channel") {
                network.channel = numeric_prefix::<u16>(raw);
            } else if key.eq_ignore_ascii_case("Radio type") {
                network.radio_type = Some(raw.into());
            }
        }
    }
    flush(&mut current, &mut items);
    items.sort_by(|a, b| {
        b.signal_percent
            .unwrap_or(0)
            .cmp(&a.signal_percent.unwrap_or(0))
    });
    items.truncate(12);
    items
}

/// Return live Wi-Fi metadata. Unsupported platforms return a truthful empty snapshot.
pub fn snapshot() -> Result<WifiSnapshot> {
    let checked_at = chrono::Utc::now().to_rfc3339();
    #[cfg(windows)]
    {
        let interfaces = run_netsh(&["wlan", "show", "interfaces"])?;
        let state = value(&interfaces, "State").unwrap_or("");
        let connected = state.eq_ignore_ascii_case("connected");
        let ssid = connected
            .then(|| value(&interfaces, "SSID").unwrap_or("").to_string())
            .filter(|s| !s.is_empty());
        let signal_percent = value(&interfaces, "Signal")
            .and_then(numeric_prefix::<u8>)
            .map(|v| v.min(100));
        let channel = value(&interfaces, "Channel").and_then(numeric_prefix::<u16>);
        let receive_mbps =
            value(&interfaces, "Receive rate (Mbps)").and_then(numeric_prefix::<f64>);
        let transmit_mbps =
            value(&interfaces, "Transmit rate (Mbps)").and_then(numeric_prefix::<f64>);
        let radio_type = value(&interfaces, "Radio type").map(str::to_owned);
        let band_hint = band_hint(channel, radio_type.as_deref());
        let nearby_text =
            run_netsh(&["wlan", "show", "networks", "mode=bssid"]).unwrap_or_default();
        let nearby_networks = parse_nearby(&nearby_text);
        Ok(WifiSnapshot {
            supported: true,
            connected,
            ssid,
            signal_percent,
            channel,
            receive_mbps,
            transmit_mbps,
            radio_type,
            band_hint,
            nearby_networks,
            checked_at,
            note: "Wi-Fi values are read-only. Link rates are adapter-to-router rates, not internet speed. Nearby SSIDs are visible on this PC only and are not stored.".into(),
        })
    }
    #[cfg(not(windows))]
    {
        Ok(WifiSnapshot {
            supported: false,
            connected: false,
            ssid: None,
            signal_percent: None,
            channel: None,
            receive_mbps: None,
            transmit_mbps: None,
            radio_type: None,
            band_hint: None,
            nearby_networks: Vec::new(),
            checked_at,
            note: "Live Wi-Fi diagnostics are currently available only on Windows 10/11.".into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_numeric_values_with_units() {
        assert_eq!(numeric_prefix::<u8>("87%"), Some(87));
        assert_eq!(numeric_prefix::<u16>("149"), Some(149));
        assert_eq!(numeric_prefix::<f64>("1201.0 Mbps"), Some(1201.0));
        assert_eq!(numeric_prefix::<u8>("not available"), None);
    }

    #[test]
    fn does_not_claim_a_precise_band_for_ambiguous_ax_channels() {
        assert_eq!(
            band_hint(Some(6), Some("802.11n")),
            Some("2.4 GHz (channel-based estimate)".into())
        );
        assert_eq!(
            band_hint(Some(6), Some("802.11ax")),
            Some("2.4 or 6 GHz (channel alone cannot distinguish)".into())
        );
        assert_eq!(
            band_hint(Some(149), Some("802.11ac")),
            Some("5 GHz (channel-based estimate)".into())
        );
        assert_eq!(band_hint(None, None), None);
    }

    #[test]
    fn parses_nearby_networks_and_keeps_only_best_duplicate_signal() {
        let sample = "SSID 1 : Home\n    BSSID 1 : xx\n         Signal : 40%\n         Radio type : 802.11n\n         Channel : 6\nSSID 2 : Guest\n    BSSID 1 : yy\n         Signal : 85%\n         Radio type : 802.11ac\n         Channel : 44";
        let found = parse_nearby(sample);
        assert_eq!(found[0].ssid, "Guest");
        assert_eq!(found[0].signal_percent, Some(85));
        assert_eq!(found[1].ssid, "Home");
    }
}
