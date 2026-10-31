# Make desktop notifications opt-in

## Changes
- Added `notification.enabled: bool` to `NotificationConfig` in `crates/config/src/lib.rs`. The default is `false`, including when the section or key is omitted.
- Added the Notifications page switch `Enable desktop notifications` through the existing field metadata and boolean-control path.
- The common notification decision in `apps/kazeterm/src/components/notifications.rs` now requires explicit enablement, in addition to the existing idle threshold and notification interval. Both command completion and Bell desktop notifications use this path.
- Native macOS, Linux, and Windows delivery code is unchanged. The switch does not mute terminal bell sounds or hide visual bell indicators.
- Added migration `20260905.1 -> 20261031.1` and advanced `CURRENT_CONFIG_VERSION`. The migration inserts `notification.enabled = false` only when absent. Explicit choices and existing timing values remain intact.
- Updated README usage and the migration step map. No dependencies, Cargo.lock, terminal backend boundaries, or user configuration files were changed directly.

## Tests
- Config tests cover missing/partial sections, default disablement, explicit on/off values, serialization roundtrips, and invalid boolean types.
- Migration tests cover missing sections/keys, explicit on/off values, preserved thresholds/intervals, version advancement, and idempotence.
- Editor tests cover persisted opt-in plus import priority, partial overlays, and byte-identical imported files.
- The existing fake-session settings-save integration test checks that the draft does not change the running flag before Save, then verifies persistence and live application without replacing terminals.
- A pure notification policy test covers both enabled states, idle and throttle boundaries, and zero thresholds/intervals. It does not send native notifications or spawn a real shell/PTY.

## Ablation study
- Extracted the old threshold/interval-only decision and ran the new notification policy test without the enabled gate.
- It failed: the policy returned `true` for a disabled configuration once the timing requirements were met.
- Added the enabled gate and reran the policy and settings/config suites. All passed. Enabled timing cases also passed, so this is not an unconditional suppression of notifications.

## Validation
- `cargo fmt --all` and `cargo fmt --all -- --check`: passed.
- `cargo test --locked --package config notification`: passed (3 tests).
- `cargo test --locked --package kazeterm notification_policy`: passed (1 test).
- `cargo test --locked --package kazeterm settings_ --quiet`: passed (30 tests).
- `cargo test --locked --workspace --quiet`: passed, including 127 config tests and 129 application tests; one existing doctest remains ignored.
- `cargo build --locked --workspace --quiet`: passed on Windows.
- `pwsh -NoProfile -File scripts/check-workspace.ps1`: passed.
- `git diff --check`: passed. Reviewed all changed runtime, config, migration, metadata, documentation, and test code.
- Existing remote-control dead-code warnings and fake-session unused import remain unchanged.
- Manual GUI/native notification checks, Linux/macOS builds, release builds, and Clippy were not run.
