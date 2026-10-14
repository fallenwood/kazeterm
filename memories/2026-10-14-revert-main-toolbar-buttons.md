# Revert main-page toolbar button styling

## Current iteration
- Per user request, reverted only the three main-page toolbar controls in `crates/kazeterm/src/components/main_window_render.rs`: toggle-tab-bar, new tab, and dropdown menu.
- Restored `.ghost().small()` instead of primary outline, and restored their container's `.gap_0()` spacing.
- The file now has no diff against HEAD. Preserved all other pending dialog, Settings, search, and global theme changes.

## Validation
- `cargo fmt --all -- --check`: passed, with existing stable-rustfmt trailing_comma warnings.
- `cargo test --package kazeterm --bin kazeterm`: 107 passed.
- Reviewed the toolbar diff and checked that other pending changes remain intact.
- Workspace tests were not repeated for this local style rollback; the previous iteration's workspace tests passed.
- No manual GUI or Linux/macOS validation was performed.
