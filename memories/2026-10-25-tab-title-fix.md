# Tab title fixes

## Scope and root causes

This iteration specifically fixes tab labels, following the earlier native-window-title changes, which remain in the working tree.

- Any pane's `UpdateTab` event previously assigned its title directly to the owning tab, so an inactive split could overwrite the active split's title.
- Changing the focused pane did not recompute the tab label. Mouse-driven focus synchronization also ran after tab labels had already been built for the frame.
- Empty terminal titles (including metadata-only updates before the first terminal title) could clear a tab label.
- Workspace restore/rebuild copied custom names into the automatic-title field, so removing a custom name after rebuilding splits could expose the custom name again.

## Changes

- Added `MainWindow::refresh_tab_titles` in `apps/kazeterm/src/components/main_window_tab_management.rs`: derive each automatic title from its active pane's canonical UI-tree metadata, with profile/shell-name fallback for empty titles. Reading confirmed metadata instead of the live terminal model preserves the configured debounce delay.
- Terminal metadata events now update pane metadata through the existing UI actions rather than independently overwriting the tab label.
- Refresh derived titles after UI actions and live-state capture in `main_window.rs`; synchronize OS pane focus and titles before building labels in `main_window_render.rs`. This also covers keyboard/mouse pane changes, pane closure, and external `UpdatePaneTitle` actions.
- Keep automatic and custom titles separate during restore/rebuild in `workspace_state.rs`.
- Added two fake-session GPUI regressions covering inactive-pane updates, keyboard/mouse focus, pane closure, empty-title fallback, custom-name clearing after rebuilding splits, and external pane-title updates. Extended the earlier debounce test to verify the title does not change before the configured delay. Adjusted the earlier native-title fallback test to exercise a genuinely empty window rather than manually clearing derived state.

## Validation

- Before the fix, the two new focused tests reproduced inactive-pane title overwrites and empty tab labels.
- `cargo test --package kazeterm --bin kazeterm title_ -- --test-threads=1`: 4 passed.
- `cargo test --workspace`: 461 passed, 0 failed; 1 existing ignored doctest.
- `cargo fmt --all -- --check`: passed.
- `pwsh -NoProfile -File scripts/check-workspace.ps1`: passed.
- `git diff --check`: passed; reviewed implementation/test diffs.
- Existing Linux unused-import/dead-code warnings remain unchanged. No manual desktop GUI test or Windows/macOS validation was performed.
