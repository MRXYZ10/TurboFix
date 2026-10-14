use serde::de::DeserializeOwned;
use tauri::{AppHandle, Runtime, plugin::PluginApi};

use crate::models::*;

pub fn init<R: Runtime, C: DeserializeOwned>(
    app: &AppHandle<R>,
    _api: PluginApi<R, C>,
) -> crate::Result<MobileBoost<R>> {
    Ok(MobileBoost(app.clone()))
}

/// Access to the mobile-boost APIs.
pub struct MobileBoost<R: Runtime>(AppHandle<R>);

impl<R: Runtime> MobileBoost<R> {
    pub fn ping(&self, payload: PingRequest) -> crate::Result<PingResponse> {
        Ok(PingResponse {
            value: payload.value,
        })
    }
}
