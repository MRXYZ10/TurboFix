import { invoke as invokeMobileCommand } from "@tauri-apps/api/core";
// The only module that talks to Rust. Every function maps to one typed Tauri command.
// There is intentionally no mock fallback: outside the desktop shell calls fail with
// a clear error rather than returning invented data.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { ApiError, Capability, PerfPoint, PerfSnapshot, ProbeOutcome, ScanConfig, ScanProgress, ScanReport, ScanSummary, SpeedTestResult, WifiSnapshot, CleanupPreview, CleanupResult, MonitorConfig, MonitorSnapshot, MonitorSessionSummary, OptimizerExecution } from "./types";

export function toApiError(e: unknown): ApiError {
  if (e && typeof e === "object" && "code" in e && "message" in e) {
    const o = e as { code: unknown; message: unknown };
    return { code: String(o.code), message: String(o.message) };
  }
  const text = e instanceof Error ? e.message : String(e);
  if (/__TAURI|invoke|undefined/.test(text)) {
    return { code: "no_backend", message: "TurboFix backend is not available. Run the app with `npm run tauri dev` instead of a plain browser." };
  }
  return { code: "unknown", message: text };
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    throw toApiError(e);
  }
}

export const api = {
  capabilities: () => call<Capability[]>("get_capabilities"),
  getConfig: () => call<ScanConfig>("get_config"),
  saveConfig: (config: ScanConfig) => call<void>("save_config", { config }),
  wifiSnapshot: () => call<WifiSnapshot>("wifi_snapshot"),
  runSpeedTest: () => call<SpeedTestResult>("run_speed_test"),
  measureTcpPing: (target: string) => call<ProbeOutcome>("measure_tcp_ping", { target }),
  startNetworkMonitor: (config: MonitorConfig) => call<MonitorSnapshot>("start_network_monitor", { config }),
  stopNetworkMonitor: () => call<boolean>("stop_network_monitor"),
  getNetworkMonitorSnapshot: () => call<MonitorSnapshot | null>("get_network_monitor_snapshot"),
  listMonitorSessions: (limit: number) => call<MonitorSessionSummary[]>("list_monitor_sessions", { limit }),
  onNetworkMonitorUpdate: (cb: (snapshot: MonitorSnapshot) => void) => listen<MonitorSnapshot>("network-monitor-update", (e) => cb(e.payload)),
  listSpeedTests: (limit: number) => call<SpeedTestResult[]>("list_speed_tests", { limit }),
  executeOptimizerAction: (action: string, baselineScanId: number) => call<OptimizerExecution>("execute_optimizer_action", { action, baselineScanId }),
  previewTempCleanup: () => call<CleanupPreview>("preview_temp_cleanup"),
  deleteTempFiles: (ids: string[]) => call<CleanupResult>("delete_temp_files", { ids }),
  startScan: () => call<ScanReport>("start_scan"),
  cancelScan: () => call<boolean>("cancel_scan"),
  samplePerformance: () => call<PerfSnapshot>("sample_performance"),
  perfHistory: (limit: number) => call<PerfPoint[]>("perf_history", { limit }),
  listScans: (limit: number) => call<ScanSummary[]>("list_scans", { limit }),
  getScan: (id: number) => call<ScanReport>("get_scan", { id }),
  deleteScan: (id: number) => call<void>("delete_scan", { id }),
  clearHistory: () => call<number>("clear_history"),
  exportScanJson: (id: number) => call<string>("export_scan_json", { id }),
  onScanProgress: (cb: (p: ScanProgress) => void) => listen<ScanProgress>("scan-progress", (e) => cb(e.payload)),
};

export type Api = typeof api;

// Android-only native measurements. The backend returns an explicit unsupported
// result on desktop rather than inventing Wi-Fi data.
export interface MobileWifiInfo {
  supported: boolean;
  wifiEnabled: boolean;
  connected: boolean;
  rssiDbm: number | null;
  rxLinkSpeedMbps: number | null;
  txLinkSpeedMbps: number | null;
  frequencyMhz: number | null;
  note: string;
}

export const mobileBoostApi = {
  wifiInfo: () => invokeMobileCommand<MobileWifiInfo>("get_mobile_wifi_info"),
  openWifiSettings: () => invokeMobileCommand<boolean>("open_wifi_settings"),
};
