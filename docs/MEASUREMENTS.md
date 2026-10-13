# How TurboFix measures things

| Metric | Method | Caveats |
|---|---|---|
| Latency | Time of a TCP connect (SYN to SYN/ACK) to each `host:port` target; resolution time excluded | Not ICMP; can differ from `ping`. Firewalls that drop SYNs look like loss. |
| Median / p95 | Median of successful probe times; p95 by nearest-rank | p95 needs enough probes (default 10) to be meaningful |
| Jitter | Mean absolute difference between consecutive successful probe times | Not the RFC 3550 smoothed estimator |
| Packet loss | failed probes / total probes x 100 (timeouts, refusals, errors) | TCP-probe loss, not ICMP loss |
| DNS time | Wall time of the OS resolver (`getaddrinfo`) per host | May be served from the Windows DNS cache |
| Gateway | TCP connect to ports 443/80/53 of the default gateway; a refusal counts as *reachable*, silence is *inconclusive* | Routers often ignore probes; never proof of failure |
| HTTPS | HEAD request with certificate validation; any HTTP response counts as reachable | Redirects are not followed |
| CPU / memory / disk / processes | `sysinfo` crate | Process CPU % is per core. Disk activity is not measured. |

Not measured by the Connectivity Diagnostics scan: download/upload throughput and Wi-Fi signal/link rate. Use the separate Speed Test and Wi-Fi Analysis pages. ICMP ping is not implemented; the app uses TCP-connect timing instead.
