# Implement Per-Window Tab Groups

- Added group entities, theme-aware preset colors, tab membership, and UI actions for creation, joining, removal, whole-group reordering, rename, recolor, and deletion in `kazeterm-ui-tree`. Older UI-tree JSON remains deserializable. Group operations maintain nonempty, contiguous groups without pinned tabs.
- Integrated groups into `kazeterm` runtime state, UI-tree reconciliation, workspace persistence/restoration, and both tab-bar layouts. Added group labels and menus, drag-and-drop behavior, grouped pin/duplicate/close handling, and three-way delete confirmation. Cross-window moves transfer the existing terminal entities and PTY and update group metadata according to the destination window.
- **Remove All** does not close terminals. **Close All** respects `close_on_last`. Stale destructive confirmations are canceled when group membership changes.
- Added UI-tree regression tests and fake-session GPUI integration tests for serialization, ordering, grouping, deletion, pinning, restoration, and cross-window transfers. Updated ADR/glossary status.

## Validation Performed

- `cargo fmt --all`, `cargo fmt --all -- --check`, and `cargo check --package kazeterm --bin kazeterm -q` passed. Rustfmt warned that the repository's existing `trailing_comma = Always` requires nightly, but formatting checks passed.
- `cargo test --package kazeterm-ui-tree --lib`: 29 passed.
- `cargo test --package kazeterm --bin kazeterm -- --test-threads=1`: 101 initially passed; the final `cargo test --workspace -- --test-threads=1` run included 103 app tests and 29 UI-tree tests, all passing.
- `cargo test --workspace -- --test-threads=1`, `git diff --check`, and `git diff --no-index --check` for new files passed.
- `cargo clippy --package kazeterm-ui-tree --package kazeterm --all-targets -- -D warnings` did not pass because of pre-existing `SearchState`/`KeyDebugState` default-derivation suggestions and several existing `redundant_clone` warnings. A redundant clone flagged in a new test was fixed. `cargo clippy --package kazeterm --bin kazeterm -- -A clippy::redundant_clone` passed with other existing warnings. Unrelated code was left untouched.
- Hands-on desktop review of drag hit areas, menus, and dialogs was still pending at this stage.
