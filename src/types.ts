// Mirrors the serde types in crates/turbofix-core. Keep in sync when Rust types change.

export type ScanStatus = "completed" | "cancelled" | "failed";
export type OverallStatus = "online" | "degraded" | "offline" | "unknown";
export type Severity = "info" | "warning" | "critical";
export type Confidence = "low" | "medium" | "high";

export interface ApiError {
  code: string;
  message: string;
}

export interface LatencyStats {
  sent: number;
  received: number;
  loss_pct: number;
  min_ms: number | null;
  median_ms: number | null;
  p95_ms: number | null;
  max_ms: number | null;
  mean_ms: number | null;
  jitter_ms: number | null;
}

export interface LatencyResult {
  target: string;
  stats: LatencyStats;
  samples: (number | null)[];
  last_error: string | null;
}

export interface DnsResult {
  host: string;
  ok: boolean;
  time_ms: number | null;
  address_count: number;
  timed_out: boolean;
  error: string | null;
}

export interface HttpsResult {
  url: string;
  ok: boolean;
  status: number | null;
  time_ms: number | null;
  timed_out: boolean;
  error: string | null;
}

export type GatewayStatus =
  | { state: "reachable"; rtt_ms: number }
  | { state: "inconclusive" }
  | { state: "unavailable"; reason: string };

export interface InterfaceInfo {
  name: string;
  kind: string;
  is_up: boolean;
  has_ipv4: boolean;
  has_ipv6: boolean;
  is_default_route: boolean;
  link_speed_mbps: number | null;
}

export interface Finding {
  id: string;
  severity: Severity;
  confidence: Confidence;
  title: string;
  detail: string;
  evidence: string[];
}

export interface Diagnosis {
  status: OverallStatus;
  findings: Finding[];
  limitations: string[];
}

export interface ProcInfo {
  pid: number;
  name: string;
  cpu_pct: number;
  memory_bytes: number;
}

export interface DiskInfo {
  name: string;
  mount: string;
  file_system: string;
  total_bytes: number;
  available_bytes: number;
  removable: boolean;
}

export interface PerfSnapshot {
  timestamp: string;
  os_name: string | null;
  os_version: string | null;
  cpu_count: number;
  cpu_pct: number;
  memory_total_bytes: number;
  memory_used_bytes: number;
  memory_available_bytes: number;
  swap_total_bytes: number;
  swap_used_bytes: number;
  disks: DiskInfo[];
  process_count: number;
  top_cpu: ProcInfo[];
  top_memory: ProcInfo[];
  net_rx_bytes_per_s: number | null;
  net_tx_bytes_per_s: number | null;
}

export interface UnavailableMetric {
  name: string;
  reason: string;
}

export interface ScanReport {
  id: number | null;
  app_version: string;
  started_at: string;
  duration_ms: number;
  status: ScanStatus;
  error: string | null;
  platform: { os: string; arch: string; is_windows: boolean };
  interfaces: InterfaceInfo[];
  gateway: GatewayStatus | null;
  dns: DnsResult[];
  https: HttpsResult[];
  latency: LatencyResult[];
  diagnosis: Diagnosis | null;
  performance: PerfSnapshot | null;
  unavailable: UnavailableMetric[];
}

export interface ScanSummary {
  id: number;
  started_at: string;
  duration_ms: number;
  status: ScanStatus;
  overall: OverallStatus | null;
  finding_count: number;
  median_latency_ms: number | null;
  loss_pct: number | null;
  median_dns_ms: number | null;
}

export interface PerfPoint {
  ts: string;
  cpu_pct: number;
  mem_used: number;
  mem_total: number;
}

export type CapabilityStatus = "available" | "planned" | "disabled";
export interface Capability {
  id: string;
  name: string;
  status: CapabilityStatus;
  note: string;
}

export interface ScanProgress {
  stage: string;
  step: number;
  total_steps: number;
}


export interface ScanConfig {
  dns_hosts: string[];
  latency_targets: string[];
  https_endpoints: string[];
  probe_count: number;
  probe_interval_ms: number;
  probe_timeout_ms: number;
  dns_timeout_ms: number;
  https_timeout_ms: number;
}

export interface SpeedTestResult {
  id: number | null;
  started_at: string;
  duration_ms: number;
  download_mbps: number;
  upload_mbps: number;
  download_bytes: number;
  upload_bytes: number;
  download_duration_ms: number;
  upload_duration_ms: number;
  endpoint_label: string;
}

export interface NearbyNetwork {
  ssid: string;
  signal_percent: number | null;
  channel: number | null;
  radio_type: string | null;
}

export interface WifiSnapshot {
  supported: boolean;
  connected: boolean;
  ssid: string | null;
  signal_percent: number | null;
  channel: number | null;
  receive_mbps: number | null;
  transmit_mbps: number | null;
  radio_type: string | null;
  band_hint: string | null;
  nearby_networks: NearbyNetwork[];
  checked_at: string;
  note: string;
}

export interface QuickFixResult {
  succeeded: boolean;
  title: string;
  message: string;
  completed_at: string;
}


export interface CleanupPreviewItem {
  id: string;
  name: string;
  size_bytes: number;
  modified_at: string;
}

export interface CleanupPreview {
  root_label: string;
  generated_at: string;
  min_age_days: number;
  files_found: number;
  total_bytes: number;
  truncated: boolean;
  items: CleanupPreviewItem[];
  note: string;
}

export interface CleanupResult {
  deleted_count: number;
  deleted_bytes: number;
  skipped_count: number;
  completed_at: string;
  message: string;
}
