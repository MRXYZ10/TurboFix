import { useEffect, useState } from "react";
import type { Api } from "../api";
import { Card, ErrorBanner } from "../components/ui";
import type { MonitorConfig, MonitorSessionSummary, MonitorSnapshot, ProbeOutcome, TargetMonitorSnapshot } from "../types";

const initialTargets = "1.1.1.1:443\n8.8.8.8:443";

function keepNewestSnapshot(current: MonitorSnapshot | null, next: MonitorSnapshot | null): MonitorSnapshot | null {
  if (!next) return current;
  if (!current) return next;
  const currentAt = Date.parse(current.last_updated_at);
  const nextAt = Date.parse(next.last_updated_at);
  return Number.isNaN(currentAt) || nextAt >= currentAt ? next : current;
}

function outcomeLabel(outcome: ProbeOutcome): string {
  switch (outcome.outcome) {
    case "connected": return `Connected · ${outcome.latency_ms.toFixed(1)} ms TCP-connect latency`;
    case "connection_refused": return `Connection refused · ${outcome.latency_ms.toFixed(1)} ms`;
    case "timeout": return `${outcome.stage === "resolution" ? "DNS resolution" : "TCP connection"} timed out`;
    case "resolution_failed": return `DNS resolution failed · ${outcome.message}`;
    case "transport_error": return `Transport error · ${outcome.message}`;
  }
}

function statusLabel(target: TargetMonitorSnapshot): string {
  const result = target.last_result?.outcome;
  if (!result) return "Waiting for first result";
  switch (result.outcome) {
    case "connected": return "Connected";
    case "connection_refused": return "Port refused connection";
    case "timeout": return result.stage === "resolution" ? "DNS resolution timeout" : "Connection timeout";
    case "resolution_failed": return "Host could not be resolved";
    case "transport_error": return "Transport error";
  }
}

function LatencyChart({ target }: { target: TargetMonitorSnapshot }) {
  const points = target.latency_history;
  if (points.length < 2) return <div className="flex h-20 items-center justify-center text-xs text-slate-500">At least two successful connections are needed to draw a trend.</div>;
  const vals = points.map((p) => p.latency_ms);
  const min = Math.min(...vals), max = Math.max(...vals), range = Math.max(max - min, 1);
  const path = vals.map((value, index) => `${index ? "L" : "M"} ${(index / (vals.length - 1)) * 100} ${36 - ((value - min) / range) * 30}`).join(" ");
  return <svg role="img" aria-label={`Successful TCP-connect latency history for ${target.target}`} viewBox="0 0 100 40" preserveAspectRatio="none" className="h-20 w-full overflow-visible"><path d={path} fill="none" stroke="currentColor" strokeWidth="1.5" vectorEffect="non-scaling-stroke" className="text-cyan-300" /></svg>;
}

function TargetCard({ target }: { target: TargetMonitorSnapshot }) {
  const tone = target.last_result?.outcome.outcome === "connected" ? "text-emerald-300" : target.last_result ? "text-amber-300" : "text-slate-400";
  return <Card title={target.target}>
    <div className="flex flex-wrap items-start justify-between gap-2"><div><p className={`text-sm font-semibold ${tone}`}>{statusLabel(target)}</p><p className="mt-1 text-xs text-slate-400">Latest successful TCP-connect latency: {target.latest_success_latency_ms === null ? "Not available" : `${target.latest_success_latency_ms.toFixed(1)} ms`}</p></div><div className="text-right text-xs text-slate-400"><div>{target.attempts} attempts · {target.successful_attempts} successful · {target.failed_attempts} failed</div><div>Jitter: {target.jitter_ms === null ? "Not available yet" : `${target.jitter_ms.toFixed(1)} ms`}</div></div></div>
    <div className="mt-3 grid grid-cols-2 gap-2 text-xs text-slate-400 md:grid-cols-5"><span>Refused: {target.connection_refusals}</span><span>DNS failures: {target.resolution_failures}</span><span>DNS timeouts: {target.resolution_timeouts}</span><span>Connect timeouts: {target.connection_timeouts}</span><span>Other errors: {target.transport_errors}</span></div>
    <p className="mt-2 text-xs text-slate-500">IP packet loss: Not measured</p>
    <div className="mt-3 border-t border-ink-700 pt-2"><LatencyChart target={target} /></div>
    <div className="mt-2 max-h-32 space-y-1 overflow-auto font-mono text-[11px]" aria-label={`Recent results for ${target.target}`}>{[...target.recent_results].reverse().slice(0, 8).map((probe, i) => <div key={`${probe.at}-${i}`} className="flex justify-between gap-3 border-b border-ink-700/60 py-1"><time>{new Date(probe.at).toLocaleTimeString()}</time><span className="text-right">{outcomeLabel(probe.outcome)}</span></div>)}</div>
  </Card>;
}

