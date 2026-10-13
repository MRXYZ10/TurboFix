# Windows verification checklist

The source is prepared for Windows 10/11, but this authoring environment does not include the Rust toolchain or a real Windows Wi-Fi adapter. Treat the following as required verification before release.

| # | What to verify | How |
|---|---|---|
| 1 | App launches and all screens load | `npm run tauri dev` on Windows 10/11 |
| 2 | Speed-test endpoints respond and the reported download/upload figures are plausible | Speed Test; compare twice with other large transfers paused |
| 3 | Test result survives app restart and Clear all history removes it | Run speed test, restart, then clear history |
| 4 | Connected SSID/state/signal/channel/link rates parse correctly on the host's Windows language and adapter | Wi-Fi Analysis vs Windows Settings / `netsh wlan show interfaces` |
| 5 | Nearby SSIDs show strongest signal and no BSSID/MAC values | Wi-Fi Analysis vs `netsh wlan show networks mode=bssid` |
| 6 | DNS flush reports success/failure accurately and doesn't require elevation unexpectedly | Smart Optimizer, then run diagnostics |
| 7 | Settings validation is enforced by backend and persists between restarts | Change probe count/interval and run a scan |
| 8 | Adapter list, gateway, DNS, HTTPS and TCP diagnostic behaviour remains correct | Internet Diagnostics with Wi-Fi/Ethernet, VPN, and offline scenarios |
| 9 | CPU/RAM/disk/process values remain plausible | Compare PC Performance with Task Manager |
| 10 | Local history migration from an existing 0.1 database works | Open existing app data, then view previous scans and run speed test |
| 11 | Tests/lint/build work cleanly | `npm ci`; `npm run lint`; `npm run typecheck`; `npm test`; `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo test --workspace`; `npm run tauri build` |
| 12 | Installer installs/uninstalls per-user | Build NSIS installer (unsigned; SmartScreen warning expected) |

## Known technical caveats

* `netsh` text parsing depends on Windows output labels. If Windows is not using English labels, some Wi-Fi fields may appear unavailable; validate before release. Nearby SSIDs are displayed only in memory and are not stored in SQLite.
* Throughput test uses a 1 KB warm-up, measures 5 MB down and sends 1 MB up per run. It requires internet access to Cloudflare. The service may process request measurement data.
* The DNS cache flush is the only active network quick fix. Preview-first temp cleanup is available separately and must be tested on disposable files. DNS server changes, registry tweaks, adapter resets and startup modifications remain disabled until robust backup/rollback workflows exist.
* Never describe TCP-connect times as ICMP ping or a Wi-Fi link rate as actual internet speed.
