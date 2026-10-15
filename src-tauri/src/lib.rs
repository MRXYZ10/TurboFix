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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio_util::sync::CancellationToken;
use turbofix_core::cleanup::{CleanupCache, CleanupPreview, CleanupResult};
use turbofix_core::config::ScanConfig;
use turbofix_core::error::CoreError;
use turbofix_core::monitor::{
    run_monitor_session, MonitorConfig, MonitorSessionSummary, MonitorSnapshot,
};
use turbofix_core::optimizer::{recommendations, MeasurementComparison};
use turbofix_core::performance::{PerfMonitor, PerfSnapshot};
use turbofix_core::platform::{capabilities, platform_info, Capability, PlatformInfo};
use turbofix_core::scan::{run_scan, ScanReport};
use turbofix_core::speedtest::SpeedTestResult;
use turbofix_core::storage::{PerfPoint, ScanSummary, Storage};
use turbofix_core::wifi::WifiSnapshot;
use turbofix_core::ApiError;

const PERF_RECORD_EVERY: Duration = Duration::from_secs(15);
type NetworkMonitorTask = (
    CancellationToken,
    tauri::async_runtime::JoinHandle<std::result::Result<(), String>>,
);

struct AppState {
    storage: Arc<Storage>,
    monitor: Arc<Mutex<PerfMonitor>>,
    scan_token: Mutex<Option<CancellationToken>>,
    last_perf_record: Mutex<Option<Instant>>,
    speed_test_running: Mutex<bool>,
    cleanup_cache: Mutex<Option<CleanupCache>>,
    network_monitor: Mutex<Option<NetworkMonitorTask>>,
    network_snapshot: Arc<Mutex<Option<MonitorSnapshot>>>,
    network_monitor_active: Arc<AtomicBool>,
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

/// One bounded TCP connect measurement with explicit outcome. This is not ICMP
/// ping; the destination must include a port and is supplied by the user.
#[tauri::command]
async fn measure_tcp_ping(target: String) -> CmdResult<turbofix_core::network::ProbeOutcome> {
    if target.len() > 255 || !target.contains(':') {
        return Err(ApiError::from(CoreError::InvalidConfig(
            "Enter a TCP target as hostname:port or IP:port".into(),
        )));
    }
    let token = CancellationToken::new();
    Ok(turbofix_core::network::tcp_connect(&target, Duration::from_secs(3), &token).await?)
}

#[tauri::command]
fn list_speed_tests(state: State<'_, AppState>, limit: u32) -> CmdResult<Vec<SpeedTestResult>> {
    Ok(state.storage.list_speed_tests(limit)?)
}

#[tauri::command]
async fn start_network_monitor(
    app: AppHandle,
    state: State<'_, AppState>,
    config: MonitorConfig,
) -> CmdResult<MonitorSnapshot> {
    config.validate()?;
    let mut slot = state
        .network_monitor
        .lock()
        .map_err(|_| internal("network monitor lock poisoned"))?;
    if slot.is_some() || state.network_monitor_active.swap(true, Ordering::AcqRel) {
        return Err(ApiError::from(CoreError::InvalidConfig(
            "network monitoring is already running".into(),
        )));
    }
    let initial = MonitorSnapshot::new(config.clone());
    *state
        .network_snapshot
        .lock()
        .map_err(|_| internal("network snapshot lock poisoned"))? = Some(initial.clone());
    let token = CancellationToken::new();
    let task_token = token.clone();
    let snapshot = state.network_snapshot.clone();
    let storage = state.storage.clone();
    let emitter = app.clone();
    let task = tauri::async_runtime::spawn(async move {
        let callback_snapshot = snapshot.clone();
        let callback_app = emitter.clone();
        match run_monitor_session(config, task_token, move |update| {
            if let Ok(mut current) = callback_snapshot.lock() {
                *current = Some(update.clone());
            }
            let _ = callback_app.emit("network-monitor-update", update);
        })
        .await
        {
            Ok(summary) => match storage.save_monitor_session(&summary) {
                Ok(saved) => {
                    if let Ok(mut current) = snapshot.lock() {
                        if let Some(last) = current.as_mut() {
                            last.id = saved.id;
                        }
                    }
                    let _ = emitter.emit("network-monitor-session-saved", saved);
                    Ok(())
                }
                Err(error) => {
                    tracing::warn!("could not save monitor session: {error}");
                    Err(format!("could not save monitor session: {error}"))
                }
            },
            Err(error) => {
                tracing::error!("network monitor stopped with error: {error}");
                Err(error.to_string())
            }
        }
    });
    *slot = Some((token, task));
    Ok(initial)
}

#[tauri::command]
async fn stop_network_monitor(state: State<'_, AppState>) -> CmdResult<bool> {
    let active = state
        .network_monitor
        .lock()
        .map_err(|_| internal("network monitor lock poisoned"))?
        .take();
    if let Some((token, task)) = active {
        token.cancel();
        let result = task
            .await
            .map_err(|_| internal("network monitor task failed"));
        state.network_monitor_active.store(false, Ordering::Release);
        result?.map_err(|error| ApiError::from(CoreError::Storage(error)))?;
        Ok(true)
    } else {
        Ok(false)
    }
}

#[tauri::command]
fn get_network_monitor_snapshot(state: State<'_, AppState>) -> CmdResult<Option<MonitorSnapshot>> {
    Ok(state
        .network_snapshot
        .lock()
        .map_err(|_| internal("network snapshot lock poisoned"))?
        .clone())
}

#[tauri::command]
fn list_monitor_sessions(
    state: State<'_, AppState>,
    limit: u32,
) -> CmdResult<Vec<MonitorSessionSummary>> {
    Ok(state.storage.list_monitor_sessions(limit)?)
}

#[derive(Debug, Clone, Serialize)]
struct QuickFixResult {
    succeeded: bool,
    title: String,
    message: String,
    completed_at: String,
}

#[derive(Debug, Clone, Serialize)]
struct OptimizerExecution {
    result: QuickFixResult,
    after: Option<ScanReport>,
    comparisons: Vec<MeasurementComparison>,
    verification_error: Option<String>,
    history_id: Option<i64>,
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

#[tauri::command]
async fn execute_optimizer_action(
    state: State<'_, AppState>,
    action: String,
    baseline_scan_id: i64,
) -> CmdResult<OptimizerExecution> {
    let before = state.storage.get_scan(baseline_scan_id)?;
    if action != "flush_dns_cache"
        || !recommendations(&before)
            .iter()
            .any(|r| r.action.as_str() == action)
    {
        return Err(ApiError::from(CoreError::InvalidConfig(
            "action is unsupported or not recommended by the supplied diagnostic evidence".into(),
        )));
    }
    // Only the fixed allowlisted action reaches the OS. The existing implementation
    // invokes ipconfig.exe with the single constant /flushdns argument.
    let result = match flush_dns_cache() {
        Ok(result) => result,
        Err(error) => QuickFixResult {
            succeeded: false,
            title: "DNS cache flush failed".into(),
            message: error.message,
            completed_at: chrono::Utc::now().to_rfc3339(),
        },
    };
    let mut after = None;
    let mut verification_error = None;
    if result.succeeded {
        match run_scan(before.config.clone(), CancellationToken::new(), |_| {}).await {
            Ok(mut report) => {
                match state.storage.save_scan(&report) {
                    Ok(id) => report.id = Some(id),
                    Err(e) => tracing::warn!("could not save optimizer verification scan: {e}"),
                }
                after = Some(report);
            }
            Err(e) => verification_error = Some(e.to_string()),
        }
    }
    let comparisons = after
        .as_ref()
        .map(|a| turbofix_core::optimizer::compare(&before, a))
        .unwrap_or_default();
    let execution = turbofix_core::optimizer::execution_state(result.succeeded, after.is_some());
    let history_id =
        match state
            .storage
            .save_optimizer_run(&action, execution, &before, after.as_ref())
        {
            Ok(id) => Some(id),
            Err(e) => {
                tracing::error!("could not persist optimizer run: {e}");
                None
            }
        };
    Ok(OptimizerExecution {
        result,
        after,
        comparisons,
        verification_error,
        history_id,
    })
}

#[tauri::command]
fn optimizer_history(
    state: State<'_, AppState>,
    limit: u32,
) -> CmdResult<Vec<turbofix_core::storage::OptimizerRun>> {
    Ok(state.storage.optimizer_history(limit)?)
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
            let dir = app.path().app_data_dir().map_err(|error| {
                tracing::error!(%error, "startup initialization failed resolving app data directory");
                format!("cannot resolve application data directory: {error}")
            })?;
            let storage = Storage::open(&dir.join("turbofix.db"))
                .map_err(|error| {
                    tracing::error!(%error, "startup initialization failed opening local storage");
                    format!("cannot open local history database: {error}")
                })?;
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
                network_monitor: Mutex::new(None),
                network_snapshot: Arc::new(Mutex::new(None)),
                network_monitor_active: Arc::new(AtomicBool::new(false)),
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
            measure_tcp_ping,
            list_speed_tests,
            start_network_monitor,
            stop_network_monitor,
            get_network_monitor_snapshot,
            list_monitor_sessions,
            preview_temp_cleanup,
            delete_temp_files,
            execute_optimizer_action,
            optimizer_history,
            export_scan_json,
            get_mobile_wifi_info,
            open_wifi_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running TurboFix");
}
