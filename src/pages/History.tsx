import { useCallback, useEffect, useState } from "react";
import type { Api } from "../api";
import { toApiError } from "../api";
import { Card, ErrorBanner, StatusBadge } from "../components/ui";
import { fmtMs, fmtTime } from "../lib/format";
import type { ApiError, MonitorSessionSummary, ScanSummary, SpeedTestResult } from "../types";

interface Props {
  api: Api;
  version: number;
  onOpen: (id: number) => void;
  onChanged: () => void;
  speedTests: SpeedTestResult[];
}

export default function History({ api, version, onOpen, onChanged, speedTests }: Props) {
  const [scans, setScans] = useState<ScanSummary[] | null>(null);
  const [monitorSessions, setMonitorSessions] = useState<MonitorSessionSummary[]>([]);
  const [error, setError] = useState<ApiError | null>(null);
  const [confirmId, setConfirmId] = useState<number | null>(null);
  const [confirmClear, setConfirmClear] = useState(false);

  const load = useCallback(async () => {
    try {
      const [nextScans, nextMonitorSessions] = await Promise.all([
        api.listScans(100),
        api.listMonitorSessions(100),
      ]);
      setScans(nextScans);
      setMonitorSessions(nextMonitorSessions);
      setError(null);
    } catch (e) {
      setError(toApiError(e));
    }
  }, [api]);

  useEffect(() => {
    void load();
  }, [load, version]);

  const run = async (fn: () => Promise<unknown>) => {
    try {
      await fn();
      await load();
      onChanged();
    } catch (e) {
      setError(toApiError(e));
    }
  };

  const exportJson = async (id: number) => {
    try {
      const text = await api.exportScanJson(id);
      const url = URL.createObjectURL(new Blob([text], { type: "application/json" }));
      const a = document.createElement("a");
      a.href = url;
      a.download = `turbofix-scan-${id}.json`;
      a.click();
      URL.revokeObjectURL(url);
    } catch (e) {
      setError(toApiError(e));
    }
  };

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold text-slate-100">Reports &amp; History</h1>
          <p className="text-sm text-slate-400">History stays on this PC. Reports omit your adapter's IP/MAC addresses, but include fixed diagnostic target hosts and URLs; exported JSON is never uploaded by TurboFix.</p>
        </div>
        {scans && (scans.length > 0 || speedTests.length > 0 || monitorSessions.length > 0) && !confirmClear && (
          <button onClick={() => setConfirmClear(true)} className="rounded border border-ink-600 px-3 py-1 text-sm hover:bg-ink-700">Clear all history…</button>
        )}
      </div>
      {confirmClear && (
        <div role="alertdialog" aria-label="Confirm clear history" className="rounded-lg border border-amber-500/30 bg-amber-500/10 p-3 text-sm">
          <p>Permanently delete saved scans, performance samples, speed tests and network monitor sessions? This cannot be undone.</p>
          <div className="mt-2 flex gap-2">
            <button onClick={() => { setConfirmClear(false); void run(() => api.clearHistory()); }} className="rounded bg-red-500 px-3 py-1 text-white">Delete everything</button>
            <button onClick={() => setConfirmClear(false)} className="rounded border border-ink-600 px-3 py-1">Keep</button>
          </div>
        </div>
      )}
      {error && <ErrorBanner message={error.message} onRetry={load} />}
      <Card>
        {!scans && !error && <p className="text-sm text-slate-400">Loading…</p>}
        {scans && scans.length === 0 && <p className="text-sm text-slate-400">No scans saved yet.</p>}
        {scans && scans.length > 0 && (
          <table className="w-full text-left text-sm">
            <thead className="text-xs text-slate-500"><tr><th className="py-1">Time</th><th>Result</th><th>Latency</th><th>TCP failures</th><th>DNS</th><th>Findings</th><th /></tr></thead>
            <tbody>
              {scans.map((s) => (
                <tr key={s.id} className="border-t border-ink-700">
                  <td className="py-2">{fmtTime(s.started_at)}</td>
                  <td>{s.status === "completed" ? <StatusBadge status={s.overall} /> : <span className="text-xs text-slate-400">{s.status}</span>}</td>
                  <td>{fmtMs(s.median_latency_ms)}</td><td>Unavailable</td><td>{fmtMs(s.median_dns_ms)}</td><td>{s.finding_count}</td>
                  <td className="space-x-2 text-right text-xs">
                    {confirmId === s.id ? (
                      <>
                        <span>Delete this scan?</span>
                        <button onClick={() => { setConfirmId(null); void run(() => api.deleteScan(s.id)); }} className="text-red-300 hover:underline">Yes, delete</button>
                        <button onClick={() => setConfirmId(null)} className="hover:underline">Cancel</button>
                      </>
                    ) : (
                      <>
                        <button onClick={() => onOpen(s.id)} className="text-accent hover:underline">Open</button>
                        <button onClick={() => void exportJson(s.id)} className="hover:underline">Export JSON</button>
                        <button onClick={() => setConfirmId(s.id)} className="text-slate-400 hover:underline">Delete</button>
                      </>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </Card>
      <Card title="Saved network monitor sessions">
        {monitorSessions.length === 0 ? <p className="text-sm text-slate-400">No completed network monitor sessions saved yet.</p> : (
          <div className="overflow-x-auto"><table className="w-full text-left text-sm"><thead className="text-xs text-slate-500"><tr><th className="py-2 pr-3">Started</th><th className="pr-3">Duration</th><th className="pr-3">Targets</th><th className="pr-3">Attempts</th><th className="pr-3">Successful</th><th>Failed</th></tr></thead><tbody>{monitorSessions.map((session) => <tr key={session.id ?? session.started_at} className="border-t border-ink-700"><td className="py-2 pr-3">{fmtTime(session.started_at)}</td><td className="pr-3">{(session.duration_ms / 1000).toFixed(1)} s</td><td className="pr-3">{session.targets.length}</td><td className="pr-3">{session.total_attempts}</td><td className="pr-3">{session.successful_attempts}</td><td>{session.failed_attempts}</td></tr>)}</tbody></table></div>
        )}
      </Card>
      <Card title="Saved speed tests">
        {speedTests.length === 0 ? <p className="text-sm text-slate-400">No speed tests saved yet. Run one from the Speed Test page.</p> : (
          <div className="overflow-x-auto"><table className="w-full text-left text-sm"><thead className="text-xs text-slate-500"><tr><th className="py-2 pr-3">Time</th><th className="pr-3">Download</th><th className="pr-3">Upload</th><th>Duration</th></tr></thead><tbody>{speedTests.map((s, i) => <tr key={s.id ?? `${s.started_at}-${i}`} className="border-t border-ink-700"><td className="py-2 pr-3">{fmtTime(s.started_at)}</td><td className="pr-3">{s.download_mbps.toFixed(1)} Mbps</td><td className="pr-3">{s.upload_mbps.toFixed(1)} Mbps</td><td>{(s.duration_ms / 1000).toFixed(1)} s</td></tr>)}</tbody></table></div>
        )}
        <p className="mt-2 text-xs text-slate-500">Only the 10 most recent results are shown here; TurboFix retains up to 100 locally.</p>
      </Card>
    </div>
  );
}
