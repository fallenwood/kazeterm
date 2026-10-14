# Borderless Settings category navigation

## Current iteration
- In `crates/kazeterm/src/components/settings_page/mod.rs`, switched category buttons (Startup, Appearance, Fonts, etc.) to ghost while unselected and filled primary while selected, removing their outline modifier.
- Kept Back and Open TOML file as primary outline, preserving the existing alignment, label, and click handlers.

## Validation
- Added `settings_navigation_has_no_outline_but_back_and_open_file_do` in `settings_page/tests.rs`, inspecting painted borders in the navigation and both retained outline controls.
- `cargo test --package kazeterm settings_navigation_has_no_outline_but_back_and_open_file_do`: failed before the change as expected.
- `cargo fmt --all` and `cargo fmt --all -- --check`: passed with existing stable-rustfmt trailing_comma warnings.
- `cargo test --package kazeterm settings_`: 22 passed, including the new border regression and existing alignment tests.
- Reviewed the Settings diff and ran `git diff --check`.
- Full workspace tests and manual GUI/platform validation were not repeated for this local styling change.
