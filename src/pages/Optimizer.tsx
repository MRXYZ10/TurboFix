import { useState } from "react";
import type { Api } from "../api";
import { Card, ErrorBanner } from "../components/ui";
import type { ApiError, OptimizerExecution, ScanReport } from "../types";

export default function Optimizer({ api, onVerify }: { api: Api; onVerify: (report: ScanReport) => void }) {
  const [preview, setPreview] = useState<ScanReport | null>(null);
  const [confirm, setConfirm] = useState(false);
  const [busy, setBusy] = useState(false);
  const [execution, setExecution] = useState<OptimizerExecution | null>(null);
  const [error, setError] = useState<ApiError | null>(null);
  const [verifyError, setVerifyError] = useState<string | null>(null);
  const recommendation = preview?.status === "completed" && preview.platform.is_windows
    ? preview.diagnosis?.findings.find((f) => ["dns_failure", "dns_partial"].includes(f.id))
    : undefined;

  const prepare = async () => {
    setBusy(true); setError(null); setExecution(null); setConfirm(false); setVerifyError(null);
    try {
      const scan = await api.startScan(); setPreview(scan);
      if (scan.status !== "completed" || !scan.platform.is_windows || !scan.diagnosis?.findings.some((f) => ["dns_failure", "dns_partial"].includes(f.id))) {
        const message = scan.status !== "completed"
          ? "The diagnostic scan did not complete, so TurboFix cannot recommend an action."
          : !scan.platform.is_windows
            ? "The DNS cache action is supported only on Windows. No optimizer action is available on this platform."
            : "This scan did not find DNS lookup failures. No optimizer action is recommended.";
        setError({ code: "no_recommendation", message });
      }
    } catch (e) { setError({ code: "preview_failed", message: e instanceof Error ? e.message : String(e) }); }
    finally { setBusy(false); }
  };

  const apply = async () => {
    if (!preview || !recommendation || busy) return;
    setBusy(true); setError(null); setVerifyError(null);
    try {
      if (preview.id === null) throw new Error("The baseline scan was not saved, so this action cannot be tied to verified evidence.");
      const result = await api.executeOptimizerAction("flush_dns_cache", preview.id);
      setExecution(result);
      if (result.after) onVerify(result.after);
      if (result.verification_error) setVerifyError(result.verification_error);
    } catch (e) { setError({ code: "fix_failed", message: e instanceof Error ? e.message : String(e) }); }
    finally { setBusy(false); setConfirm(false); }
  };

  return <div className="space-y-4">
    <div><h1 className="text-2xl font-semibold text-slate-100">Smart Optimizer</h1><p className="text-sm text-slate-400">Recommendations use diagnostic findings. Every action requires review and confirmation.</p></div>
    {error && <ErrorBanner message={error.message} />}{verifyError && <ErrorBanner message={`Action completed, but verification scan failed: ${verifyError}`} />}
    <Card title="Evidence-based recommendations">
      <p className="text-sm text-slate-300">TurboFix currently supports one low-impact Windows action: flushing the resolver cache when a scan detects DNS lookup failures. DNS failure is verified by the scan; whether a stale local cache is the cause remains a hypothesis.</p>
      {!preview && <button type="button" onClick={() => void prepare()} disabled={busy} className="mt-4 rounded-lg bg-accent px-4 py-2 text-sm font-semibold text-ink-950">{busy ? "Scanning…" : "Run scan for recommendations"}</button>}
      {preview && !recommendation && <p className="mt-3 text-sm text-slate-400">No supported action matches this scan. Verified findings: {preview.diagnosis?.findings.map((f) => f.title).join("; ") || "No diagnosis available"}.</p>}
      {preview && recommendation && <div className="mt-4 rounded-lg border border-ink-600 p-4">
        <h2 className="font-semibold text-slate-100">Flush Windows DNS resolver cache</h2>
        <p className="mt-2 text-sm text-slate-300"><b>Evidence:</b> {recommendation.evidence.join("; ") || recommendation.detail}</p>
        <p className="mt-2 text-sm text-slate-300"><b>Finding:</b> {recommendation.title} ({recommendation.confidence} confidence). DNS lookup failure is verified; stale-cache cause is a hypothesis.</p>
        <p className="mt-2 text-sm text-slate-300"><b>Expected effect:</b> May remove a stale cached DNS answer. It cannot repair an unreachable or misconfigured DNS server. No speed improvement is promised.</p>
        <p className="mt-2 text-sm text-slate-300"><b>Risk:</b> Cached names may need to be resolved again, briefly adding lookup time.</p>
        <p className="mt-1 text-sm text-slate-300"><b>Prerequisite:</b> Windows 10/11 and a completed scan with DNS lookup failures.</p>
        {!confirm ? <button type="button" onClick={() => setConfirm(true)} disabled={busy} className="mt-4 rounded-lg bg-accent px-4 py-2 text-sm font-semibold text-ink-950">Review DNS cache flush</button> : <div className="mt-4 rounded-lg border border-amber-500/30 p-3"><p className="text-sm text-slate-200">Run the fixed Windows <code>ipconfig /flushdns</code> action, then rerun equivalent diagnostics. Continue?</p><div className="mt-3 flex gap-2"><button type="button" onClick={() => void apply()} disabled={busy} className="rounded-lg bg-accent px-4 py-2 text-sm font-semibold text-ink-950">{busy ? "Applying and verifying…" : "Confirm and apply"}</button><button type="button" onClick={() => setConfirm(false)} disabled={busy} className="rounded-lg border border-ink-600 px-4 py-2 text-sm">Cancel</button></div></div>}
      </div>}
    </Card>
    {execution && <Card title="Execution and verification"><p role="status" className="text-sm text-slate-200">{execution.result.title}: {execution.result.message}</p><p className="mt-1 text-xs text-slate-400">Result: {!execution.result.succeeded ? "action failed; verification was not run" : execution.after ? "action succeeded and diagnostics reran" : "action succeeded; verification unavailable"}. History record: {execution.history_id ?? "could not be saved"}.</p>{execution.comparisons.map((m) => <p key={m.metric} className="mt-2 text-sm text-slate-300">{m.metric}: {m.before === null ? "unavailable" : `${m.before.toFixed(1)} ms`} → {m.after === null ? "unavailable" : `${m.after.toFixed(1)} ms`} — {m.result}</p>)}<p className="mt-2 text-xs text-slate-500">Measurements are separate samples. Changes can reflect unrelated network conditions and do not establish that the action caused them.</p></Card>}
    <Card title="Unsupported actions"><p className="text-sm text-slate-300">Changing DNS servers, resetting Winsock, editing TCP registry values, disabling services or security protections, and adapter resets are unavailable because safe rollback and reliable verification are not established. TurboFix never runs arbitrary shell commands or requires elevation for this action.</p></Card>
  </div>;
}
