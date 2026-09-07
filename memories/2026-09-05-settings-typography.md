# Settings typography and concise forms

- Applied the current UI theme's font family and base size to the settings page
  root, including subsequent UI-font changes. Terminal font settings remain
  independent.
- Removed visible category and field descriptions, collection-editor help
  paragraphs, and verbose argument labels. Field description metadata remains
  available to search without being displayed.
- Shortened save feedback and the import-override notice. Error messages,
  unsaved-change confirmations, and all editing controls remain available.
- Extracted the typed page renderer for headless inspection and added a grouped
  regression covering UI typography changes, compact scalar fields, and collection
  rows without explanatory blocks. Updated the README.

## Validation

- The typography regression failed before the fix on the missing explicit UI font.
- `cargo fmt --package kazeterm` completed.
- `cargo test --package kazeterm --bin kazeterm settings_page -- --test-threads=1`
  passed all 20 settings tests.
- `cargo check --package kazeterm --bin kazeterm` passed.
- `git diff --check` passed; the complete focused diff was reviewed.
- Existing rustfmt/linker warnings and incremental-cache access notes remain.
- No dependencies, configuration format, or platform boundaries changed.
  Full-workspace tests and live UI automation were not rerun for this local UI
  adjustment. No real terminal sessions or user configuration were modified.
