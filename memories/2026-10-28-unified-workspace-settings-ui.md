# Unified workspace and Settings UI

## Changes
- Added `apps/kazeterm/src/components/chrome.rs` for shared page surfaces, interface typography, toolbars, compact ghost buttons, six-pixel corners, and navigation selection styling.
- Updated `main_window_render.rs` to use these helpers for the main page, Settings window shell, fullscreen title bars, and horizontal/vertical tabs. Kept event handlers, drag/drop, grouped-tab indicator colors, and terminal entities unchanged.
- Updated Settings to use the main page's title-bar surface for its sidebar, header, and footer. Replaced primary-filled category buttons with tab surfaces and a two-pixel left accent edge. Category labels are left-aligned and retain button accessibility and keyboard behavior.
- Made Back, Open TOML, Reload, and Clear search use the workspace's flat tool-button language. Save changes remains primary. Kept compact forms and responsive control layout from the first Settings redesign.
- Aligned Settings panel corners with workspace tabs. Changed the import notice to a surface token instead of using a muted text token as a background.
- Limited the old flat `Section::ALL` list to tests; production navigation now uses groups. No dependencies, configuration schema, platform boundaries, or Cargo.lock changes.

## Regression coverage
- Added a shared-chrome test for interface fonts, surfaces, selected/unselected navigation, both indicator axes, and distinct group indicators.
- Updated Settings theme tests for tab surfaces and accent edges across light, dark, and system modes. Verify translucent backgrounds and borders separately because GPUI paints them as separate quads.
- Updated toolbar tests for flat Back/Open TOML controls. Retained small-window bounds, font-size, search/draft, save/hot-reload, and live-session regressions.
- Fixed wildcard test-macro imports found during the first full compilation.

## Ablation study
- Baseline: `cargo test --locked --package kazeterm chrome_` passed.
- Temporarily replaced the shared navigation indicator color with transparent. The same test failed on the selected-state border assertion, as expected.
- Restored the indicator and ran the complete workspace suite successfully. No ablated code remains.

## Validation
Dependencies became available in this follow-up, so the earlier network/cache limitation no longer blocks validation.

- `cargo fmt --all` and `cargo fmt --all -- --check`: passed.
- `cargo test --locked --package kazeterm chrome_`: passed (1 test).
- `cargo test --locked --package kazeterm settings_`: passed (27 tests).
- `cargo test --locked --workspace`: passed, including all 124 application tests; one existing doctest remains ignored.
- `cargo build --locked --workspace`: passed on Windows.
- `pwsh -NoProfile -File scripts/check-workspace.ps1`: passed.
- `git diff --check`: passed. Reviewed the main-page diff, Settings diff, shared helper, and tests.
- Existing warnings in remote-control code and the fake-session test were left unchanged.

## Not run
- Manual GUI preview and Linux/macOS builds were not run.
- Release builds and Clippy were not run.
