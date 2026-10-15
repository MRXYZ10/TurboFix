import { vi } from "vitest";
import type { Api } from "../api";
import type { PerfSnapshot, ScanReport, ScanSummary, SpeedTestResult, ScanConfig, WifiSnapshot, CleanupPreview, CleanupResult } from "../types";

export function perfSnapshot(over: Partial<PerfSnapshot> = {}): PerfSnapshot {
  return {
    timestamp: "2026-01-01T00:00:00Z", os_name: "Windows", os_version: "11", cpu_count: 8, cpu_pct: 12,
    memory_total_bytes: 16 * 1024 ** 3, memory_used_bytes: 8 * 1024 ** 3, memory_available_bytes: 8 * 1024 ** 3,
    swap_total_bytes: 0, swap_used_bytes: 0,
    disks: [{ name: "C:", mount: "C:\\", file_system: "NTFS", total_bytes: 500 * 1024 ** 3, available_bytes: 250 * 1024 ** 3, removable: false }],
    process_count: 100, top_cpu: [{ pid: 1, name: "busy.exe", cpu_pct: 40, memory_bytes: 1000 }], top_memory: [],
    net_rx_bytes_per_s: null, net_tx_bytes_per_s: null, ...over,
  };
}

export function report(over: Partial<ScanReport> = {}): ScanReport {
  return {
    id: 1, app_version: "0.3.0", started_at: "2026-01-01T00:00:00Z", duration_ms: 3000, status: "completed", error: null,
    platform: { os: "windows", arch: "x86_64", is_windows: true }, interfaces: [], gateway: { state: "reachable", rtt_ms: 2 },
    dns: [{ host: "a.example", ok: true, time_ms: 20, address_count: 1, timed_out: false, error: null }],
    https: [],
    latency: [{ target: "1.1.1.1:443", samples: [20, 22], outcomes: [{ outcome: "connected", latency_ms: 20 }, { outcome: "connected", latency_ms: 22 }], last_error: null, stats: { sent: 2, received: 2, loss_pct: null, connected: 2, refused: 0, timed_out: 0, resolution_timeouts: 0, connection_timeouts: 0, resolution_failed: 0, transport_errors: 0, tcp_failure_pct: 0, min_ms: 20, median_ms: 21, p95_ms: 22, max_ms: 22, mean_ms: 21, jitter_ms: 2 } }],
    diagnosis: { status: "online", findings: [{ id: "healthy", severity: "info", confidence: "medium", title: "No problems detected in these tests", detail: "ok", evidence: [] }], limitations: ["Throughput not measured"] },
    performance: null, unavailable: [{ name: "Download / upload speed", reason: "Throughput is measured separately in Speed Test." }], ...over,
  };
}


export function speedTest(over: Partial<SpeedTestResult> = {}): SpeedTestResult {
  return { id: 1, started_at: "2026-01-01T00:00:00Z", duration_ms: 4200, download_mbps: 82.5, upload_mbps: 14.2, download_bytes: 5_000_000, upload_bytes: 1_000_000, download_duration_ms: 410, upload_duration_ms: 560, endpoint_label: "Cloudflare speed-test edge", ...over };
}

export function scanConfig(over: Partial<ScanConfig> = {}): ScanConfig {
  return { dns_hosts: ["www.cloudflare.com", "www.google.com"], latency_targets: ["1.1.1.1:443", "8.8.8.8:443"], https_endpoints: ["https://www.cloudflare.com/"], probe_count: 10, probe_interval_ms: 200, probe_timeout_ms: 2000, dns_timeout_ms: 3000, https_timeout_ms: 5000, ...over };
}

export function wifiSnapshot(over: Partial<WifiSnapshot> = {}): WifiSnapshot {
  return { supported: true, connected: true, ssid: "Lab Wi-Fi", signal_percent: 82, channel: 44, receive_mbps: 866, transmit_mbps: 780, radio_type: "802.11ac", band_hint: "5 GHz (channel-based estimate)", nearby_networks: [{ ssid: "Lab Wi-Fi", signal_percent: 82, channel: 44, radio_type: "802.11ac" }], checked_at: "2026-01-01T00:00:00Z", note: "Read-only diagnostic snapshot.", ...over };
}


export function cleanupPreview(over: Partial<CleanupPreview> = {}): CleanupPreview {
  return { root_label: "Current user's temporary folder", generated_at: "2026-01-01T00:00:00Z", min_age_days: 7, files_found: 1, total_bytes: 1024, truncated: false, items: [{ id: "temp-000", name: "old-cache.tmp", size_bytes: 1024, modified_at: "2025-12-01T00:00:00Z" }], note: "Preview only.", ...over };
}

export function cleanupResult(over: Partial<CleanupResult> = {}): CleanupResult {
  return { deleted_count: 1, deleted_bytes: 1024, skipped_count: 0, completed_at: "2026-01-01T00:00:00Z", message: "No files in use.", ...over };
}

export function summary(id: number): ScanSummary {
  return { id, started_at: "2026-01-01T00:00:00Z", duration_ms: 1000, status: "completed", overall: "online", finding_count: 1, median_latency_ms: 20, loss_pct: 0, median_dns_ms: 15 };
}

export function mockApi(over: Partial<Api> = {}): Api {
  return {
    capabilities: vi.fn().mockResolvedValue([]),
    getConfig: vi.fn().mockResolvedValue(scanConfig()),
    saveConfig: vi.fn().mockResolvedValue(undefined),
    wifiSnapshot: vi.fn().mockResolvedValue(wifiSnapshot()),
    runSpeedTest: vi.fn().mockResolvedValue(speedTest()),
    measureTcpPing: vi.fn().mockResolvedValue({ outcome: "connected", latency_ms: 12.5 }),
    startNetworkMonitor: vi.fn().mockResolvedValue(null),
    stopNetworkMonitor: vi.fn().mockResolvedValue(true),
    getNetworkMonitorSnapshot: vi.fn().mockResolvedValue(null),
    listMonitorSessions: vi.fn().mockResolvedValue([]),
    onNetworkMonitorUpdate: vi.fn().mockResolvedValue(() => undefined),
    listSpeedTests: vi.fn().mockResolvedValue([]),
    executeOptimizerAction: vi.fn().mockResolvedValue({ result: { succeeded: true, title: "DNS cache flushed", message: "Cache flushed.", completed_at: "2026-01-01T00:00:00Z" }, after: report(), comparisons: [{ metric: "median_dns_ms", before: 20, after: 10, delta: -10, result: "lower after measurement; change alone does not establish cause" }], verification_error: null, history_id: 1 }),
    previewTempCleanup: vi.fn().mockResolvedValue(cleanupPreview()),
    deleteTempFiles: vi.fn().mockResolvedValue(cleanupResult()),
    startScan: vi.fn().mockResolvedValue(report()),
    cancelScan: vi.fn().mockResolvedValue(true),
    samplePerformance: vi.fn().mockResolvedValue(perfSnapshot()),
    perfHistory: vi.fn().mockResolvedValue([]),
    listScans: vi.fn().mockResolvedValue([]),
    getScan: vi.fn().mockResolvedValue(report()),
    deleteScan: vi.fn().mockResolvedValue(undefined),
    clearHistory: vi.fn().mockResolvedValue(0),
    exportScanJson: vi.fn().mockResolvedValue("{}"),
    onScanProgress: vi.fn().mockResolvedValue(() => undefined),
    ...over,
  } as Api;
}
