# Fix the missing theme selection icon

## Cause and fix
- The theme gallery uses `IconName::Check`, which resolves to `icons/check.svg`.
- The application provides its own `AssetSource`. A GPUI Kit icon enum does not make the matching SVG available in that source.
- Added `assets/icons/check.svg` with the same size, stroke, and currentColor convention as existing icons. The existing RustEmbed icon include rule picks it up. No loader fallback or dependency change is needed.
- Expanded the existing status-icon resource test to include selection icons. It loads the real path returned by `IconName::Check` through the application's `AssetSource`, rather than relying on headless scene rendering to report missing SVG assets.

## Ablation / negative control
- With the new regression test but without `check.svg`, the test failed with the exact reported error: `could not find asset at path "icons/check.svg"`.
- With the SVG present, the same test passed. The resource test remains in the code.

## Validation
- `cargo fmt --all` and `cargo fmt --all -- --check`: passed.
- `cargo test --locked --package kazeterm status_and_selection_icons_are_available_from_application_assets`: reproduced the missing resource before the fix, then passed after the fix.
- `cargo test --locked --package kazeterm settings_ --quiet`: passed (29 tests).
- `cargo test --locked --workspace --quiet`: passed, including 126 application tests; one existing doctest remains ignored.
- `cargo build --locked --package kazeterm --quiet`: passed on Windows.
- `pwsh -NoProfile -File scripts/check-workspace.ps1`: passed.
- `git diff --check`: passed. Reviewed the asset test diff and SVG against existing icon conventions.
- Existing remote-control dead-code warnings and fake-session unused import remain unchanged.
- Manual GUI inspection, release builds, Linux/macOS builds, and Clippy were not run.

The earlier uncommitted theme-gallery work was preserved. Restart the application to use the rebuilt asset bundle.
