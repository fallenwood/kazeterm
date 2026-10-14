# Shorten Settings back button label

- Changed the Settings return button label from `Back to terminal` to `Back` in `crates/kazeterm/src/components/settings_page/mod.rs`.
- Preserved the arrow icon, sidebar-aligned width, styling, disabled state, and return behavior.

## Validation
- `cargo fmt --all -- --check`: passed, with existing stable-rustfmt trailing_comma warnings.
- `cargo test --package kazeterm settings_back_button_aligns_with_sidebar_navigation`: 1 passed.
- Reviewed the label change and ran `git diff --check`.
- Full application/workspace tests and manual GUI checks were not repeated for this text-only change.
