
import { useState } from "react";
import type { Api } from "../api";
import { Card, ErrorBanner } from "../components/ui";
import type { ApiError, QuickFixResult, ScanReport } from "../types";

type OptimizerProps = {
  api: Api;
  onVerify: (report: ScanReport) => void;
};

export default function Optimizer({ api, onVerify }: OptimizerProps) {
  const [confirm, setConfirm] = useState(false);
  const [running, setRunning] = useState(false);
  const [verifying, setVerifying] = useState(false);
  const [result, setResult] = useState<QuickFixResult | null>(null);
  const [error, setError] = useState<ApiError | null>(null);
  const [verifyError, setVerifyError] = useState<string | null>(null);
  const [before, setBefore] = useState<ScanReport | null>(null);
  const [after, setAfter] = useState<ScanReport | null>(null);

  const apply = async () => {
    if (running) return;

    setRunning(true);
    setError(null);
    setVerifyError(null);
    setResult(null);
    setBefore(null);
    setAfter(null);

    try {
      const baseline = await api.startScan();
      setBefore(baseline);
      const response = await api.flushDnsCache();
      setResult(response);
      setConfirm(false);

      if (!response.succeeded) {
        setError({
          code: "fix_failed",
          message:
            response.message ||
            "Windows could not confirm that the DNS cache was flushed.",
        });
      } else {
        setVerifying(true);
        try {
          const verification = await api.startScan();
          setAfter(verification);
          onVerify(verification);
        } catch (e) {
          setVerifyError(e instanceof Error ? e.message : String(e));
        } finally {
          setVerifying(false);
        }
      }
    } catch (e) {
      const message =
        e && typeof e === "object" && "message" in e
          ? String((e as { message: unknown }).message)
          : String(e);

      setError({
        code: "fix_failed",
        message,
      });
    } finally {
      setRunning(false);
    }
  };

  return (
    <div className="space-y-4">
      <div>
        <h1 className="text-2xl font-semibold text-slate-100">
          Smart Optimizer
        </h1>
        <p className="text-sm text-slate-400">
          Evidence-based fixes only. No registry tweaks, hidden service
          changes, or fake “gaming boosts”.
        </p>
      </div>

      {error && <ErrorBanner message={error.message} />}

      {verifyError && <ErrorBanner message={verifyError} />}

      {result && result.succeeded && (
        <div
          role="status"
          className="rounded-lg border border-emerald-500/30 bg-emerald-500/10 p-4"
        >
          <h2 className="font-semibold text-emerald-300">
            {result.title}
          </h2>

          <p className="mt-1 text-sm text-slate-300">
            {result.message}
          </p>

          <p className="mt-2 text-xs text-slate-400">{verifying ? "Rerunning diagnostics to verify current network measurements…" : after ? "Diagnostics reran after the change. Compare measured values below; command completion does not itself prove improvement." : "DNS cache flush completed, but verification diagnostics could not be completed."}</p>
        </div>
      )}

      <Card title="Available quick fix">
        <div className="flex items-start gap-3">
          <div className="rounded-lg bg-cyan-500/10 p-3 text-xl text-cyan-300">
            ↻
          </div>

          <div className="flex-1">
            <h2 className="font-semibold text-slate-100">
              Flush Windows DNS cache
            </h2>

            <p className="mt-1 text-sm text-slate-400">
              Clears cached hostname lookups. Useful for stale or bad cached
              DNS answers; it will not increase your plan speed or repair
              weak Wi-Fi.
            </p>

            <div className="mt-3 flex flex-wrap gap-2">
              <span className="rounded border border-ink-600 px-2 py-1 text-xs text-slate-400">
                Low risk
              </span>
              <span className="rounded border border-ink-600 px-2 py-1 text-xs text-slate-400">
                Windows 10/11
              </span>
              <span className="rounded border border-ink-600 px-2 py-1 text-xs text-slate-400">
                No DNS server changes
              </span>
            </div>
          </div>
        </div>

        {!confirm ? (
          <button
            type="button"
            onClick={() => {
              setResult(null);
              setError(null);
              setVerifyError(null);
              setConfirm(true);
            }}
            disabled={running}
            className="mt-4 rounded-lg bg-accent px-4 py-2 text-sm font-semibold text-ink-950 disabled:opacity-50"
          >
            Review &amp; flush DNS cache
          </button>
        ) : (
          <div className="mt-4 rounded-lg border border-amber-500/30 bg-amber-500/5 p-3">
            <p className="text-sm text-slate-200">
              TurboFix will run Windows&apos; fixed{" "}
              <code>ipconfig /flushdns</code> command. It changes no adapter
              settings. Continue?
            </p>

            <div className="mt-3 flex flex-wrap gap-2">
              <button
                type="button"
                onClick={() => void apply()}
                disabled={running}
                className="rounded-lg bg-accent px-4 py-2 text-sm font-semibold text-ink-950 disabled:cursor-not-allowed disabled:opacity-50"
              >
                {running ? "Applying…" : "Confirm and flush"}
              </button>

              <button
                type="button"
                onClick={() => setConfirm(false)}
                disabled={running}
                className="rounded-lg border border-ink-600 px-4 py-2 text-sm hover:bg-ink-700 disabled:opacity-50"
              >
                Cancel
              </button>
            </div>
          </div>
        )}
      </Card>

      {before && <Card title="Diagnostic baseline before DNS flush">
        <p className="text-sm text-slate-300">DNS: {before.dns.filter((x) => x.time_ms !== null).map((x) => `${x.host} ${x.time_ms!.toFixed(1)} ms`).join(" · ") || "No successful DNS measurements"}</p>
        <p className="mt-1 text-sm text-slate-300">TCP latency: {before.latency.filter((x) => x.stats.median_ms !== null).map((x) => `${x.target} ${x.stats.median_ms!.toFixed(1)} ms median`).join(" · ") || "No successful TCP measurements"}</p>
        {after && <><p className="mt-3 text-sm font-medium text-cyan-200">After diagnostics</p><p className="mt-1 text-sm text-slate-300">DNS: {after.dns.filter((x) => x.time_ms !== null).map((x) => `${x.host} ${x.time_ms!.toFixed(1)} ms`).join(" · ") || "No successful DNS measurements"}</p><p className="mt-1 text-sm text-slate-300">TCP latency: {after.latency.filter((x) => x.stats.median_ms !== null).map((x) => `${x.target} ${x.stats.median_ms!.toFixed(1)} ms median`).join(" · ") || "No successful TCP measurements"}</p><p className="mt-2 text-xs text-slate-500">These are separate network samples. A DNS cache flush may change lookup timing; TCP connection timing is shown as context. Differences do not establish causation.</p></>}
      </Card>}

      <Card title="Why other tweaks are not automatic">
        <p className="text-sm text-slate-300">
          Changing adapter DNS, resetting Winsock, altering TCP registry
          values, disabling services, or bulk-deleting files can break VPNs,
          enterprise settings, or active work. TurboFix keeps those
          network/system changes disabled until it can back up settings,
          verify an adapter-specific change, and offer reliable rollback.
          Temporary-file removal is a separate preview-and-confirm action.
        </p>
      </Card>
    </div>
  );
}
