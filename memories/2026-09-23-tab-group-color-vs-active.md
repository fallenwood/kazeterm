# Separate Group Color from the Active-Tab Indicator

- In this intermediate iteration, horizontal and vertical tab bars showed group color as a dot on the group label and on each member tab. Group names returned to the theme's ordinary text color.
- Active grouped tabs used a theme-text-colored border instead of the group color; ungrouped tabs kept their original accent border. The group-color dot remained unchanged when the active member changed.
- Updated the ADR and glossary's color semantics and added a GPUI test covering grouped and ungrouped active-indicator branches.

Subsequent feedback removed the member dots, then changed the group label to a colored capsule and added a rail beside members in the vertical tab bar. See [remove member dots](2026-09-23-remove-group-member-dots.md) and [Edge-inspired group color](2026-09-23-edge-style-tab-group-color.md) for the current appearance.

## Validation

- `cargo fmt --all` and `cargo fmt --all -- --check` passed.
- `cargo test --package kazeterm --bin kazeterm -- --test-threads=1`: 104 passed.
- `cargo check --package kazeterm --bin kazeterm -q` and `cargo build --package kazeterm --bin kazeterm -q` passed.
- `git diff --check` passed. This iteration did not rerun workspace-wide tests or perform a hands-on GUI/theme comparison.
