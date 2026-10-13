# TurboFix implementation status

## Architecture
Cargo workspace: `turbofix-core` (network diagnostics, Wi-Fi reader, speed test, cleanup safety checks, diagnosis and SQLite) and `src-tauri` (typed desktop commands and Windows-specific actions). React/TypeScript UI communicates with Rust through `src/api.ts`. SQLite schema changes use `PRAGMA user_version` migrations.

## Current status (0.3.0 source)
| Area | Status | Notes |
|---|---|---|
| Connectivity diagnostics | Implemented | Gateway reachability, DNS timing, TCP-connect timing/jitter/loss, HTTPS reachability, timeouts and scan cancellation. TCP-connect is not ICMP ping. |
| PC monitoring | Implemented | CPU, memory, swap, disk capacity, process summaries and aggregate network activity; disk I/O is not measured. |
| Speed test | Implemented in source | Explicit Cloudflare HTTPS throughput test with bounded transfer and local SQLite history. Must be exercised on Windows and compared with a second speed-test service before release. |
| Wi-Fi analysis | Implemented in source for Windows | Fixed `netsh` read-only queries. Parsing depends on Windows command output language and adapter driver. |
| DNS cache flush | Implemented in source for Windows | Fixed `ipconfig /flushdns`, confirmation-first. It can help stale cache cases but does not raise the ISP plan speed. |
| Temporary-file cleanup | Implemented in source | Preview and individually selected direct-child temp files at least 7 days old; no recursion; rechecks file type, size and modified time. Must be verified on Windows. |
| Scan settings, history and export | Implemented | Probe count/interval validation, SQLite history, JSON export and clear-history action. |
| DNS server modification/rollback | Disabled | Needs reliable adapter selection, backups, permission behaviour, connectivity verification and rollback. |
| Startup manager, service/registry tweaks | Not implemented | Deliberately not exposed as active controls. |
| Installer/signing | Not verified | Windows CI definition is present; this source build was not compiled in the authoring container. |

## Release gates
1. `npm ci`, `npm run lint`, `npm run typecheck`, and `npm test`.
2. `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace`.
3. On Windows 10/11: `npm run tauri dev`, test every feature and run through `docs/WINDOWS_VERIFICATION.md`.
4. Build the unsigned NSIS installer; verify clean install, upgrade over 0.1.0, uninstall, database migration and SmartScreen behaviour.
5. Only enable additional system-changing optimizations after backup, user consent, outcome validation and reliable rollback exist.
