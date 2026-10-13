import { Card, ConfidenceTag, SeverityDot } from "../components/ui";
import { fmtMs, fmtPct, fmtTime } from "../lib/format";
import type { ScanReport } from "../types";

export default function Diagnostics({ report }: { report: ScanReport | null }) {
  if (!report) {
    return (
      <div className="space-y-2">
        <h1 className="text-2xl font-semibold text-slate-100">Internet Diagnostics</h1>
        <p className="text-sm text-slate-400">No scan available. Run a diagnostic scan from the Overview page.</p>
      </div>
    );
  }
  const gw = report.gateway;
  return (
    <div className="space-y-4">
      <div>
        <h1 className="text-2xl font-semibold text-slate-100">Internet Diagnostics</h1>
        <p className="text-sm text-slate-400">Scan from {fmtTime(report.started_at)} · {report.status} · v{report.app_version} · {report.platform.os}/{report.platform.arch}</p>
      </div>

      {report.diagnosis && (
        <Card title="Findings">
          <ul className="space-y-4">
            {report.diagnosis.findings.map((f) => (
              <li key={f.id + f.evidence.join("|")}>
                <div className="flex items-center justify-between text-sm">
                  <span className="font-medium text-slate-100"><SeverityDot severity={f.severity} />{f.title}</span>
                  <ConfidenceTag value={f.confidence} />
                </div>
                <p className="mt-1 text-sm text-slate-400">{f.detail}</p>
                {f.evidence.length > 0 && (
                  <ul className="mt-1 list-disc pl-5 text-xs text-slate-500">
                    {f.evidence.map((e) => <li key={e}>{e}</li>)}
                  </ul>
                )}
              </li>
            ))}
          </ul>
        </Card>
      )}

      <Card title="Adapters and gateway">
        <p className="mb-2 text-sm text-slate-300">
          Default gateway:{" "}
          {!gw ? "not measured" : gw.state === "reachable" ? `answered in ${fmtMs(gw.rtt_ms)}` : gw.state === "inconclusive" ? "no answer on probed ports (inconclusive: many routers ignore probes)" : `unavailable (${gw.reason})`}
        </p>
        {report.interfaces.length === 0 ? (
          <p className="text-sm text-slate-500">No network interfaces were reported.</p>
        ) : (
          <table className="w-full text-left text-sm">
            <thead className="text-xs text-slate-500"><tr><th className="py-1">Adapter</th><th>Type</th><th>State</th><th>IP</th><th>Default route</th></tr></thead>
            <tbody>
              {report.interfaces.map((i) => (
                <tr key={i.name} className="border-t border-ink-700">
                  <td className="py-1">{i.name}</td><td>{i.kind}</td><td>{i.is_up ? "Up" : "Down"}</td>
                  <td>{[i.has_ipv4 && "IPv4", i.has_ipv6 && "IPv6"].filter(Boolean).join(", ") || "none"}</td>
                  <td>{i.is_default_route ? "Yes" : "No"}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </Card>

      <Card title="DNS lookups (system resolver)">
        {report.dns.length === 0 ? <p className="text-sm text-slate-500">Not measured.</p> : (
          <table className="w-full text-left text-sm">
            <thead className="text-xs text-slate-500"><tr><th className="py-1">Host</th><th>Result</th><th>Time</th></tr></thead>
            <tbody>
              {report.dns.map((d) => (
                <tr key={d.host} className="border-t border-ink-700">
                  <td className="py-1">{d.host}</td>
                  <td>{d.ok ? `${d.address_count} address(es)` : d.timed_out ? "Timed out" : `Failed: ${d.error ?? "unknown"}`}</td>
                  <td>{fmtMs(d.time_ms)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </Card>

      <Card title="Latency, jitter and loss (TCP connect)">
        {report.latency.length === 0 ? <p className="text-sm text-slate-500">Not measured.</p> : (
          <table className="w-full text-left text-sm">
            <thead className="text-xs text-slate-500"><tr><th className="py-1">Target</th><th>Median</th><th>p95</th><th>Jitter</th><th>Loss</th><th>Probes</th></tr></thead>
            <tbody>
              {report.latency.map((l) => (
                <tr key={l.target} className="border-t border-ink-700">
                  <td className="py-1">{l.target}</td><td>{fmtMs(l.stats.median_ms)}</td><td>{fmtMs(l.stats.p95_ms)}</td>
                  <td>{fmtMs(l.stats.jitter_ms)}</td><td>{fmtPct(l.stats.loss_pct, 0)}</td><td>{l.stats.received}/{l.stats.sent}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </Card>

      <Card title="HTTPS reachability">
        {report.https.length === 0 ? <p className="text-sm text-slate-500">Not measured.</p> : (
          <table className="w-full text-left text-sm">
            <thead className="text-xs text-slate-500"><tr><th className="py-1">Endpoint</th><th>Result</th><th>Time</th></tr></thead>
            <tbody>
              {report.https.map((h) => (
                <tr key={h.url} className="border-t border-ink-700">
                  <td className="py-1 break-all">{h.url}</td>
                  <td>{h.ok ? `HTTP ${h.status}` : h.timed_out ? "Timed out" : h.error ?? "Failed"}</td>
                  <td>{fmtMs(h.time_ms)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </Card>

      <Card title="Not measured, and limitations">
        <ul className="list-disc space-y-1 pl-5 text-sm text-slate-400">
          {report.unavailable.map((u) => <li key={u.name}><span className="text-slate-200">{u.name}:</span> {u.reason}</li>)}
          {report.diagnosis?.limitations.map((l) => <li key={l}>{l}</li>)}
        </ul>
        <p className="mt-3 text-xs text-slate-500">Wi-Fi link rate and internet throughput are different things. Use Wi-Fi Analysis for adapter details and Speed Test for measured download/upload; neither is part of this scan report.</p>
      </Card>
    </div>
  );
}
