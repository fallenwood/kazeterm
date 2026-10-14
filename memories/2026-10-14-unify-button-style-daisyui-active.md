# Unify button styling to DaisyUI-like active appearance

## What changed
- Updated global GPUI Kit theme mapping in `crates/themeing/src/lib.rs`.
- Imported `gpui_kit::component::Colorize` and centralized filled-button state colors in `SettingsStore::init_gpui_kit_theme`:
  - `button_fill = colors.text_accent`
  - `button_hover = button_fill.lighten(0.06)` in dark mode, `button_fill.darken(0.06)` in light mode
  - `button_active = button_fill.darken(0.14)`
  - foreground uses `colors.terminal_foreground`
- Applied that same state trio to all filled button token families:
  - default: `button`, `button_hover`, `button_active`, `button_foreground`
  - `button_primary_*`
  - `button_secondary_*`
  - `button_danger_*`
  - `button_warning_*`
  - `button_success_*`
  - `button_info_*`
- Also aligned semantic primary tokens used by primary buttons:
  - `theme.primary`, `theme.primary_hover`, `theme.primary_active`

## Follow-up on close dialog button
- In `crates/kazeterm/src/components/close_confirm_dialog.rs`, the non-restore confirm action remains `.primary()` (no local outline override), relying on the unified global button state palette.

## Validation
- `cargo fmt --all`
- `cargo test --package themeing`
  - Result: 3 passed, 0 failed
- `cargo test --package kazeterm close_confirm_dialog`
  - Result: 6 passed, 0 failed
