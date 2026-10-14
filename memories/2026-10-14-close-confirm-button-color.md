# Close confirm dialog button color update

## What changed
- Updated `crates/kazeterm/src/components/close_confirm_dialog.rs`.
- In the non-restore close flow (`Close Window?` dialog with `Cancel` + `Close`), changed the confirm button variant from `.danger()` to `.primary()`.
- This keeps destructive styling for the `Don't Save` action (restore-workspace flow), while making plain window-close UX visually cleaner.

## Follow-up adjustment
- The `.primary()` fill variant could appear to lose its background on hover under the current theme.
- Updated the same non-restore confirm button to `.primary().outline()` so hover keeps a visible border/shape and no longer looks like it disappears.

## Validation
- `cargo fmt --all`
- `cargo test --package kazeterm close_confirm_dialog`
  - Result: 6 passed, 0 failed.
