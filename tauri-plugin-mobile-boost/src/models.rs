use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PingRequest {
    pub value: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PingResponse {
    pub value: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WifiInfoResponse {
    pub supported: bool,
    pub wifi_enabled: bool,
    pub connected: bool,
    pub rssi_dbm: Option<i32>,
    pub rx_link_speed_mbps: Option<i32>,
    pub tx_link_speed_mbps: Option<i32>,
    pub frequency_mhz: Option<i32>,
    pub note: String,
}

impl WifiInfoResponse {
    pub fn unsupported() -> Self {
        Self {
            supported: false,
            wifi_enabled: false,
            connected: false,
            rssi_dbm: None,
            rx_link_speed_mbps: None,
            tx_link_speed_mbps: None,
            frequency_mhz: None,
            note: "Native Wi-Fi measurements are available in the Android app only.".into(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenWifiSettingsResponse {
    pub ok: bool,
}
