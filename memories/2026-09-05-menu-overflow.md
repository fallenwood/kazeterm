# Menu overflow

- Enabled GPUI Kit's scrollable popup mode for settings dropdowns, the new-tab
  menu, tab context menus, and terminal context menus including their submenus.
- Popup height now uses the toolkit's adaptive limit (half the window height,
  capped at 450 pixels). Existing window-edge positioning can keep the visible
  menu inside the window while overflowing items remain reachable by scrolling.
- Preserved settings dropdown width matching and all existing menu actions.
- Added headless regressions for long lists near the bottom/right window edges,
  wheel scrolling, keyboard scrolling, and clicking the final item after
  scrolling. Nested submenu scrolling and mouse selection are covered as well.
- Documented scrollable menus in the visual settings README section.

## Validation

- The new overflow regressions failed before the implementation change because
  the last menu items remained outside the window after scrolling.
- `cargo fmt --package kazeterm` completed.
- Focused popup tests were run during reproduction and implementation.
- `cargo test --package kazeterm --bin kazeterm -- --test-threads=1` passed all
  96 application tests.
- `cargo check --package kazeterm --bin kazeterm` passed.
- `git diff --check` passed; the implementation and regression tests were reviewed.
- Existing rustfmt/linker warnings and incremental-cache access notes remain.
- No dependency changes, real PTYs, or live application automation were needed.
  The full workspace suite was not rerun for this app-only menu adjustment.
