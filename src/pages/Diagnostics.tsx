import type { ReactNode } from "react";
import { Card, ConfidenceTag, SeverityDot } from "../components/ui";
import { fmtMs, fmtPct, fmtTime } from "../lib/format";
import type { ScanReport } from "../types";

type MobileRow = { label: string; value: ReactNode };

function MobileDataCard({ title, rows }: { title: ReactNode; rows: MobileRow[] }) {
  return (
    <article className="mobile-data-card">
      <div className="mobile-data-title">{title}</div>
      <dl className="mobile-data-rows">
        {rows.map((row) => (
          <div className="mobile-data-row" key={row.label}>
            <dt>{row.label}</dt>
            <dd>{row.value}</dd>
          </div>
        ))}
      </dl>
    </article>
  );
}

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
    <div className="space-y-4 diagnostics-page">
      <div className="diagnostics-summary">
        <h1 className="text-2xl font-semibold text-slate-100">Internet Diagnostics</h1>
        <p className="text-sm text-slate-400">
          Scan from {fmtTime(report.started_at)} · {report.status} · v{report.app_version} · {report.platform.os}/{report.platform.arch}
        </p>
      </div>

      {report.diagnosis && (
        <Card title="Findings">
          <ul className="space-y-4">
            {report.diagnosis.findings.map((finding) => (
              <li key={finding.id + finding.evidence.join("|")}>
                <div className="finding-heading flex flex-wrap items-start justify-between gap-2 text-sm">
                  <span className="font-medium text-slate-100">
                    <SeverityDot severity={finding.severity} />{finding.title}
                  </span>
                  <ConfidenceTag value={finding.confidence} />
                </div>
                <p className="mt-1 text-sm text-slate-400">{finding.detail}</p>
                {finding.evidence.length > 0 && (
                  <ul className="mt-1 list-disc pl-5 text-xs text-slate-500">
                    {finding.evidence.map((evidence) => <li key={evidence}>{evidence}</li>)}
                  </ul>
                )}
              </li>
            ))}
          </ul>
        </Card>
      )}

      {report.diagnosis?.findings.length ? (
        <Card title="Recommended fixes">
          <ul className="space-y-4">
            {report.diagnosis.findings.map((finding) => {
              const advice: Record<string, { reason: string; steps: string[] }> = {
                no_interface: { reason: "Windows did not report an active network interface.", steps: ["Check that Wi-Fi or Ethernet is enabled.", "Reconnect to your network and scan again."] },
                external_unreachable: { reason: "External targets could not be reached; the cause may be local networking, the router, or the ISP.", steps: ["Check router connectivity and its internet/WAN status.", "Retry the scan; compare another device on the same network."] },
                gateway_silent: { reason: "The gateway did not answer the tested ports; some routers ignore these probes.", steps: ["Check whether websites work before assuming the router is faulty.", "If connectivity is actually broken, reconnect to Wi-Fi or inspect the router."] },
                no_default_route: { reason: "No default route was detected for normal internet traffic.", steps: ["Reconnect to the intended Wi-Fi or Ethernet network.", "If this persists, inspect Windows network settings or contact your administrator."] },
                dns_failure: { reason: "The system DNS resolver failed to resolve test names.", steps: ["Retry the scan and check whether multiple websites fail.", "If the problem persists, reconnect to the network or check router DNS settings."] },
                dns_partial: { reason: "Some DNS lookups failed while others succeeded.", steps: ["Retry to rule out a temporary resolver issue.", "Check whether the failures affect particular domains or services."] },
                dns_slow: { reason: "DNS responses took longer than expected.", steps: ["Retry the scan to confirm the delay.", "Compare another network before changing DNS settings."] },
                https_blocked: { reason: "One or more HTTPS test endpoints could not be reached; this does not prove all HTTPS traffic is blocked.", steps: ["Check the endpoint results and retry.", "If multiple services fail, inspect VPN, proxy, or managed-network restrictions without disabling security protections."] },
                packet_loss: { reason: "Many TCP connection probes failed; this is not a direct measurement of all network packet loss.", steps: ["Move closer to the Wi-Fi router or test Ethernet if available.", "Repeat the scan and compare results at another time."] },
                high_latency: { reason: "TCP connection response times were high; distance, congestion, VPNs, and Wi-Fi can contribute.", steps: ["Pause heavy downloads and retry.", "Compare Wi-Fi with Ethernet or another network if possible."] },
                high_jitter: { reason: "Connection response times varied substantially between probes.", steps: ["Retry when the network is less busy.", "Compare a wired connection or a closer Wi-Fi position."] },
                partial_reach: { reason: "Some test targets failed while others were reachable; a target or route may be blocking probes.", steps: ["Retry the scan to confirm the result.", "Check whether the affected service itself is unavailable."] },
                healthy: { reason: "These checks did not detect a connectivity problem; download and upload throughput are not measured here.", steps: ["Use Speed Test to measure throughput.", "Run diagnostics again if the issue returns."] },
              };
              const item = advice[finding.id];
              if (!item) return null;
              return (
                <li key={finding.id} className="text-sm">
                  <p className="font-medium text-slate-100">{finding.title}</p>
                  <p className="mt-1 text-slate-400">Why: {item.reason}</p>
                  <ul className="mt-1 list-disc space-y-1 pl-5 text-slate-400">
                    {item.steps.map((step) => <li key={step}>{step}</li>)}
                  </ul>
                </li>
              );
            })}
          </ul>
          <p className="mt-3 text-xs text-slate-500">These are suggested checks, not automatic repairs. Run diagnostics again after a change to verify whether the result improved.</p>
        </Card>
      ) : null}

      <Card title="Adapters and gateway">
        <p className="mb-3 text-sm text-slate-300">
          Default gateway: { !gw ? "not measured" : gw.state === "reachable" ? `answered in ${fmtMs(gw.rtt_ms)}` : gw.state === "inconclusive" ? "no answer on probed ports (inconclusive: many routers ignore probes)" : `unavailable (${gw.reason})` }
        </p>
        {report.interfaces.length === 0 ? (
          <p className="text-sm text-slate-500">No network interfaces were reported.</p>
        ) : (
          <>
            <div className="desktop-data-table">
              <table className="w-full text-left text-sm">
                <thead className="text-xs text-slate-500"><tr><th className="py-2 pr-3">Adapter</th><th className="pr-3">Type</th><th className="pr-3">State</th><th className="pr-3">IP</th><th>Default route</th></tr></thead>
                <tbody>
                  {report.interfaces.map((item) => (
                    <tr key={item.name} className="border-t border-ink-700">
                      <td className="py-2 pr-3">{item.name}</td><td className="pr-3">{item.kind}</td><td className="pr-3">{item.is_up ? "Up" : "Down"}</td>
                      <td className="pr-3">{[item.has_ipv4 && "IPv4", item.has_ipv6 && "IPv6"].filter(Boolean).join(", ") || "none"}</td><td>{item.is_default_route ? "Yes" : "No"}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            <div className="mobile-data-list">
              {report.interfaces.map((item) => (
                <MobileDataCard key={item.name} title={item.name} rows={[
                  { label: "Type", value: item.kind },
                  { label: "State", value: item.is_up ? "Up" : "Down" },
                  { label: "IP", value: [item.has_ipv4 && "IPv4", item.has_ipv6 && "IPv6"].filter(Boolean).join(", ") || "none" },
                  { label: "Default route", value: item.is_default_route ? "Yes" : "No" },
                ]} />
              ))}
            </div>
          </>
        )}
      </Card>

      <Card title="DNS lookups (system resolver)">
        {report.dns.length === 0 ? <p className="text-sm text-slate-500">Not measured.</p> : (
          <>
            <div className="desktop-data-table">
              <table className="w-full text-left text-sm">
                <thead className="text-xs text-slate-500"><tr><th className="py-2 pr-3">Host</th><th className="pr-3">Result</th><th>Time</th></tr></thead>
                <tbody>
                  {report.dns.map((item) => (
                    <tr key={item.host} className="border-t border-ink-700">
                      <td className="py-2 pr-3">{item.host}</td>
                      <td className="pr-3">{item.ok ? `${item.address_count} address(es)` : item.timed_out ? "Timed out" : `Failed: ${item.error ?? "unknown"}`}</td>
                      <td>{fmtMs(item.time_ms)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            <div className="mobile-data-list">
              {report.dns.map((item) => (
                <MobileDataCard key={item.host} title={item.host} rows={[
                  { label: "Result", value: item.ok ? `${item.address_count} address(es)` : item.timed_out ? "Timed out" : `Failed: ${item.error ?? "unknown"}` },
                  { label: "Time", value: fmtMs(item.time_ms) },
                ]} />
              ))}
            </div>
          </>
        )}
      </Card>

      <Card title="Latency, jitter and loss (TCP connect)">
        {report.latency.length === 0 ? <p className="text-sm text-slate-500">Not measured.</p> : (
          <>
            <div className="desktop-data-table">
              <table className="w-full text-left text-sm">
                <thead className="text-xs text-slate-500"><tr><th className="py-2 pr-3">Target</th><th className="pr-3">Median</th><th className="pr-3">p95</th><th className="pr-3">Jitter</th><th className="pr-3">Loss</th><th>Probes</th></tr></thead>
                <tbody>
                  {report.latency.map((item) => (
                    <tr key={item.target} className="border-t border-ink-700">
                      <td className="py-2 pr-3">{item.target}</td><td className="pr-3">{fmtMs(item.stats.median_ms)}</td><td className="pr-3">{fmtMs(item.stats.p95_ms)}</td>
                      <td className="pr-3">{fmtMs(item.stats.jitter_ms)}</td><td className="pr-3">{fmtPct(item.stats.loss_pct, 0)}</td><td>{item.stats.received}/{item.stats.sent}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            <div className="mobile-data-list">
              {report.latency.map((item) => (
                <MobileDataCard key={item.target} title={item.target} rows={[
                  { label: "Median", value: fmtMs(item.stats.median_ms) },
                  { label: "95th percentile", value: fmtMs(item.stats.p95_ms) },
                  { label: "Jitter", value: fmtMs(item.stats.jitter_ms) },
                  { label: "Loss", value: fmtPct(item.stats.loss_pct, 0) },
                  { label: "Probes received", value: `${item.stats.received}/${item.stats.sent}` },
                ]} />
              ))}
            </div>
          </>
        )}
      </Card>

      <Card title="HTTPS reachability">
        {report.https.length === 0 ? <p className="text-sm text-slate-500">Not measured.</p> : (
          <>
            <div className="desktop-data-table">
              <table className="w-full text-left text-sm">
                <thead className="text-xs text-slate-500"><tr><th className="py-2 pr-3">Endpoint</th><th className="pr-3">Result</th><th>Time</th></tr></thead>
                <tbody>
                  {report.https.map((item) => (
                    <tr key={item.url} className="border-t border-ink-700">
                      <td className="py-2 pr-3 break-all">{item.url}</td>
                      <td className="pr-3">{item.ok ? `HTTP ${item.status}` : item.timed_out ? "Timed out" : item.error ?? "Failed"}</td>
                      <td>{fmtMs(item.time_ms)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            <div className="mobile-data-list">
              {report.https.map((item) => (
                <MobileDataCard key={item.url} title={item.url} rows={[
                  { label: "Result", value: item.ok ? `HTTP ${item.status}` : item.timed_out ? "Timed out" : item.error ?? "Failed" },
                  { label: "Time", value: fmtMs(item.time_ms) },
                ]} />
              ))}
            </div>
          </>
        )}
      </Card>

      <Card title="Not measured, and limitations">
        <ul className="list-disc space-y-2 pl-5 text-sm text-slate-400">
          {report.unavailable.map((item) => <li key={item.name}><span className="text-slate-200">{item.name}:</span> {item.reason}</li>)}
          {report.diagnosis?.limitations.map((limitation) => <li key={limitation}>{limitation}</li>)}
        </ul>
        <p className="mt-3 text-xs text-slate-500">Wi-Fi link rate and internet throughput are different things. Use Wi-Fi Analysis for adapter details and Speed Test for measured download/upload; neither is part of this scan report.</p>
      </Card>
    </div>
  );
}
