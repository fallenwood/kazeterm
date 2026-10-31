# Source-specific desktop notification content

## Changes
- Added `apps/kazeterm/src/components/notifications/content.rs` for pure notification content generation and native text serialization.
- The existing `CommandFinished` source now uses `Kazeterm — Command finished`, with the previous terminal title as the subject and the current title as fallback.
- The existing `Bell` source now uses `Kazeterm — Terminal bell`, with the current terminal/program title. A bell does not imply command completion or success.
- Bodies include the owning tab name when it adds context. Whitespace-only titles use source-specific fallback text. Matching tab and subject names are not repeated.
- `notifications.rs` now generates one owned title/body payload and passes it to every platform adapter. Windows Toast XML, Linux summary/body, macOS native delivery, and the macOS osascript fallback preserve the source heading.
- Both Windows text fields are XML-escaped. The osascript fallback escapes both fields, including quotes, backslashes, newlines, carriage returns, and tabs. Native macOS title encoding uses UTF-8 byte lengths.
- The existing opt-in flag, default-off behavior, idle threshold, global notification interval, bell sound, event subscriptions, and thread delivery remain unchanged. No new event sources or success/exit-status inference were added.
- Updated README documentation. No persisted config fields, migrations, dependencies, Cargo.lock, terminal backend APIs, or user files changed.

## Tests and ablation
- Added grouped pure tests for command versus bell content, missing/whitespace titles, previous/current title priority, duplicate tab context, and native XML/script escaping. These tests do not send native notifications or spawn a real shell/PTY.
- Negative control: implemented the old generic Kazeterm heading/body in the new formatter. `notification_content_distinguishes_command_completion_from_bell` failed on the generic heading.
- Replaced it with source-specific formatting. All content tests and the existing enablement/throttling policy test then passed.

## Validation
- `cargo fmt --all` and `cargo fmt --all -- --check`: passed.
- `cargo test --locked --package kazeterm notifications`: passed (4 tests).
- `cargo test --locked --workspace --quiet`: passed, including 132 application tests; one existing doctest remains ignored.
- `cargo build --locked --workspace --quiet`: passed on Windows.
- `pwsh -NoProfile -File scripts/check-workspace.ps1`: passed.
- `git diff --check`: passed. Reviewed all formatter, caller, adapter, and documentation changes.
- Existing remote-control dead-code warnings and fake-session unused import remain unchanged.
- Manual native notification/GUI checks, Linux/macOS builds, release builds, and Clippy were not run. XML and osascript serialization were tested as pure string operations on Windows; that does not verify native delivery on other platforms.
