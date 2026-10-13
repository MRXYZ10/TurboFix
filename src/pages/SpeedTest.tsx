import { useState } from "react";
import type { Api } from "../api";
import { Card, ErrorBanner } from "../components/ui";
import { fmtTime } from "../lib/format";
import type { ApiError, SpeedTestResult } from "../types";

interface Props {
  api: Api;
  latest: SpeedTestResult | null;
  history: SpeedTestResult[];
  onFinish: (result: SpeedTestResult) => void;
}

function speed(value: number | null | undefined): string {
  return value == null || !Number.isFinite(value) ? "—" : `${value >= 100 ? value.toFixed(0) : value.toFixed(1)} Mbps`;
}

export default function SpeedTest({ api, latest, history, onFinish }: Props) {
  const [running, setRunning] = useState(false);
  const [error, setError] = useState<ApiError | null>(null);
  // Keep the completed result in this page too, so it is shown immediately even
  // while the parent updates overview/history state asynchronously.
  const [completedResult, setCompletedResult] = useState<SpeedTestResult | null>(null);
  const displayedResult = completedResult ?? latest;

  const run = async () => {
    setRunning(true);
    setError(null);
    try {
      const result = await api.runSpeedTest();
      setCompletedResult(result);
      onFinish(result);
    } catch (e) {
      const err = e && typeof e === "object" && "message" in e
        ? { code: "speed_test_failed", message: String((e as { message: unknown }).message) }
        : { code: "speed_test_failed", message: String(e) };
      setError(err);
    } finally {
      setRunning(false);
    }
  };

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h1 className="text-2xl font-semibold text-slate-100">Internet Speed Test</h1>
          <p className="text-sm text-slate-400">Measure real download and upload throughput from this PC.</p>
        </div>
        <button onClick={run} disabled={running} className="rounded-lg bg-accent px-5 py-2.5 text-sm font-semibold text-ink-950 hover:bg-cyan-300 disabled:opacity-50">
          {running ? "Testing connection…" : "Run speed test"}
        </button>
      </div>

      {running && <div role="status" className="rounded-lg border border-accent/30 bg-accent/10 p-3 text-sm text-cyan-100">Testing download and upload. Keep other large downloads paused for a more representative result.</div>}
      {error && <ErrorBanner message={error.message} onRetry={run} />}

      <div className="grid grid-cols-1 gap-4 md:grid-cols-3">
        <Card title="Download">
          <div className="text-3xl font-semibold text-slate-100">{speed(displayedResult?.download_mbps)}</div>
          <p className="mt-1 text-xs text-slate-500">Data received by this PC</p>
        </Card>
        <Card title="Upload">
          <div className="text-3xl font-semibold text-slate-100">{speed(displayedResult?.upload_mbps)}</div>
          <p className="mt-1 text-xs text-slate-500">Synthetic test data sent</p>
        </Card>
        <Card title="Last test">
          <div className="text-xl font-semibold text-slate-100">{displayedResult ? `${(displayedResult.duration_ms / 1000).toFixed(1)} s` : "Not run"}</div>
          <p className="mt-1 text-xs text-slate-500">{displayedResult ? fmtTime(displayedResult.started_at) : "Your results appear here"}</p>
        </Card>
      </div>

      <Card title="How this test works">
        <ul className="list-disc space-y-1 pl-5 text-sm text-slate-300">
          <li>Uses Cloudflare's public speed-test edge over HTTPS.</li>
          <li>Uses a 1 KB warm-up request, measures 5 MB of download data, and uploads 1 MB of synthetic data per test.</li>
          <li>TurboFix stores the measurements locally; Cloudflare may process request measurements to provide its service.</li>
          <li>Results vary with server location, Wi-Fi conditions, ISP congestion and other active traffic. Run twice if a result looks unusual.</li>
        </ul>
        <p className="mt-3 text-xs text-slate-500">This measures throughput, not your ISP's advertised maximum. A speed test can temporarily use bandwidth while it runs.</p>
      </Card>

      <Card title="Previous tests">
        {history.length === 0 ? <p className="text-sm text-slate-500">No saved speed tests yet.</p> : (
          <div className="overflow-x-auto">
            <table className="w-full text-left text-sm">
              <thead className="text-xs text-slate-500"><tr><th className="py-2 pr-4">When</th><th className="pr-4">Download</th><th className="pr-4">Upload</th><th>Duration</th></tr></thead>
              <tbody>{history.map((h, i) => (
                <tr key={h.id ?? `${h.started_at}-${i}`} className="border-t border-ink-700">
                  <td className="py-2 pr-4">{fmtTime(h.started_at)}</td>
                  <td className="pr-4">{speed(h.download_mbps)}</td>
                  <td className="pr-4">{speed(h.upload_mbps)}</td>
                  <td>{(h.duration_ms / 1000).toFixed(1)} s</td>
                </tr>
              ))}</tbody>
            </table>
          </div>
        )}
      </Card>
    </div>
  );
}
