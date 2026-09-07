# Settings menu alignment

- Changed Settings to use the same custom menu-row layout as Open Config Path,
  Open Config File, and the other new-tab menu entries.
- Matched the centered 16-pixel icon container, icon sizing, and label gap instead
  of mixing a standard leading-icon menu item with custom-content menu items.
- Preserved the existing Settings click handler and all other menu behavior.

## Validation

- `cargo fmt --package kazeterm` completed.
- `cargo test --package kazeterm --bin kazeterm settings_page -- --test-threads=1`
  passed all 13 tests.
- `git diff --check` passed; the focused diff only changes the Settings row.
- Existing stable-rustfmt and MSVC linker warnings remain unchanged.
- No full-workspace rerun or live UI automation: this is a local menu layout fix,
  and the earlier UI automation was stopped by the user.
