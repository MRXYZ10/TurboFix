# Changelog

## 0.3.4 — Evidence-based diagnostics and optimization

- Report TCP diagnostic outcomes distinctly and label TCP-connect measurements accurately; IP packet loss is not measured.
- Add bounded, cancellable multi-target monitoring with jitter reporting and SQLite session history.
- Add evidence-based DNS cache flush recommendations with an explanation preview, explicit confirmation, fixed action validation, repeat measurements, and before/after history.
- Preserve additive SQLite migration and compatibility with existing scan and history records.
- Display the package-metadata version and surface retryable frontend startup checks when backend initialization reports a failure.
- No arbitrary shell commands, broad registry changes, security disabling, or Windows service disabling are introduced.

## 0.3.0 — Premium UX and release hardening

- Reworked the desktop shell with a premium dark visual system, clearer navigation groups, active-state affordances, privacy messaging, contextual page descriptions, and a consistent top bar.
- Added responsive mobile navigation, keyboard focus visibility, reduced-motion support, and tighter small-window handling.
- Refined metadata and app branding for the Network Intelligence positioning.
- Lowered the minimum native window dimensions to improve usability on smaller laptop displays.
- Kept all actions evidence-based and confirmation-first; no unsupported claim of faster internet or hidden system tweaks was added.
- Preserved the existing Rust/Tauri diagnostic, speed-test, Wi-Fi, history, performance, optimizer, and cleanup flows.

## Verification status

The current execution environment does not contain populated npm executable packages or a Rust toolchain, so a production build and automated test suite could not be run here. See `docs/VERIFICATION_STATUS.md` and `docs/RELEASE_AUDIT.md`; do not treat this source release as a signed or verified installer.
