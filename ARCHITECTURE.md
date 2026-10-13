# Architecture

```
React UI (src/) --typed invoke/events--> src-tauri (commands) --> turbofix-core
                                                                   |- network   (DNS, TCP latency, HTTPS, gateway, interfaces)
                                                                   |- stats     (median, percentile, jitter, loss, throughput)
                                                                   |- diagnosis (pure evidence -> findings + confidence)
                                                                   |- scan      (orchestration, cancellation, progress)
                                                                   |- performance (sysinfo snapshots)
                                                                   |- storage   (SQLite, migrations, retention)
                                                                   |- speedtest (bounded user-triggered HTTPS throughput test)
                                                                   |- wifi      (fixed, read-only Windows `netsh` queries)
                                                                   |- cleanup   (preview IDs + age/type/identity re-checks)
                                                                   |- platform  (capability registry: available/planned/disabled)
```

* **No generic execution:** there is no command that runs shell text, and the Tauri capability file grants only `core:default`.
* **Cancellation:** a `CancellationToken` is checked with biased `select!` around every await; a cancelled scan returns partial data with status `cancelled` and no diagnosis.
* **Bounded work:** probe counts, timeouts and target counts are validated (`ScanConfig::validate`); one scan at a time; performance sampling is 2 s only while the page is open, persisted at most every 15 s, capped at 20,000 rows; scans capped at 500.
* **Privacy:** live Wi-Fi SSIDs are not stored. Scan reports include the fixed diagnostic target hostnames and endpoint URLs; users may export these reports. A user-triggered speed test sends synthetic data to Cloudflare; TurboFix telemetry is not implemented.
* **Windows-specific work** is guarded by `cfg(windows)` and is never simulated on other systems. This authoring environment does not have Rust tooling or a Windows machine, so a successful Windows build and real adapter behaviour remain release gates.
