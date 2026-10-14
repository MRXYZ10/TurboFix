use serde::de::DeserializeOwned;
use tauri::{
    AppHandle, Runtime,
    plugin::{PluginApi, PluginHandle},
};

use crate::models::*;

#[cfg(target_os = "ios")]
tauri::ios_plugin_binding!(init_plugin_mobile_boost);

// initializes the Kotlin or Swift plugin classes
pub fn init<R: Runtime, C: DeserializeOwned>(
    _app: &AppHandle<R>,
    api: PluginApi<R, C>,
) -> crate::Result<MobileBoost<R>> {
    #[cfg(target_os = "android")]
    let handle = api.register_android_plugin("com.plugin.mobile_boost", "ExamplePlugin")?;
    #[cfg(target_os = "ios")]
    let handle = api.register_ios_plugin(init_plugin_mobile_boost)?;
    Ok(MobileBoost(handle))
}

/// Access to the mobile-boost APIs.
pub struct MobileBoost<R: Runtime>(PluginHandle<R>);

impl<R: Runtime> MobileBoost<R> {
    pub fn ping(&self, payload: PingRequest) -> crate::Result<PingResponse> {
        self.0
            .run_mobile_plugin("ping", payload)
            .map_err(Into::into)
    }

    pub fn wifi_info(&self) -> crate::Result<WifiInfoResponse> {
        self.0.run_mobile_plugin("wifiInfo", ()).map_err(Into::into)
    }

    pub fn open_wifi_settings(&self) -> crate::Result<OpenWifiSettingsResponse> {
        self.0
            .run_mobile_plugin("openWifiSettings", ())
            .map_err(Into::into)
    }
}
