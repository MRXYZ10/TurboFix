# TurboFix v0.3.4 Verification Status

## Release candidate checks

The following checks passed on the v0.3.4 working tree:

- Frontend tests: 30 passed across 5 files (`npm.cmd test`).
- Frontend lint: passed (`npm.cmd run lint`).
- Frontend production build: passed (`npm.cmd run build`, also run by the Tauri package command).
- Rust formatting: passed (`cargo fmt --all -- --check`).
- Rust Clippy: passed (`cargo clippy --workspace --all-targets -- -D warnings`).
- Rust workspace tests: passed (69 core unit tests, 11 integration tests, and all remaining workspace/doc-test targets).
- `git diff --check`: passed; Git reported existing LF-to-CRLF working-copy warnings.
- Windows NSIS installer: built successfully with the isolated Cargo target directory.
- Installer path: `target-v034-isolated/release/bundle/nsis/TurboFix_0.3.4_x64-setup.exe` (2,742,884 bytes).
- Installer SHA-256: `4A9892937255164633C15869D00A84A3BBA9FADB8A64B3D3783C76C48DB3F56A`.
- Installer signing: unsigned unless signing is separately completed and verified.

## Manual Windows checks

Interactive desktop testing is not implied by automated tests or installer generation. Verify launching and closing the app; successful and refused local TCP targets; monitoring start/stop and persisted history after restart; and optimizer preview, cancellation, explicit confirmation, execution, and repeat-scan evidence. Also test clean installation, upgrade, and uninstall in a disposable Windows account or VM.

## Limitations

No broad Windows hardware or network certification is claimed. TCP-connect timings are not ICMP ping or packet loss. DNS-cache flushing may not resolve an observed DNS failure; measurements can vary with targets, adapter, resolver, firewall, and background traffic. The installer is unsigned until signing and signature verification are completed.
