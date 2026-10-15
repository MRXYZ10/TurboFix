import { Bar, Card, ConfidenceTag, ErrorBanner, Metric, SeverityDot, Sparkline, StatusBadge } from "../components/ui";
import { fmtBytes, fmtMs, fmtPct, fmtTime, median } from "../lib/format";
import type { ApiError, PerfSnapshot, ScanProgress, ScanReport, SpeedTestResult } from "../types";

interface Props {
  report: ScanReport | null;
  running: boolean;
  cancelling: boolean;
  progress: ScanProgress | null;
  scanError: ApiError | null;
  onScan: () => void;
  onCancel: () => void;
  perf: { snapshot: PerfSnapshot | null; cpuHistory: number[]; error: ApiError | null };
  onOpenDiagnostics: () => void;
  speedTest: SpeedTestResult | null;
  onOpenSpeedTest: () => void;
}

export default function Overview({ report, running, cancelling, progress, scanError, onScan, onCancel, perf, onOpenDiagnostics, speedTest, onOpenSpeedTest }: Props) {
  const done = report && report.status === "completed";
  const latencyMedian = done ? median(report.latency.map((l) => l.stats.median_ms)) : null;
  const jitter = done ? median(report.latency.map((l) => l.stats.jitter_ms)) : null;
  const tcpFailureValues = done ? report.latency.map((l) => l.stats.tcp_failure_pct).filter((v): v is number => v !== null) : [];
  const tcpFailurePct = tcpFailureValues.length ? Math.max(...tcpFailureValues) : null;
  const dns = done ? median(report.dns.map((d) => d.time_ms)) : null;
  const s = perf.snapshot;
  const memPct = s && s.memory_total_bytes > 0 ? (s.memory_used_bytes / s.memory_total_bytes) * 100 : null;
  const sysDisk = s?.disks.filter((d) => !d.removable).sort((a, b) => b.total_bytes - a.total_bytes)[0];
  const diskPct = sysDisk ? ((sysDisk.total_bytes - sysDisk.available_bytes) / sysDisk.total_bytes) * 100 : null;

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold text-slate-100">Overview</h1>
          <p className="text-sm text-slate-400">Measured diagnostics with explicit, confirmation-first fixes. TurboFix does not change settings silently.</p>
        </div>
        <div className="flex items-center gap-3">
          {running && (
            <button onClick={onCancel} disabled={cancelling} className="rounded-lg border border-ink-600 px-4 py-2 text-sm text-slate-200 hover:bg-ink-700 disabled:opacity-50">
              {cancelling ? "Cancelling…" : "Cancel scan"}
            </button>
          )}
          <button onClick={onScan} disabled={running} className="rounded-lg bg-accent px-5 py-2 text-sm font-semibold text-ink-950 hover:bg-cyan-300 disabled:opacity-50">
            {running ? "Scanning…" : "Run diagnostic scan"}
          </button>
        </div>
      </div>

      {running && (
        <div role="status" className="rounded-lg border border-accent/30 bg-accent/10 p-3 text-sm text-cyan-100">
          {progress ? `Step ${progress.step} of ${progress.total_steps}: ${progress.stage}` : "Starting scan…"}
        </div>
      )}
      {scanError && <ErrorBanner message={scanError.message} onRetry={running ? undefined : onScan} />}
      {report && report.status !== "completed" && (
        <ErrorBanner message={report.status === "cancelled" ? "The last scan was cancelled; partial results were saved but no diagnosis was made." : `The last scan failed: ${report.error ?? "unknown error"}`} />
      )}

      <Card title="Connectivity" right={<StatusBadge status={done ? report.diagnosis?.status ?? null : null} />}>
        {!report && <p className="text-sm text-slate-400">No scan has been run yet. Run a diagnostic scan to measure latency, DNS and reachability.</p>}
        {report && (
          <p className="text-xs text-slate-500">
            Last scan {fmtTime(report.started_at)} · {(report.duration_ms / 1000).toFixed(1)} s
          </p>
        )}
        <div className="mt-3 grid grid-cols-2 gap-3 md:grid-cols-5">
          <Metric label="Latency (median)" value={fmtMs(latencyMedian)} unavailable={latencyMedian === null} hint="TCP connect time" />
          <Metric label="Jitter" value={fmtMs(jitter)} unavailable={jitter === null} />
          <Metric label="TCP connect failures" value={tcpFailurePct === null ? "Not measured" : fmtPct(tcpFailurePct, 0)} unavailable={tcpFailurePct === null} hint="not IP packet loss" />
          <Metric label="DNS (median)" value={fmtMs(dns)} unavailable={dns === null} />
          <div className="relative"><Metric label="Download / upload" value={speedTest ? `${speedTest.download_mbps.toFixed(1)} / ${speedTest.upload_mbps.toFixed(1)} Mbps` : "Not tested"} unavailable={!speedTest} hint={speedTest ? "Download / upload · Cloudflare" : "Run a measured speed test"} /><button onClick={onOpenSpeedTest} className="mt-2 text-xs font-medium text-accent hover:underline">{speedTest ? "Run again" : "Start speed test"}</button></div>
        </div>
      </Card>

      <Card title="This PC (live)">
        {perf.error && <ErrorBanner message={perf.error.message} />}
        {!s && !perf.error && <p className="text-sm text-slate-400">Reading system metrics…</p>}
        {s && (
          <div className="grid grid-cols-1 gap-4 md:grid-cols-3">
            <div>
              <div className="mb-1 flex justify-between text-sm"><span>CPU</span><span>{fmtPct(s.cpu_pct, 0)}</span></div>
              <Bar pct={s.cpu_pct} label="CPU usage" />
              <Sparkline values={perf.cpuHistory} max={100} label="CPU history" />
            </div>
            <div>
              <div className="mb-1 flex justify-between text-sm"><span>Memory</span><span>{memPct === null ? "Unavailable" : `${fmtBytes(s.memory_used_bytes)} / ${fmtBytes(s.memory_total_bytes)}`}</span></div>
              <Bar pct={memPct ?? 0} label="Memory usage" />
            </div>
            <div>
              <div className="mb-1 flex justify-between text-sm"><span>Disk {sysDisk ? `(${sysDisk.mount})` : ""}</span><span>{sysDisk ? `${fmtBytes(sysDisk.total_bytes - sysDisk.available_bytes)} / ${fmtBytes(sysDisk.total_bytes)}` : "Unavailable"}</span></div>
              <Bar pct={diskPct ?? 0} label="Disk space used" />
              <p className="mt-1 text-xs text-slate-500">Capacity only; disk activity is not measured yet.</p>
            </div>
          </div>
        )}
      </Card>

      {done && report.diagnosis && (
        <Card title="Findings" right={<button onClick={onOpenDiagnostics} className="text-xs text-accent hover:underline">Details</button>}>
          <ul className="space-y-3">
            {report.diagnosis.findings.map((f) => (
              <li key={f.id + f.evidence.join("|")} className="text-sm">
                <div className="flex items-center justify-between">
                  <span className="font-medium text-slate-100"><SeverityDot severity={f.severity} />{f.title}</span>
                  <ConfidenceTag value={f.confidence} />
                </div>
                <p className="mt-1 text-slate-400">{f.detail}</p>
              </li>
            ))}
          </ul>
        </Card>
      )}
    </div>
  );
}
