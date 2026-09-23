# Add an Icon to Create Group

- Used the existing `icons/folder.svg` for **Create Group** in the tab context menu, matching the other icon-bearing items. No grouping behavior, dependencies, or assets changed.
- Rebuilt the development executable. Already-running windows must be restarted to show the new icon.

## Validation

- `cargo fmt --all` and `cargo fmt --all -- --check` passed.
- `cargo test --package kazeterm --bin kazeterm menu_builder -- --test-threads=1`: 1 passed.
- `cargo build --package kazeterm --bin kazeterm -q` and `git diff --check` passed.
- This visual-only menu iteration did not rerun workspace-wide tests or perform hands-on GUI review.
