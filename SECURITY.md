# Security

* The app runs as a normal user and never requests elevation in this version.
* No arbitrary command execution is exposed to the UI. Tauri capabilities: `core:default` only. CSP restricts the webview to local assets and Tauri IPC.
* Probe targets are validated (hostnames/IPs and `https://` URLs only, bounded counts and timeouts).
* Stored data: scan reports, performance samples, settings, and speed-test measurements. No passwords, tokens, live Wi-Fi SSIDs/BSSIDs, or adapter MAC addresses are persisted. Scan reports do include the fixed diagnostic target hostnames and endpoint URLs, and exported JSON should be treated as a local diagnostic report.
* Network traffic: small TCP handshakes, DNS lookups and HTTPS probes. A user-triggered speed test transfers a 1 KB warm-up request, measures 5 MB down and sends 1 MB of synthetic data up to Cloudflare; Cloudflare may collect request measurements. No TurboFix telemetry, accounts or local-file uploads.
* DNS server changes, startup changes, arbitrary command execution and registry/service tweaks remain absent. Temp cleanup is preview-first, limited to directly contained old regular files, and requires selection plus explicit confirmation. DNS flushing is one fixed Windows command and changes no adapter settings.
* The installer is not code-signed.
* Report vulnerabilities privately to the repository owner.
