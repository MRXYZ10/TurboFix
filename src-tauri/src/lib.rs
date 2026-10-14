//! Thin Tauri shell. All logic lives in `turbofix-core`; this file only wires
//! typed commands, shared state and progress events. No command accepts arbitrary
//! shell input; the only system-facing actions are narrowly scoped and user-confirmed.

use serde::Serialize;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
use std::path::PathBuf;
#[cfg(windows)]
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio_util::sync::CancellationToken;
use turbofix_core::cleanup::{CleanupCache, CleanupPreview, CleanupResult};
use turbofix_core::config::ScanConfig;
use turbofix_core::error::CoreError;
use turbofix_core::performance::{PerfMonitor, PerfSnapshot};
use turbofix_core::platform::{capabilities, platform_info, Capability, PlatformInfo};
use turbofix_core::scan::{run_scan, ScanReport};
use turbofix_core::speedtest::SpeedTestResult;
use turbofix_core::storage::{PerfPoint, ScanSummary, Storage};
use turbofix_core::wifi::WifiSnapshot;
use turbofix_core::ApiError;

const PERF_RECORD_EVERY: Duration = Duration::from_secs(15);

struct AppState {
    storage: Arc<Storage>,
    monitor: Arc<Mutex<PerfMonitor>>,
    scan_token: Mutex<Option<CancellationToken>>,
    last_perf_record: Mutex<Option<Instant>>,
    speed_test_running: Mutex<bool>,
    cleanup_cache: Mutex<Option<CleanupCache>>,
}

type CmdResult<T> = Result<T, ApiError>;

fn internal(msg: &str) -> ApiError {
    ApiError::from(CoreError::Internal(msg.to_string()))
}

#[tauri::command]
fn get_capabilities() -> Vec<Capability> {
    capabilities()
}

#[tauri::command]
fn get_platform() -> PlatformInfo {
    platform_info()
}

#[tauri::command]
fn get_config(state: State<'_, AppState>) -> CmdResult<ScanConfig> {
    Ok(state.storage.load_config()?)
}

#[tauri::command]
fn save_config(state: State<'_, AppState>, config: ScanConfig) -> CmdResult<()> {
    Ok(state.storage.save_config(&config)?)
}

#[tauri::command]
async fn sample_performance(state: State<'_, AppState>) -> CmdResult<PerfSnapshot> {
    let monitor = state.monitor.clone();
    let snap =
        tauri::async_runtime::spawn_blocking(move || monitor.lock().ok().map(|mut m| m.sample()))
            .await
            .map_err(|_| internal("sampling task failed"))?
            .ok_or_else(|| internal("monitor lock poisoned"))?;
    let due = {
        let mut last = state
            .last_perf_record
            .lock()
            .map_err(|_| internal("lock poisoned"))?;
        let due = last.is_none_or(|t| t.elapsed() >= PERF_RECORD_EVERY);
        if due {
            *last = Some(Instant::now());
        }
        due
    };
    if due {
        if let Err(e) = state.storage.record_perf(&snap) {
            tracing::warn!("could not record performance sample: {e}");
        }
    }
    Ok(snap)
}

#[tauri::command]
fn perf_history(state: State<'_, AppState>, limit: u32) -> CmdResult<Vec<PerfPoint>> {
    Ok(state.storage.recent_perf(limit)?)
}

#[tauri::command]
async fn start_scan(app: AppHandle, state: State<'_, AppState>) -> CmdResult<ScanReport> {
    let token = CancellationToken::new();
    {
        let mut slot = state
            .scan_token
            .lock()
            .map_err(|_| internal("lock poisoned"))?;
        if slot.is_some() {
            return Err(ApiError::from(CoreError::InvalidConfig(
                "a scan is already running".into(),
            )));
        }
        *slot = Some(token.clone());
    }
    let result = scan_inner(&app, &state, token).await;
    if let Ok(mut slot) = state.scan_token.lock() {
        *slot = None;
    }
    result
}

