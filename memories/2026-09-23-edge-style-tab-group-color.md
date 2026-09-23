# Edge-Inspired Color Treatment for Vertical Tab Groups

- Replaced the group label's thin border/dot with a compact capsule filled with the group color. The label text switches between light and dark according to background brightness; dropping a tab onto the label shows a contrasting outline.
- In the vertical tab bar, a continuous colored rail spans the gap below the label and the gaps between group members. Grouped tabs are indented so the rail sits beside their icons rather than adding a dot before each icon. Ungrouped tabs retain their position and have no group rail. Horizontal groups still use the colored capsule without per-tab dots.
- Kept the neutral active-tab border for grouped tabs and the original accent border for ungrouped tabs. Did not add Edge's collapse arrow or group collapsing, which the first version does not support. Updated the ADR and glossary and extended the light/dark capsule text-color test.

## Validation

- `cargo fmt --all`, `cargo fmt --all -- --check`, `cargo test --package kazeterm --bin kazeterm -- --test-threads=1` (104 passed), `cargo build --package kazeterm --bin kazeterm -q`, and `git diff --check` passed.
- This iteration did not rerun workspace-wide tests or perform hands-on GUI review. The installed version was not replaced.
