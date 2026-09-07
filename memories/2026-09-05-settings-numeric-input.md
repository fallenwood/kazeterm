# Settings numeric input restrictions

- Attached GPUI input-state validators to all numeric settings, using the existing
  field metadata to distinguish integer and decimal inputs.
- Integer inputs accept ASCII digits; decimal inputs also accept a single decimal
  point. Unsupported signs, letters, spaces, separators, and exponent notation
  are rejected before the draft changes.
- Kept empty and partial decimal input editable so clearing, replacing selections,
  and typing fractional values still work. Existing save-time range validation
  remains unchanged, and ordinary text fields are unaffected.
- Added metadata-wide syntax tests and GPUI coverage for invalid edits, IME
  composition, selection replacement, partial numbers, and clipboard paste.
- Updated the existing invalid-value test to use an out-of-range number, since
  typing NaN is now blocked at the input boundary.

## Validation

- `cargo fmt --package kazeterm` completed.
- `cargo test --package kazeterm --bin kazeterm settings_page -- --test-threads=1`
  passed all 17 tests.
- `cargo check --package kazeterm --bin kazeterm` passed.
- `git diff --check` passed.
- Existing rustfmt/linker warnings and incremental-cache access notes remain.
- No workspace-wide rerun was needed for this settings-page-only change.
  No live UI automation was performed.
