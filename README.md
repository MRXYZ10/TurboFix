# TurboFix 0.3.0

**Diagnose. Fix. Verify.** A Windows 10/11 desktop network diagnostic utility built with Tauri 2, Rust, React/TypeScript and SQLite. TurboFix measures real values, explains likely causes and provides a small set of safe, explicit quick fixes. It does not promise magical bandwidth increases.

## What works

| Area | Current implementation |
|---|---|
| Connectivity diagnostics | Active network adapters, gateway probes, OS DNS timings, TCP-connect latency/jitter/probe failure rate and HTTPS reachability with timeouts/cancellation. |
| Speed test | User-triggered HTTPS download/upload measurement against Cloudflare's public speed-test edge. Per run, the test uses a 1 KB connection warm-up, measures exactly 5 MB of download data, and uploads 1 MB of synthetic data. Response streaming is bounded to avoid unbounded memory use. Results are saved locally. Cloudflare may process test request measurements. |
| Wi-Fi analysis | On Windows, reads current connection signal, channel, radio type, receive/transmit link rate and up to 12 nearby SSIDs through fixed `netsh wlan show` queries. No SSID/BSSID data is persisted. |
| Safe quick fix | Explicit-confirmation DNS resolver cache flush via the fixed Windows `ipconfig /flushdns` command. No DNS server settings are changed. |
| Cleanup | Preview-first removal of selected regular files directly in the user temp folder that are at least 7 days old. No recursion; symlinks/folders are skipped; selection IDs must match the latest preview. |
| PC health | Live CPU, memory, swap, disk capacity, top CPU/memory processes and aggregate network activity. Disk I/O activity is not measured. |
| Settings | Configure bounded TCP probe count and interval for future diagnostic scans. |
| Local history | Scan reports, performance samples, configuration and latest 100 speed tests in SQLite. Scan reports can be exported as JSON. |
| Privacy/security | No telemetry or accounts in TurboFix. No arbitrary shell command API. External speed tests only run when clicked. |

## Important limitations

* Software cannot increase the bandwidth purchased from an ISP or repair a weak RF signal by itself. TurboFix can detect and help narrow down issues; some fixes are physical (move closer to the router), router-side or ISP-side.
* TCP-connect timings are not ICMP ping and may be affected by a firewall. Speed-test results depend on server location and active traffic.
* Wi-Fi details rely on the text output of Windows `netsh`; verify the results on your Windows language/adapter. Signal and link rate are different from internet throughput.
* DNS server changing/rollback, startup manager, TCP/registry tweaks and automatic adapter resets intentionally remain unavailable. These need stronger safety guarantees and rollback before they should ship.
* This archive is source code, not a prebuilt installer. The authoring environment could not complete a dependency install or run the Rust/Windows build; see `docs/VERIFICATION_STATUS.md`. The eventual installer will be unsigned unless you code-sign it, so Windows SmartScreen may warn.

## Windows 11 setup (PowerShell)

Prerequisites: Node.js 22 LTS, Rust stable MSVC toolchain, Visual Studio Build Tools with C++ workload, and WebView2. Then:

```powershell
cd path\to\turbofix
npm ci
npm run tauri -- dev

# Validate the frontend
npm run lint
npm run typecheck
npm test

# Validate Rust and build the app (on a machine with Rust installed)
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm run tauri -- build
```

For a repeatable verification run and optional installer build, use `powershell -ExecutionPolicy Bypass -File .\scripts\verify-windows.ps1 -BuildInstaller`. The NSIS installer is expected at `target\release\bundle\nsis\` (or the target directory reported by Cargo). See `docs/VERIFICATION_STATUS.md` for which checks were actually run for this source archive.

## Data and privacy

The SQLite database is stored in the app's OS app-data directory. Reports and up to 100 speed-test results are kept locally; use **Reports & History → Clear all history** to clear retained scan/performance/speed-test history. A speed test connects to `https://speed.cloudflare.com` and sends synthetic bytes only; it does not upload files from your PC. Cloudflare's test service may collect measurement/request data for aggregated insights (see the [Cloudflare Speedtest project](https://github.com/cloudflare/speedtest)).

## Project layout

```text
crates/turbofix-core/   Probe engine, diagnosis, Wi-Fi reader, speed test and SQLite storage
src-tauri/              Typed Tauri commands, app state, Windows safe quick fix
src/                    React UI and feature pages
docs/                   Windows verification checklist and measurement notes
```