async fn scan_inner(
    app: &AppHandle,
    state: &State<'_, AppState>,
    token: CancellationToken,
) -> CmdResult<ScanReport> {
    let config = state.storage.load_config()?;
    let emitter = app.clone();
    let mut report = run_scan(config, token, move |p| {
        let _ = emitter.emit("scan-progress", p);
    })
    .await?;
    let monitor = state.monitor.clone();
    report.performance =
        tauri::async_runtime::spawn_blocking(move || monitor.lock().ok().map(|mut m| m.sample()))
            .await
            .ok()
            .flatten();
    let id = state.storage.save_scan(&report)?;
    report.id = Some(id);
    Ok(report)
}

#[tauri::command]
fn cancel_scan(state: State<'_, AppState>) -> bool {
    match state.scan_token.lock() {
        Ok(slot) => slot.as_ref().map(|t| t.cancel()).is_some(),
        Err(_) => false,
    }
}

#[tauri::command]
fn list_scans(state: State<'_, AppState>, limit: u32) -> CmdResult<Vec<ScanSummary>> {
    Ok(state.storage.list_scans(limit)?)
}

#[tauri::command]
fn get_scan(state: State<'_, AppState>, id: i64) -> CmdResult<ScanReport> {
    Ok(state.storage.get_scan(id)?)
}

#[tauri::command]
fn delete_scan(state: State<'_, AppState>, id: i64) -> CmdResult<()> {
    Ok(state.storage.delete_scan(id)?)
}

#[tauri::command]
fn clear_history(state: State<'_, AppState>) -> CmdResult<u64> {
    Ok(state.storage.clear_history()?)
}

#[tauri::command]
async fn wifi_snapshot() -> CmdResult<WifiSnapshot> {
    Ok(
        tauri::async_runtime::spawn_blocking(turbofix_core::wifi::snapshot)
            .await
            .map_err(|_| internal("Wi-Fi query task failed"))??,
    )
}

#[tauri::command]
async fn run_speed_test(state: State<'_, AppState>) -> CmdResult<SpeedTestResult> {
    {
        let mut running = state
            .speed_test_running
            .lock()
            .map_err(|_| internal("speed-test lock poisoned"))?;
        if *running {
            return Err(ApiError::from(CoreError::InvalidConfig(
                "a speed test is already running".into(),
            )));
        }
        *running = true;
    }
    let result = turbofix_core::speedtest::run_speed_test().await;
    if let Ok(mut running) = state.speed_test_running.lock() {
        *running = false;
    }
    let result = result?;
    Ok(state.storage.save_speed_test(&result)?)
}

#[tauri::command]
fn list_speed_tests(state: State<'_, AppState>, limit: u32) -> CmdResult<Vec<SpeedTestResult>> {
    Ok(state.storage.list_speed_tests(limit)?)
}

#[derive(Debug, Clone, Serialize)]
struct QuickFixResult {
    succeeded: bool,
    title: String,
    message: String,
    completed_at: String,
}

/// Runs one fixed, read-only-impact maintenance command. No UI text is passed to the OS.
#[tauri::command]
async fn preview_temp_cleanup(state: State<'_, AppState>) -> CmdResult<CleanupPreview> {
    let (preview, cache) =
        tauri::async_runtime::spawn_blocking(turbofix_core::cleanup::build_preview)
            .await
            .map_err(|_| internal("cleanup preview task failed"))??;
    let mut slot = state
        .cleanup_cache
        .lock()
        .map_err(|_| internal("cleanup preview lock poisoned"))?;
    *slot = Some(cache);
    Ok(preview)
}

#[tauri::command]
fn delete_temp_files(state: State<'_, AppState>, ids: Vec<String>) -> CmdResult<CleanupResult> {
    let mut slot = state
        .cleanup_cache
        .lock()
        .map_err(|_| internal("cleanup preview lock poisoned"))?;
    let cache = slot.as_mut().ok_or_else(|| {
        ApiError::from(CoreError::InvalidConfig(
            "scan temporary files before requesting deletion".into(),
        ))
    })?;
    Ok(turbofix_core::cleanup::delete_selected(cache, &ids)?)
}

