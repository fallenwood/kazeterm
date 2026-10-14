# Align Settings back button with sidebar navigation

## Current iteration
- Updated `crates/kazeterm/src/components/settings_page/mod.rs` so Back to terminal uses the same fixed-width column and horizontal padding as the navigation below it.
- Shared the existing 180px sidebar width through `SIDEBAR_WIDTH`, made the back button fill the column, and reserved the sidebar's 1px right border transparently in the header for exact edge alignment.
- Preserved button styling, height, busy-state handling, and click behavior. The main-page toolbar rollback and other pending changes remain intact.

## Regression coverage
- Added `settings_back_button_aligns_with_sidebar_navigation` in `crates/kazeterm/src/components/settings_page/tests.rs`.
- Checks both edges and width at 600px/1400px window widths and 18px/22px UI font sizes.
- Failed before the fix (back left edge 12px versus navigation 8px), passed after the fix.

## Validation
- `cargo test --package kazeterm settings_back_button_aligns_with_sidebar_navigation`: failed as expected before the fix.
- `cargo fmt --all`
- `cargo fmt --all -- --check`: passed, with existing stable-rustfmt trailing_comma warnings.
- `cargo test --package kazeterm settings_`: 21 passed.
- Reviewed changed Settings layout/test diffs and ran `git diff --check`.
- Workspace tests were not repeated for this local layout change. No manual screenshot or Linux/macOS validation was performed.
