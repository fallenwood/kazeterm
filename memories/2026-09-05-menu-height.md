# Menu height

- Replaced the previous half-window/450-pixel popup height limit with the full
  available viewport height, reserving 20 logical pixels for borders and margins.
- Centralized sizing for settings dropdowns, the new-tab menu, tab context menus,
  and terminal context menus including submenus. Short menus still size to their
  content; long menus scroll only when they cannot fit.
- Explicit GPUI popup sizes are fixed at creation. Window resizing now dismisses
  open menus through their normal cancel action and restores the previous focus,
  so reopening recomputes bounds instead of retaining an oversized popup.
  Resize subscriptions are released with their owning menu entities.
- Extended headless regressions to check use of available height, windows up to
  1000 pixels tall, compact short menus, nested scrolling, resize dismissal, and
  focus restoration. The available-height assertions failed before the fix.
- Updated the README to describe the new sizing behavior.

## Validation

- `cargo fmt --package kazeterm` completed.
- `cargo test --package kazeterm --bin kazeterm popup -- --test-threads=1`
  passed all 3 focused popup regressions after the fix.
- `cargo test --package kazeterm --bin kazeterm -- settings_page menu_builder --test-threads=1`
  passed all 20 selected settings and menu tests.
- `cargo check --package kazeterm --bin kazeterm` passed.
- Reviewed the shared helper and all callers; no dependencies or platform
  boundaries changed. Full-workspace tests and live UI automation were not rerun
  for this app-only adjustment.
- Existing rustfmt/linker warnings and incremental-cache access notes remain.
