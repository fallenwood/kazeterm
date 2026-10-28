# Settings UI redesign

This records the first design pass. The follow-up in `2026-10-28-unified-workspace-settings-ui.md` replaces the toolbar outlines with flat controls and completes workspace tests and builds after dependencies became available.

## Changes
- Added `apps/kazeterm/src/components/settings_page/layout.rs` for the page header, grouped sidebar, responsive title/search area, and empty states.
- Grouped all 14 categories under Workspace, Terminal, Preferences, and Advanced. Kept category buttons borderless and retained the Back/Open TOML outlines and their alignment.
- In `settings_page/mod.rs`, replaced individual scalar cards with one panel of aligned rows. Wide windows use compact right-hand controls. Narrow windows stack text controls; switches stay inline. Field help is available through info tooltips.
- Search results show category context. Clear-search controls return to the selected category without replacing the draft. Category navigation shares the same search/scroll reset helper.
- Kept the footer outside the scroll area. Reduced secondary action emphasis and made Save changes the primary action. Kept existing save, reload, revert, and unsaved-change confirmation behavior.
- In `settings_page/collections.rs`, made shortcut, environment, and import editors use compact columns in wide windows and stack in narrow windows. Profiles use their names as headings. Added controls use one size; destructive controls use danger styling.
- Used existing theme tokens. No new dependencies, persisted settings, schema changes, or Cargo.lock changes.

## Validation
- `cargo fmt --all`: passed.
- `cargo fmt --all -- --check`: passed.
- `pwsh -NoProfile -File scripts/check-workspace.ps1`: passed.
- `git diff --check`: passed. Reviewed tracked changes and the new layout module.
- Added GPUI regressions for control bounds at 600/1200 pixels and 18/22-pixel interface fonts, plus search/clear preserving a modified draft.
- Added a pure navigation coverage test. Extracted the actual Section metadata, navigation table, and test into a temporary dependency-free Rust test harness. `rustc --edition 2024 --test` ran it successfully: 1 passed.

## Ablation study
- Removed the Advanced group from an in-memory copy of the actual navigation table and ran the same isolated coverage test.
- The complete table passed. The ablated table failed as expected: 12 categories versus 14. This confirms that the test detects categories lost during grouping. Repository sources were not changed for this comparison.

## Blocked or skipped
- `cargo test --locked --package kazeterm settings_` could not fetch the crates.io index entry for `gpui-fast`: network transfer timed out.
- `cargo test --offline --locked --package kazeterm settings_` was also attempted. The required `gpui-fast 0.1.10` is not cached; only 0.1.9/0.1.8 are available.
- Full application compilation, GPUI tests (including the new regressions), and manual window previews remain unverified. Did not downgrade dependencies to bypass the failure.
- Full workspace tests and other-platform builds were not run for this app-local presentation change.