#[tauri::command]
fn flush_dns_cache() -> CmdResult<QuickFixResult> {
    #[cfg(windows)]
    {
        let root = std::env::var_os("SystemRoot")
            .ok_or_else(|| internal("Windows SystemRoot environment variable is unavailable"))?;
        let executable: PathBuf = PathBuf::from(root).join("System32").join("ipconfig.exe");
        let output = Command::new(executable)
            .creation_flags(0x08000000)
            .arg("/flushdns")
            .output()
            .map_err(|e| ApiError::from(CoreError::from(e)))?;
        let completed_at = chrono::Utc::now().to_rfc3339();
        if output.status.success() {
            Ok(QuickFixResult {
                succeeded: true,
                title: "DNS cache flushed".into(),
                message: "Windows completed the DNS resolver cache flush. This can help when a stale cached lookup is the issue; it does not increase your internet plan speed.".into(),
                completed_at,
            })
        } else {
            let details = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let output_text = if details.is_empty() {
                String::from_utf8_lossy(&output.stdout).trim().to_string()
            } else {
                details
            };
            Err(ApiError::from(CoreError::Internal(format!(
                "DNS cache flush failed: {output_text}"
            ))))
        }
    }
    #[cfg(not(windows))]
    {
        Err(ApiError::from(CoreError::Internal(
            "DNS cache flushing is available only on Windows 10/11".into(),
        )))
    }
}

/// Returns the report as pretty JSON text for the UI to save. Nothing is uploaded.
#[tauri::command]
fn export_scan_json(state: State<'_, AppState>, id: i64) -> CmdResult<String> {
    let report = state.storage.get_scan(id)?;
    serde_json::to_string_pretty(&report).map_err(|e| ApiError::from(CoreError::from(e)))
}

#[tauri::command]
async fn get_mobile_wifi_info(
    app: tauri::AppHandle,
) -> std::result::Result<tauri_plugin_mobile_boost::WifiInfoResponse, String> {
    #[cfg(target_os = "android")]
    {
        use tauri_plugin_mobile_boost::MobileBoostExt;
        app.mobile_boost()
            .wifi_info()
            .map_err(|error| error.to_string())
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        Ok(tauri_plugin_mobile_boost::WifiInfoResponse::unsupported())
    }
}

#[tauri::command]
async fn open_wifi_settings(app: tauri::AppHandle) -> std::result::Result<bool, String> {
    #[cfg(target_os = "android")]
    {
        use tauri_plugin_mobile_boost::MobileBoostExt;
        app.mobile_boost()
            .open_wifi_settings()
            .map(|response| response.ok)
            .map_err(|error| error.to_string())
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        Err("Opening Android Wi-Fi settings is only supported in the Android app.".into())
    }
}
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    tauri::Builder::default()
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            let storage = Storage::open(&dir.join("turbofix.db"))
                .map_err(|e| format!("cannot open database: {e}"))?;
            // Retention: keep at most 500 scans / 90 days of scans and 7 days of performance samples.
            if let Err(e) = storage
                .prune_scans(90, 500)
                .and_then(|_| storage.prune_perf(7))
            {
                tracing::warn!("history pruning failed: {e}");
            }
            app.manage(AppState {
                storage: Arc::new(storage),
                monitor: Arc::new(Mutex::new(PerfMonitor::new())),
                scan_token: Mutex::new(None),
                last_perf_record: Mutex::new(None),
                speed_test_running: Mutex::new(false),
                cleanup_cache: Mutex::new(None),
            });
            Ok(())
        })
        .plugin(tauri_plugin_mobile_boost::init())
        .invoke_handler(tauri::generate_handler![
            get_capabilities,
            get_platform,
            get_config,
            save_config,
            sample_performance,
            perf_history,
            start_scan,
            cancel_scan,
            list_scans,
            get_scan,
            delete_scan,
            clear_history,
            wifi_snapshot,
            run_speed_test,
            list_speed_tests,
            preview_temp_cleanup,
            delete_temp_files,
            flush_dns_cache,
            export_scan_json,
            get_mobile_wifi_info,
            open_wifi_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running TurboFix");
}
