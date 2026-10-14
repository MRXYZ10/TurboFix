import { useEffect, useRef, useState } from "react";
import type { Api } from "../api";
import { Card, ErrorBanner } from "../components/ui";
import { summarizePings } from "../lib/ping";

type Probe = { at: number; ms: number | null };

export default function PingMonitor({ api }: { api: Api }) {
  const [target, setTarget] = useState("1.1.1.1:443");
  const [running, setRunning] = useState(false);
  const [probes, setProbes] = useState<Probe[]>([]);
  const [error, setError] = useState<string | null>(null);
  const active = useRef(false);

  useEffect(() => () => { active.current = false; }, []);

  const toggle = async () => {
    if (active.current) {
      active.current = false;
      setRunning(false);
      return;
    }
    const destination = target.trim();
    if (!destination.includes(":")) {
      setError("Enter a TCP target with a port, such as 1.1.1.1:443.");
      return;
    }
    active.current = true;
    setRunning(true);
    setError(null);
    setProbes([]);
    while (active.current) {
      const at = Date.now();
      try {
        const ms = await api.measureTcpPing(destination);
        if (!active.current) break;
        setProbes((items) => [...items, { at, ms }].slice(-120));
      } catch (e) {
        if (!active.current) break;
        setError(e instanceof Error ? e.message : String(e));
        setProbes((items) => [...items, { at, ms: null }].slice(-120));
      }
      await new Promise((resolve) => setTimeout(resolve, 1000));
    }
    setRunning(false);
  };

  const summary = summarizePings(probes.map((probe) => probe.ms));
  const latest = probes.length ? probes[probes.length - 1] : null;
  return <div className="space-y-4">
    <div><h1 className="text-2xl font-semibold text-slate-100">Ping Monitor</h1><p className="text-sm text-slate-400">Live TCP connection timing with one probe per second.</p></div>
    {error && <ErrorBanner message={error} />}
    <Card title="Monitor target">
      <div className="flex flex-wrap gap-3"><label className="flex-1 text-sm text-slate-400">Host and port<input aria-label="Ping target" value={target} disabled={running} onChange={(event) => setTarget(event.target.value)} className="mt-1 w-full rounded-lg border border-ink-600 bg-ink-900 px-3 py-2 font-mono text-slate-100" /></label><button onClick={() => void toggle()} className="self-end rounded-lg bg-accent px-5 py-2.5 font-semibold text-ink-950">{running ? "Stop monitor" : "Start monitor"}</button></div>
      <p className="mt-3 text-xs text-slate-500">Method: TCP connect to the selected port; this is not ICMP echo ping. DNS lookup time is excluded when using a hostname. A refused connection is counted as a failed probe.</p>
    </Card>
    <div className="grid grid-cols-2 gap-3 md:grid-cols-4">{[["Latest", latest ? (latest.ms == null ? "Failed" : `${latest.ms.toFixed(1)} ms`) : "—"], ["Average", summary.averageMs === null ? "—" : `${summary.averageMs.toFixed(1)} ms`], ["Jitter", summary.jitterMs === null ? "—" : `${summary.jitterMs.toFixed(1)} ms`], ["Failed probes", String(summary.failed)]].map(([label, value]) => <Card key={label} title={label}><div className="text-2xl font-semibold text-slate-100">{value}</div></Card>)}</div>
    <Card title="Recent probes"><div aria-live="polite" className="max-h-64 space-y-1 overflow-auto font-mono text-xs">{probes.length === 0 ? <p className="text-slate-500">{running ? "Waiting for first measurement…" : "Start monitoring to collect measurements."}</p> : [...probes].reverse().map((probe, index) => <div key={`${probe.at}-${index}`} className="flex justify-between border-b border-ink-700 py-1"><span>{new Date(probe.at).toLocaleTimeString()}</span><span className={probe.ms === null ? "text-rose-300" : "text-cyan-200"}>{probe.ms === null ? "No TCP connection" : `${probe.ms.toFixed(1)} ms`}</span></div>)}</div></Card>
  </div>;
}
