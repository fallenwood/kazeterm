# Titlebar title updates

## Changes

- `apps/kazeterm/src/components/main_window_render.rs` now updates the native window title from the active tab's display title (`<title> - Kazeterm`), with `Kazeterm` as the empty/missing-title fallback. Reuses `display_text` for native title presentation and caches the last applied title on `MainWindow` to avoid redundant platform calls on repaint.
- `main_window_tab_management.rs` keeps the automatic title current while a custom display name is set. Clearing the custom name therefore reveals the latest terminal title, not a stale one.
- `main_window_dialog_handlers.rs` dispatches `UIAction::RenameTab` instead of changing only the live item, keeping UI-tree state and rename/clear operations consistent.
- Terminal title debouncing uses the GPUI background executor timer, preserving the configured delay and cancellable task while allowing deterministic fake-clock integration tests.
- Added two fake-session GPUI regression tests covering native titles, active-tab switching, empty-title fallback, background terminal title changes, custom-name preservation, rename-dialog integration, and restoring the latest automatic title.

## Validation

- Before the fix, both new focused tests failed (stale automatic title and no native window title).
- `cargo test --package kazeterm --bin kazeterm title_ -- --test-threads=1`: 2 passed after the fix.
- `cargo test --workspace`: 459 passed, 0 failed; 1 existing ignored doctest.
- `cargo fmt --all -- --check`: passed.
- `pwsh -NoProfile -File scripts/check-workspace.ps1`: passed.
- `git diff --check`: passed; reviewed implementation and test diffs.
- Existing Linux unused-import/dead-code warnings remain unchanged. No manual native-window smoke test or Windows/macOS validation was performed; validation used the headless GPUI test platform on Linux.
