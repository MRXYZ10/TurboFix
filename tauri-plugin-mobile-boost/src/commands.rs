use tauri::{AppHandle, Runtime, command};

use crate::MobileBoostExt;
use crate::Result;
use crate::models::*;

#[command]
pub(crate) async fn ping<R: Runtime>(
    app: AppHandle<R>,
    payload: PingRequest,
) -> Result<PingResponse> {
    app.mobile_boost().ping(payload)
}
