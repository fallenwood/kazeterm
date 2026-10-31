# OSC 7501 desktop notifications and event deduplication

## Scope

- Extend the source-specific notification content from the previous iteration.
- Notify for OSC 7501 `done`, `error`, and `blocked` states. Distinguish permission, question, and authentication requests. Include the application, task title/ID, message, and useful tab context.
- Do not notify for `idle` or `working`. Keep the existing default-off desktop-notification switch. Explicit status notifications bypass the legacy idle threshold, but still obey the switch and notification interval.
- No new persisted configuration or dependencies. `Cargo.lock` and the OSC wire format are unchanged. Settings copy and README describe the new source.

## Implementation

- Add an internal `ProgramStatusRecord.state_revision` in terminal core. It stays stable for text/progress refreshes and changes for state/kind transitions or task recreation. This preserves lifecycle changes even when intermediate reports are absent from a UI snapshot. Core stays independent of GPUI and configuration.
- Keep notification history in an app-global center, keyed by terminal-view entity ID. Observe suppressed events so enabling notifications or changing policy does not replay old reports. Prune records that leave the bounded core snapshot.
- Coalesce Bell and prompt-return signals for 200 ms. Prefer prompt-return content over Bell content, and structured OSC content over either fallback. An actually submitted fallback can consume one late structured event in the same input cycle; rejected fallback events do not consume structured notifications.
- Keep active structured tasks authoritative after input. Track separate task IDs and terminal entities independently. Prioritize blocked/error records when the configured interval limits a batch.
- Retain cancellable tasks in the center. Use weak terminal/window references, rebind window ownership with terminal subscriptions, and verify ownership before flushing. New input invalidates stale fallback work. Tab transfers retain the terminal entity, notification history, and pending work; no PTY is recreated.
- Native adapters share the owned content payload. Escape XML/Pango markup and the AppleScript fallback so status text is not interpreted as markup or script.

## Regression coverage

- Core: stable revisions on duplicate and metadata reports; new revisions after unseen working/done transitions, clear/recreate, and blocked-kind changes.
- Pure notification logic: content/fallback formatting, native text encoding, enabled/idle/interval policy, semantic deduplication, independent tasks, active tasks after input, both legacy/OSC arrival orders, and stale-input cancellation.
- GPUI integration: fake sessions and captured payloads only. Exercise parser -> terminal status event -> application delivery, background tabs, repeated reports with interval zero, late OSC, distinct terminals, pending delivery across window transfer, disabled-event consumption, and closing a terminal.
- The close test resolves the actual tab index from the terminal entity and asserts that the terminal was removed before checking cancellation. A UI position is not a stable tab index.

## Ablation study

- Temporarily remove the repeated-state `continue` in `NotificationState::observe`.
- Run `cargo test --locked --package kazeterm osc_notifications_are_once_per_state_transition_not_metadata_refresh`.
- The test fails at the duplicate-report assertion as expected. Restore the branch. The final focused and workspace suites pass.

## Validation

Passed on Windows:

- `cargo fmt --all` and `cargo fmt --all -- --check`.
- `cargo test --locked --package terminal-kernel program_status_state_revision` (1 test).
- `cargo test --locked --package kazeterm notifications` (10 tests).
- `cargo test --locked --workspace --quiet` (includes 138 application tests, 127 config tests, and 32 core tests; one existing doctest is ignored).
- `cargo build --locked --workspace --quiet`.
- `pwsh -NoProfile -File scripts/check-workspace.ps1`.
- `git diff --check` and source/diff review.

Existing remote-control dead-code and fake-session import warnings remain. Rust also reported non-fatal incremental-cache finalization access errors; both tests and build completed successfully. No generated files were edited by hand.

Not run: Clippy, Linux/macOS target builds, manual GUI checks, or real OS-notification delivery. Platform payload encoders were tested as pure functions on Windows. The integration tests do not send native notifications or start a real shell/PTY.
