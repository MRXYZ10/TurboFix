# How TurboFix measures things

| Metric | Method | Caveats |
|---|---|---|
| Latency | Time from starting a TCP connection attempt (after DNS resolution) to a completed TCP handshake for each `host:port` target | TCP-connect latency, not ICMP ping; can differ from `ping`. |
| Median / p95 | Median of successful probe times; p95 by nearest-rank | p95 needs enough probes (default 10) to be meaningful |
| Jitter | Mean absolute difference between consecutive successful probe times | Not the RFC 3550 smoothed estimator |
| TCP outcomes | Connected, connection refused, resolution timeout/failure, connection timeout, and other transport errors are reported separately | A refusal means a TCP reset was returned; a firewall or intermediary may generate it. Timeouts are inconclusive. These results are not IP packet loss. |
| IP packet loss | Not measured | No packet-loss percentage is reported. Use a separately implemented ICMP or other packet-level measurement before making packet-loss claims. |
| Pro Network Monitor | Repeats TCP connects for 1–8 unique host:port targets. Each round runs with configured bounded concurrency and the next round starts only after the configured interval delay | Interval 500–60000 ms; timeout 200–10000 ms separately for resolution and connection; concurrency 1–4. TCP-connect latency excludes DNS lookup. Stop cancels outstanding async probe futures; platform resolver work may not be interruptible internally. |
| Monitor jitter | Mean absolute difference between consecutive successful TCP-connect latency samples for each target; failed attempts are skipped and do not reset the previous success | This is an application-level summary, not RFC 3550 jitter, and is not ICMP RTT variation. |
| Monitor history | Latest 30 probe outcomes and 60 successful latency points per target remain in memory; at most 100 completed session summaries are stored in SQLite | Probe-by-probe history is not persisted. Summary target names are local user-provided host:port values. |
| Monitor database migration | Schema v3 adds an indexed `monitor_sessions` table with summary JSON; existing scan, speed-test, settings, and performance tables are retained and migrated forward transactionally | No legacy report columns or serialized scan formats are rewritten. Clear all history also removes monitor summaries. |
| Optimizer history migration | Schema v4 adds indexed `optimizer_runs` records with the selected action, execution state, baseline report, optional verification report, and per-target comparisons; retains the latest 100 | Existing scan and legacy history records are retained; clear all history removes optimizer runs too. |
| DNS time | Wall time of the OS resolver (`getaddrinfo`) per host | May be served from the Windows DNS cache |
| Gateway | TCP connect to ports 443/80/53 of the default gateway; a refusal counts as *reachable*, silence is *inconclusive* | Routers often ignore probes; never proof of failure |
| HTTPS | HEAD request with certificate validation; any HTTP response counts as reachable | Redirects are not followed |
| CPU / memory / disk / processes | `sysinfo` crate | Process CPU % is per core. Disk activity is not measured. |

Not measured by the Connectivity Diagnostics scan: IP packet loss, download/upload throughput, and Wi-Fi signal/link rate. Use the separate Speed Test and Wi-Fi Analysis pages for throughput and radio data. ICMP ping is not implemented; the app uses TCP-connect timing instead.
# Optimizer verification

The optimizer currently supports only the fixed Windows DNS resolver-cache flush (`ipconfig.exe /flushdns`) and offers it only after a completed diagnostic scan reports DNS lookup failures. Those failures are measured findings; the stale-cache explanation is explicitly a hypothesis. The expected effect is limited to removing stale cached answers. No speed, latency, Wi-Fi, or FPS gain is promised.

The user reviews the finding, evidence, expected effect, risk, and Windows prerequisite before confirming. TurboFix invokes a fixed executable and fixed argument; it accepts no shell command text. No administrator elevation, registry edits, service changes, security changes, or DNS server changes are performed. Unsupported operations are not executed.

On confirmation, the saved scan is the baseline, the same configured diagnostic scan runs afterward, and both reports plus action, timestamp, outcome, and metric comparisons are stored in SQLite. DNS and TCP-connect values are compared per identical hostname/target; measurements from different successful targets are never combined. Missing values remain unavailable. A difference below 1 ms is reported as no measurable improvement; other changes are described without causal attribution. The scans are separate samples and can be affected by network conditions, resolver cache state, endpoint availability, VPN/proxy behavior, and concurrent traffic. If verification fails, execution and verification status remain separate and the before report is retained.
