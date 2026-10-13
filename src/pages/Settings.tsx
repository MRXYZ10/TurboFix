import { useCallback, useEffect, useState } from "react";
import type { Api } from "../api";
import { Card, ErrorBanner } from "../components/ui";
import type { ApiError, ScanConfig } from "../types";

export default function Settings({ api }: { api: Api }) {
  const [config, setConfig] = useState<ScanConfig | null>(null);
  const [probeCount, setProbeCount] = useState(10);
  const [probeInterval, setProbeInterval] = useState(200);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<ApiError | null>(null);
  const [saved, setSaved] = useState(false);

  const load = useCallback(async () => {
    setLoading(true); setError(null);
    try {
      const value = await api.getConfig();
      setConfig(value); setProbeCount(value.probe_count); setProbeInterval(value.probe_interval_ms);
    } catch (e) {
      const message = e && typeof e === "object" && "message" in e ? String((e as { message: unknown }).message) : String(e);
      setError({ code: "settings_load_failed", message });
    } finally { setLoading(false); }
  }, [api]);
  useEffect(() => { void load(); }, [load]);

  const save = async () => {
    if (!config) return;
    if (!Number.isFinite(probeCount) || probeCount < 1 || probeCount > 100 || !Number.isFinite(probeInterval) || probeInterval < 0 || probeInterval > 5000) {
      setError({ code: "invalid_settings", message: "Probe count must be 1–100 and interval must be 0–5000 ms." });
      return;
    }
    setSaving(true); setError(null); setSaved(false);
    try {
      const next = { ...config, probe_count: Math.round(probeCount), probe_interval_ms: Math.round(probeInterval) };
      await api.saveConfig(next); setConfig(next); setSaved(true);
    } catch (e) {
      const message = e && typeof e === "object" && "message" in e ? String((e as { message: unknown }).message) : String(e);
      setError({ code: "settings_save_failed", message });
    } finally { setSaving(false); }
  };

  return (
    <div className="space-y-4">
      <div><h1 className="text-2xl font-semibold text-slate-100">Settings</h1><p className="text-sm text-slate-400">Tune diagnostic sampling. Changes are stored locally in TurboFix.</p></div>
      {error && <ErrorBanner message={error.message} onRetry={load} />}
      {saved && <div role="status" className="rounded-lg border border-emerald-500/30 bg-emerald-500/10 p-3 text-sm text-emerald-200">Settings saved. The next connectivity scan will use these values.</div>}
      {loading ? <p className="text-sm text-slate-400">Loading settings…</p> : config && <>
        <Card title="Connectivity scan">
          <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
            <label className="block"><span className="text-sm font-medium text-slate-200">TCP probes per target</span><p className="mt-1 text-xs text-slate-500">More samples improve stability estimates but take longer (1–100).</p><input aria-label="TCP probes per target" type="number" min={1} max={100} value={probeCount} onChange={(e) => setProbeCount(Number(e.target.value))} className="mt-2 w-full rounded-lg border border-ink-600 bg-ink-900 px-3 py-2 text-slate-100" /></label>
            <label className="block"><span className="text-sm font-medium text-slate-200">Interval between probes (ms)</span><p className="mt-1 text-xs text-slate-500">0–5000 ms. Increase it if you want less bursty probing.</p><input aria-label="Interval between probes (ms)" type="number" min={0} max={5000} step={50} value={probeInterval} onChange={(e) => setProbeInterval(Number(e.target.value))} className="mt-2 w-full rounded-lg border border-ink-600 bg-ink-900 px-3 py-2 text-slate-100" /></label>
          </div>
          <button onClick={save} disabled={saving} className="mt-4 rounded-lg bg-accent px-4 py-2 text-sm font-semibold text-ink-950 disabled:opacity-50">{saving ? "Saving…" : "Save settings"}</button>
        </Card>
        <Card title="Current test endpoints">
          <p className="text-sm text-slate-400">These conservative defaults are validated by the backend and are used for connection checks. They do not store passwords or submit your browser history.</p>
          <div className="mt-3 space-y-2">{config.latency_targets.map((x) => <div key={x} className="rounded-lg border border-ink-700 bg-ink-900/50 px-3 py-2 font-mono text-sm text-slate-200">TCP · {x}</div>)}{config.dns_hosts.map((x) => <div key={x} className="rounded-lg border border-ink-700 bg-ink-900/50 px-3 py-2 font-mono text-sm text-slate-200">DNS · {x}</div>)}</div>
        </Card>
        <Card title="Privacy and safety">
          <ul className="list-disc space-y-1 pl-5 text-sm text-slate-300"><li>Scan history and speed-test results stay in the local SQLite database until you clear them. Live nearby Wi-Fi SSIDs are not saved.</li><li>A speed test transfers synthetic data to Cloudflare only after you start it.</li><li>TurboFix does not change DNS servers, edit the registry, or stop Windows services. Optional cleanup can remove only explicitly selected eligible temp files after a preview and confirmation.</li></ul>
        </Card>
      </>}
    </div>
  );
}
