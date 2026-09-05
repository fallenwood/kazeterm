# Settings dropdown width

- Added a shared settings-dropdown renderer that measures the resolved control
  width through GPUI Kit's prepaint hook and retains it in window-keyed state.
- Popup menus use that width for both minimum and maximum sizing instead of the
  default narrow content width and 500-pixel maximum.
- Applied the same sizing to scalar choices, default profiles, themes, and
  keybinding actions. Selection callbacks and disabled behavior are unchanged.
- Added a headless interaction test that opens the popup at 600-, 1400-, and
  420-pixel window widths, including reopening after resize, and compares the
  option bounds with the control bounds.

## Validation

- `cargo fmt --package kazeterm` completed.
- `cargo test --package kazeterm --bin kazeterm settings_page -- --test-threads=1`
  passed all 18 tests.
- `cargo check --package kazeterm --bin kazeterm` passed.
- `git diff --check` passed and the focused implementation diff was reviewed.
- Existing rustfmt/linker warnings and incremental-cache access notes remain.
- No full-workspace rerun or live UI automation was needed for this local layout
  adjustment.
