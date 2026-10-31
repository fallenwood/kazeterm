# Preserve the default theme directory after saving settings

## Root cause
- The reported `two or three.toml` file exists in the default user theme directory and parses successfully. Spaces in the file stem are valid; no rename or theme-ID conversion is needed.
- Startup used the default `themes` directory beside `kazeterm.toml` when `appearance.themes_path` was absent.
- `config_watcher::apply_loaded_config` instead cleared the custom theme path for the same configuration. Saving a valid default-directory theme therefore removed its source before loading its palette.
- Settings save applies configuration immediately. The file watcher can then apply it again after its 200 ms debounce. Both used the same incorrect path handling, consistent with the two reported warnings.

## Changes
- Added `configure_theme_path` in `apps/kazeterm/src/config.rs` and reused it from startup and configuration reload.
- Missing explicit overrides now resolve to `Config::get_config_path().join("themes")`. This reuses the config owner's Windows/Linux/macOS path rules rather than duplicating platform branches in main.
- Existing explicit directories still take priority. Missing directories or file paths do not retain stale overrides; invalid explicit paths still report a warning.
- The file watcher continues to obtain its active directory from the same registered theme path, so a save no longer removes the default-directory theme watch.
- Extended the settings save/hot-reload integration test to check the resulting directory. Reused the existing test lock for gallery catalog snapshots because save changes the process-global path.
- No user settings or theme files were modified. No warnings were suppressed, and no config schema, dependencies, Cargo.lock, or terminal-session behavior changed.

## Regression and ablation control
- Extracted the old reload behavior (no explicit path means no custom directory) and ran `theme_directory_resolution_preserves_defaults_and_explicit_overrides`.
- The test failed: `None` was returned instead of the existing default `themes` directory.
- Restored the default-directory branch and shared it with startup. The same test passed.
- The fixture uses an isolated temporary directory and checks absent defaults, existing defaults, explicit overrides, removing an override, missing directories, files used as directories, and a valid `two or three.toml` filename. It does not depend on the user's profile contents.

## Validation
- `cargo fmt --all` and `cargo fmt --all -- --check`: passed.
- `cargo test --locked --package kazeterm theme_directory_resolution_preserves_defaults_and_explicit_overrides`: failed with the old behavior, then passed with the fix.
- `cargo test --locked --package kazeterm settings_ --quiet`: passed (29 tests).
- `cargo test --locked --workspace --quiet`: passed, including 127 application tests; one existing doctest remains ignored.
- `cargo build --locked --workspace --quiet`: passed on Windows.
- `pwsh -NoProfile -File scripts/check-workspace.ps1`: passed.
- `git diff --check`: passed. Reviewed startup, reload, helper, and test changes. Earlier gallery and icon changes remain intact.
- Existing remote-control dead-code warnings and the fake-session unused import remain unchanged.
- Manual GUI verification, Linux/macOS builds, release builds, and Clippy were not run.

Restart the rebuilt application, then select and save the custom theme again.
