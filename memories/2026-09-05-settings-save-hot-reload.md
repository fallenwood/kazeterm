# Settings save and immediate hot reload

- Kept explicit Save behavior as requested; editing a draft does not change the
  running configuration.
- Confirmed that successful saves already call the shared configuration hot-reload
  application path. Updated page messaging and README to state that live settings
  apply immediately after saving, without leaving the page or restarting terminals.
- Reload the exact saved file through `ConfigFile::load_effective`, reusing the
  runtime loader and preserving import precedence instead of independently resolving
  the default configuration path again.
- Added an actual file-save integration test with two registered windows and fake
  terminal sessions. It covers draft isolation, persisted values, imported overrides,
  theme/font updates, both windows' tab layout updates, and preservation of the
  settings page and existing terminal entities.

## Validation

- `cargo fmt --all` and `cargo fmt --all -- --check` completed.
- The focused `settings_save_hot_reloads_open_windows_without_restarting_terminals`
  test passed with `cargo test --package kazeterm --bin kazeterm`.
- `cargo test --workspace -- --test-threads=1` passed: 390 tests, one ignored doctest.
- `cargo check --package kazeterm --bin kazeterm` passed.
- `git diff --check` passed; temporary settings fixtures were removed.
- Stable-rustfmt and MSVC linker warnings remain. Rust also reported denied access
  while finalizing some incremental caches, without failing compilation or tests.
- No live UI automation or cross-platform execution was performed. No real user
  configuration files or terminal processes were changed by the integration test.