export default function PingMonitor({ api }: { api: Api }) {
  const [targets, setTargets] = useState(initialTargets);
  const [intervalMs, setIntervalMs] = useState("1000");
  const [timeoutMs, setTimeoutMs] = useState("2000");
  const [concurrency, setConcurrency] = useState("2");
  const [snapshot, setSnapshot] = useState<MonitorSnapshot | null>(null);
  const [history, setHistory] = useState<MonitorSessionSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [now, setNow] = useState(Date.now());

  useEffect(() => {
    let alive = true;
    const unlisten = api.onNetworkMonitorUpdate((next) => { if (alive) setSnapshot((current) => keepNewestSnapshot(current, next)); });
    void Promise.all([api.getNetworkMonitorSnapshot(), api.listMonitorSessions(10)]).then(([current, sessions]) => {
      if (alive) { if (current) { setSnapshot((previous) => keepNewestSnapshot(previous, current)); setTargets(current.config.targets.join("\n")); setIntervalMs(String(current.config.interval_ms)); setTimeoutMs(String(current.config.timeout_ms)); setConcurrency(String(current.config.max_concurrency)); } setHistory(sessions); }
    }).catch((e: unknown) => { if (alive) setError(e instanceof Error ? e.message : String(e)); }).finally(() => { if (alive) setLoading(false); });
    return () => { alive = false; void unlisten.then((off) => off()); };
  }, [api]);

  useEffect(() => {
    if (!snapshot?.running) return;
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, [snapshot?.running]);

  const start = async () => {
    setBusy(true); setError(null);
    const config: MonitorConfig = { targets: targets.split(/\r?\n/).map((x) => x.trim()).filter(Boolean), interval_ms: Number(intervalMs), timeout_ms: Number(timeoutMs), max_concurrency: Number(concurrency) };
    try { const current = await api.startNetworkMonitor(config); setSnapshot((previous) => keepNewestSnapshot(previous, current)); }
    catch (e) { setError(e instanceof Error ? e.message : String(e)); }
    finally { setBusy(false); }
  };
  const stop = async () => {
    setBusy(true); setError(null);
    try {
      let stopError: unknown;
      try { await api.stopNetworkMonitor(); }
      catch (e) { stopError = e; }
      try {
        const current = await api.getNetworkMonitorSnapshot();
        setSnapshot((previous) => keepNewestSnapshot(previous, current));
        setHistory(await api.listMonitorSessions(10));
      } catch (e) {
        stopError ??= e;
      }
      if (stopError) setError(stopError instanceof Error ? stopError.message : String(stopError));
    } finally { setBusy(false); }
  };

  const running = snapshot?.running === true;
  const elapsedSeconds = Math.floor(Math.max(snapshot?.elapsed_ms ?? 0, snapshot?.running ? now - Date.parse(snapshot.started_at) : 0) / 1000);
  return <div className="space-y-4">
    <header><h1 className="text-2xl font-semibold text-slate-100">Pro Network Monitor</h1><p className="text-sm text-slate-400">Repeated TCP connection checks across multiple targets, with bounded concurrency and local session history.</p></header>
    {error && <ErrorBanner message={error} />}
    <Card title="Monitor configuration">
      <div className="grid gap-3 md:grid-cols-2"><label className="text-sm text-slate-400">Targets (one host:port per line)<textarea aria-label="Monitor targets" value={targets} disabled={running || busy} onChange={(e) => setTargets(e.target.value)} rows={4} className="mt-1 w-full rounded-lg border border-ink-600 bg-ink-900 px-3 py-2 font-mono text-slate-100" /></label><div className="grid grid-cols-3 gap-2"><label className="text-xs text-slate-400">Interval (ms)<input aria-label="Interval in milliseconds" type="number" min="500" max="60000" value={intervalMs} disabled={running || busy} onChange={(e) => setIntervalMs(e.target.value)} className="mt-1 w-full rounded-lg border border-ink-600 bg-ink-900 px-2 py-2 text-slate-100" /></label><label className="text-xs text-slate-400">Timeout (ms)<input aria-label="Timeout in milliseconds" type="number" min="200" max="10000" value={timeoutMs} disabled={running || busy} onChange={(e) => setTimeoutMs(e.target.value)} className="mt-1 w-full rounded-lg border border-ink-600 bg-ink-900 px-2 py-2 text-slate-100" /></label><label className="text-xs text-slate-400">Concurrency<input aria-label="Maximum concurrency" type="number" min="1" max="4" value={concurrency} disabled={running || busy} onChange={(e) => setConcurrency(e.target.value)} className="mt-1 w-full rounded-lg border border-ink-600 bg-ink-900 px-2 py-2 text-slate-100" /></label></div></div>
      <div className="mt-3 flex flex-wrap items-center gap-3"><button onClick={() => void (running ? stop() : start())} disabled={busy || loading} className="rounded-lg bg-accent px-5 py-2.5 font-semibold text-ink-950 disabled:opacity-50">{busy ? "Please wait…" : running ? "Stop Monitoring" : "Start Monitoring"}</button><span aria-live="polite" className="text-sm text-slate-400">{loading ? "Loading monitor history…" : running ? `Running · ${elapsedSeconds}s · ${snapshot?.total_attempts ?? 0} attempts` : snapshot ? `Stopped · ${elapsedSeconds}s · ${snapshot.total_attempts} attempts` : "Stopped · ready to start"}</span></div>
      <p className="mt-3 text-xs text-slate-500">Measures TCP-connect latency; DNS lookup is outside successful connection timing. Jitter is the mean absolute difference between consecutive successful latency samples, skipping failed attempts. It is not an ICMP ping. IP packet loss: Not measured.</p>
      <p className="mt-1 text-xs text-slate-500">Limits: 1–8 unique targets, 500–60000 ms interval, 200–10000 ms timeout per resolution/connect stage, concurrency 1–4. No round overlaps. Stop cancels pending async probes. Only the latest 30 results and 60 successful latencies stay in memory; at most 100 session summaries are stored locally.</p>
    </Card>
    {snapshot && <div className="grid gap-3 md:grid-cols-2">{snapshot.targets.map((target) => <TargetCard key={target.target} target={target} />)}</div>}
    {snapshot && !snapshot.targets.length && <Card title="No targets"><p className="text-sm text-slate-400">Add at least one valid target to begin monitoring.</p></Card>}
    <Card title="Recent sessions"><div className="overflow-x-auto">{loading ? <p className="text-sm text-slate-400">Loading saved sessions…</p> : history.length === 0 ? <p className="text-sm text-slate-400">No completed monitor sessions saved yet.</p> : <table className="w-full text-left text-xs text-slate-300"><thead><tr className="text-slate-500"><th className="py-2">Started</th><th>Duration</th><th>Targets</th><th>Attempts</th><th>Successful</th><th>Failed</th></tr></thead><tbody>{history.map((session) => <tr key={session.id ?? session.started_at} className="border-t border-ink-700"><td className="py-2">{new Date(session.started_at).toLocaleString()}</td><td>{(session.duration_ms / 1000).toFixed(1)} s</td><td>{session.targets.length}</td><td>{session.total_attempts}</td><td>{session.successful_attempts}</td><td>{session.failed_attempts}</td></tr>)}</tbody></table>}</div></Card>
  </div>;
}
