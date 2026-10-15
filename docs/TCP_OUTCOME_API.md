# TCP probe outcome API compatibility

Each `LatencyResult` now includes an `outcomes` array. Entries are tagged by
`outcome` and distinguish `connected`, `connection_refused`, `timeout` (with a
`resolution` or `connection` stage), `resolution_failed`, and
`transport_error`. A refusal records that a TCP reset was returned; a firewall
or intermediary may generate it. TCP-connect latency statistics include only completed
handshakes. `tcp_failure_pct` is the percentage of attempted TCP connections
that did not complete; it is not an IP packet-loss measurement. `loss_pct` is
retained as a deprecated nullable field for older stored reports and is null
for newly measured results.

The Tauri `measure_tcp_ping` command now returns a tagged outcome object instead
of a nullable latency number. The command name is retained for compatibility,
but callers should treat it as a TCP-connect measurement, not ICMP ping.

No SQLite migration is required. Scan reports are stored as JSON, and the new
`outcomes` field defaults to an empty list when reading older reports. Their
legacy `loss_pct` values remain readable as historical data but the UI does not
display them as packet loss. The existing nullable `scans.loss_pct` SQLite
column remains in place; new rows store NULL, and scan-history summaries omit
old values because they represented TCP probe failures rather than measured
IP packet loss.
