# Add Icons to Group Context Menus

- Added icons to **Rename Group**, **Group Color**, and **Delete Group** on the group label's context menu. All six color presets in the submenu show round swatches drawn from the active theme.
- Added icons to **Remove from Group**, **Move to Group**, and its target-group entries in the tab context menu. **Create Group** continues to use the existing folder icon.
- Added `assets/icons/palette.svg` and `assets/icons/circle.svg` through the existing asset embedding mechanism, without adding dependencies.
- Extended menu integration coverage for the group context menu and checked that the required icon assets exist.

## Validation

- `cargo fmt --all` and `cargo fmt --all -- --check` passed.
- `cargo test --package kazeterm --bin kazeterm menu_builder -- --test-threads=1`: 1 passed.
- `cargo build --package kazeterm --bin kazeterm` passed.
- This visual-only menu iteration did not rerun workspace-wide tests or perform hands-on desktop review.
