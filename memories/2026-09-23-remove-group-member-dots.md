# Remove Colored Dots from Grouped Tabs

Following user feedback, removed the colored dot previously added after the shell icon of each group member in both horizontal and vertical tab bars. At this point, the group color appeared only as a dot on the group label. The neutral active border for grouped tabs and original accent border for ungrouped tabs remained unchanged. Selection color now depends directly on the tab's `group_id`, without looking up the group's color. Updated the ADR and glossary.

A later iteration replaced the label dot with a colored capsule and added a rail beside members in the vertical tab bar; see [Edge-inspired group color](2026-09-23-edge-style-tab-group-color.md).

## Validation

- `cargo fmt --all`, `cargo fmt --all -- --check`, `cargo test --package kazeterm --bin kazeterm -- --test-threads=1` (104 passed), `cargo build --package kazeterm --bin kazeterm -q`, and `git diff --check` passed.
- This iteration did not rerun workspace-wide tests or perform hands-on GUI review.
