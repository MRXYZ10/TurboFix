# TurboFix 0.3.4

**Diagnose. Fix. Verify.**

A Windows 10/11 desktop network diagnostic utility built with Tauri 2, Rust, React/TypeScript and SQLite. TurboFix measures real values, explains likely causes and provides a small set of safe, explicit quick fixes. It does not promise magical bandwidth increases.

## Download TurboFix

**[Download TurboFix 0.3.4 for Windows (64-bit)](https://github.com/MRXYZ10/TurboFix/releases/download/v0.3.4/TurboFix_0.3.4_x64-setup.exe)**

[View release details and SHA-256 checksum](https://github.com/MRXYZ10/TurboFix/releases/tag/v0.3.4)

Download the installer, open `TurboFix_0.3.4_x64-setup.exe`, and follow the installation instructions. Verify its SHA-256 against the value published on the release page.

**Security notice:** The installer is currently unsigned. Windows may display a security warning. Download it only from the official GitHub release page.

## What works

| Area | Current implementation |
|---|---|
| Connectivity diagnostics | Active network adapters, gateway probes, OS DNS timings, TCP-connect latency/jitter with distinct connection outcome categories, and HTTPS reachability with timeouts/cancellation. IP packet loss is not measured. |
| Pro Network Monitor | Continuous monitoring of 1–8 host:port targets; bounded TCP probe concurrency, per-stage timeouts, distinct outcomes, successful-latency history and bounded local session summaries. TCP-connect latency only; no ICMP ping or IP packet-loss measurement. |
| Speed test | User-triggered HTTPS download/upload measurement against Cloudflare's public speed-test edge. Per run, the test uses a 1 KB connection warm-up, measures exactly 5 MB of download data, and uploads 1 MB of synthetic data. Response streaming is bounded to avoid unbounded memory use. Results are saved locally. Cloudflare may process test request measurements. |
| Wi-Fi analysis | On Windows, reads current connection signal, channel, radio type, receive/transmit link rate and up to 12 nearby SSIDs through fixed `netsh wlan show` queries. No SSID/BSSID data is persisted. |
| Evidence-based optimizer | Recommends the fixed DNS resolver-cache flush only when a completed scan finds DNS lookup failures. It explains measured evidence, hypothesis, expected effect, risk and prerequisite; requires explicit confirmation; then reruns diagnostics and records before/after values and execution outcome locally. |
| Cleanup | Preview-first removal of selected regular files directly in the user temp folder that are at least 7 days old. No recursion; symlinks/folders are skipped; selection IDs must match the latest preview. |
| PC health | Live CPU, memory, swap, disk capacity, top CPU/memory processes and aggregate network activity. Disk I/O activity is not measured. |
| Settings | Configure bounded TCP probe count and interval for future diagnostic scans. |
| Local history | Scan reports, performance samples, configuration, and the latest 100 speed-test, monitor-session, and optimizer-run records in SQLite. Scan reports can be exported as JSON. |
| Privacy/security | No telemetry or accounts in TurboFix. No arbitrary shell command API. External speed tests only run when clicked. |

## Important limitations

- Software cannot increase the bandwidth purchased from an ISP or repair a weak RF signal by itself. TurboFix can detect and help narrow down issues; some fixes are physical (move closer to the router), router-side or ISP-side.
- TCP-connect timings are not ICMP ping and may be affected by a firewall. Speed-test results depend on server location and active traffic.
- Wi-Fi details rely on the text output of Windows `netsh`; verify the results on your Windows language/adapter. Signal and link rate are different from internet throughput.
- DNS server changing/rollback, startup manager, TCP/registry tweaks and automatic adapter resets intentionally remain unavailable. These need stronger safety guarantees and rollback before they should ship.
- The DNS cache flush can only remove stale cached answers. A diagnostic DNS failure does not prove stale cache is the cause, and the flush may not help. DNS and TCP measurements are separate samples; changes do not establish causation. Missing measurements and verification failures are reported as such.
- TurboFix is not production-certified and has not undergone a comprehensive independent security audit.
- The Windows installer is unsigned unless it is separately code-signed and verified.
- The Android version is not included in this release.

## Verification status

For this v0.3.4 release, the checks below are run against the release working tree. The exact results and installer SHA-256 are recorded in [`docs/VERIFICATION_STATUS.md`](docs/VERIFICATION_STATUS.md) after packaging.

- Frontend lint.
- TypeScript type checking.
- Rust formatting, Clippy, and workspace tests.
- Frontend tests, lint, and production build.
- Windows NSIS package build and installer checksum verification.

Automated checks and packaging do not establish interactive desktop, clean installation, upgrade, or uninstall behavior. Those Windows checks remain listed in the verification status document unless separately completed.

See [`docs/VERIFICATION_STATUS.md`](docs/VERIFICATION_STATUS.md) and [`docs/RELEASE_AUDIT.md`](docs/RELEASE_AUDIT.md) for further details.

## Windows setup (PowerShell)

Prerequisites: Node.js 22 LTS, Rust stable MSVC toolchain, Visual Studio Build Tools with the C++ workload, and WebView2.

Clone the repository and install dependencies:

```powershell
git clone https://github.com/MRXYZ10/TurboFix.git
cd TurboFix

npm ci
npm run tauri -- dev
```

### Validate the frontend

```powershell
npm run lint
npm run typecheck
npm test
npm run build
```

### Validate Rust

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

### Build the Windows installer

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\verify-windows.ps1 -BuildInstaller
```

The NSIS installer is normally generated under:

`target\release\bundle\nsis\`

See `docs/VERIFICATION_STATUS.md` for the verification results and limitations.

## Data and privacy

The SQLite database is stored in the app's OS app-data directory. Reports and up to 100 speed-test and monitor-session summaries are kept locally; individual monitor probes are not persisted. Use **Reports & History → Clear all history** to clear retained scan, performance, speed-test, and monitor-session history.

A speed test connects to `https://speed.cloudflare.com` and sends synthetic bytes only; it does not upload files from your PC. Cloudflare's test service may collect measurement/request data for aggregated insights. See the [Cloudflare Speedtest project](https://github.com/cloudflare/speedtest).

Wi-Fi scan results, including nearby SSIDs, are not persisted in TurboFix history according to the current implementation.

## Project layout

```text
crates/turbofix-core/   Probe engine, diagnosis, Wi-Fi reader, speed test and SQLite storage
src-tauri/              Typed Tauri commands, app state and Windows quick fixes
src/                    React UI and feature pages
docs/                   Verification checklists and measurement notes
scripts/                Windows verification script
```

## Security

Please report suspected security vulnerabilities responsibly. See [`SECURITY.md`](SECURITY.md) for project security guidance.

## License

A license has not yet been specified. Check the repository for licensing information before redistributing or incorporating this project into other software.

---

**Repository:** https://github.com/MRXYZ10/TurboFix

**Latest release:** https://github.com/MRXYZ10/TurboFix/releases/tag/v0.3.4
