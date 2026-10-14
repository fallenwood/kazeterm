# Settings buttons style unification

## What changed
- Updated Settings UI buttons to follow the unified filled/outline style instead of ghost style.

### `crates/kazeterm/src/components/settings_page/mod.rs`
- `settings-back`: `.ghost()` -> `.primary().outline()`
- Sidebar section buttons:
  - base style `.ghost()` -> `.primary()`
  - selected logic `.when(section == self.section, |button| button.primary())`
    -> `.when(section != self.section, |button| button.outline())`
  - Result: selected section is filled primary, non-selected sections are outlined primary.
- `settings-open-toml`: `.ghost()` -> `.primary().outline()`

### `crates/kazeterm/src/components/settings_page/collections.rs`
- Converted ghost remove buttons to outlined primary:
  - `settings-remove-arg-*`
  - `settings-remove-binding`
  - `settings-remove-env`
  - `settings-remove-import`
- Each changed from `.ghost()` to `.primary().outline()`.

## Follow-up (footer hierarchy polish)
- Updated footer actions in `crates/kazeterm/src/components/settings_page/mod.rs`:
  - `settings-keep-editing`: now `.primary().outline().small()`
  - `settings-discard`: now `.danger().outline().small()`
  - `settings-reload`: now `.primary().outline().small()`
  - `settings-revert`: now `.primary().outline().small()`
- Kept save actions as filled primary for clear primary/secondary hierarchy.

## Validation
- `cargo fmt --all`
- `cargo test --package kazeterm settings_`
  - Result: 20 passed, 0 failed.
