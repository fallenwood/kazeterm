# Independent OSC 7501 notification switch

## Changes

- Add `[notification].osc7501_enabled` and the Settings -> Notifications -> Enable OSC 7501 notifications switch.
- Default the source switch to `true` to preserve the preceding iteration's behavior. The desktop-notification master switch still defaults to `false`. Both must be enabled to submit OSC status notifications.
- Gate the shared notification policy, not the terminal parser or event observation. Status indicators and deduplication remain active while the source is disabled. Re-enabling it does not replay consumed reports. Bell/prompt echoes do not bypass suppression of the same structured event; ordinary commands without OSC status can still notify.
- Reuse the settings draft/save/live-application path. No terminal or PTY restart is required.
- Bump `CURRENT_CONFIG_VERSION` to `20261031.2`. Append the adjacent `20261031.1 -> 20261031.2` migration, inserting only a missing source switch. Preserve explicit values, the master switch, timing fields, and import-overlay priority.
- Update README and the migration codemap. No dependencies, `Cargo.lock`, terminal protocol, backend boundaries, or workspace/UI-tree formats change in this iteration. Preserve the earlier uncommitted notification work.

## Tests

- Extend grouped config tests for defaults, partial legacy sections, explicit source choices, serialization round trips, and invalid boolean types.
- Test migration preservation and idempotence; extend editor round-trip and partial-import priority coverage.
- Exercise all four master/source policy combinations, including idle and interval boundaries.
- Extend the existing settings save test to verify draft isolation, persistence, live global application, and unchanged terminal entities.
- Add a fake-session GPUI test for disabled structured delivery, retained status records, cross-source suppression, live enable/disable, no stale replay, a new lifecycle, and ordinary legacy notifications. Capture payloads instead of invoking real OS notifications.

## Ablation study

- Temporarily remove the OSC source-switch predicate from the shared delivery policy.
- `cargo test --locked --package kazeterm osc7501_switch_applies_live` fails at the disabled-source assertion as expected.
- Restore the predicate. The focused and complete workspace suites pass afterward.

## Validation

Passed on Windows:

- `cargo fmt --all` and `cargo fmt --all -- --check`.
- `cargo test --locked --package config notification` (4 tests).
- `cargo test --locked --package kazeterm notifications` (11 tests).
- `cargo test --locked --package kazeterm settings_ --quiet` (30 tests).
- `cargo test --locked --workspace --quiet` (includes 128 config and 139 application tests; one existing doctest remains ignored).
- `cargo build --locked --workspace --quiet`.
- `pwsh -NoProfile -File scripts/check-workspace.ps1`.
- `git diff --check` and source/diff review.

Existing remote-control dead-code and fake-session import warnings remain. Not run: Clippy, Linux/macOS target builds, manual GUI checks, or real OS-notification delivery. The referenced `.claude/COMPONENT_TEST_RULES.md` was absent in this checkout; tests follow the grouped branch/transition guidance in the repository agent guide.
