# Settings theme thumbnail gallery

## Changes
- Added `apps/kazeterm/src/components/settings_page/theme_gallery.rs` for cached theme palettes, miniature terminal scenes, responsive cards, and focused tests.
- Replaced the Theme text dropdown with a gallery of every entry returned by `config::list_available_themes()`. This includes the seven built-in themes and themes in the active custom directory. Custom files retain the existing loader priority.
- Each card shows the theme's real title-bar, terminal background/foreground, sample ANSI text, and color swatches. Preview backgrounds are opaque for consistent comparison across themes.
- Cards have a selected border and check mark, native button accessibility/keyboard handling, and disabled interaction during loading/saving. Unreadable themes show Preview unavailable instead of a misleading fallback thumbnail.
- Moved Theme mode ahead of the gallery. Dark/light/system drafts select the cached palette without reading files during rendering. Missing light variants follow the same fallback as the application's theme loader.
- Kept a theme-name input for custom/unknown names. Clicking a card changes only the settings draft; Save changes still applies the configuration.
- Theme catalogs and both palettes are loaded in the existing background load/reload task. Catalog refresh uses the active installed-theme directory; an edited custom directory takes effect through the existing save/apply and reload flow.
- Added viewport/panel selectors for containment regressions. No dependencies, generated images, config schema, Cargo.lock, or terminal-session changes.

## Tests
- Checked responsive columns and light/dark/system mode selection, including Vibrant Light/Dark, missing light variants, and unavailable previews.
- Checked all catalog entries at 600/1000/1400-pixel window widths, visible card/panel/viewport containment, selection without changing the app global, and disabled selection during save.
- Verified real light-palette paint output by inspecting nested scene layers. GPUI compiles rounded clipping into offscreen scenes, so checking only the top-level quad list does not inspect the thumbnail pixels. Production rendering and retained layers are unchanged.

## Ablation study
- Baseline: both gallery tests passed.
- Temporarily mapped the light preview branch to the dark variant. `cargo test --locked --package kazeterm theme_gallery_columns_and_mode` failed on the light-mode assertion, as expected.
- Restored the branch, then passed all focused and workspace tests. No temporary diagnostics or ablated behavior remain.

## Validation
- `cargo fmt --all` and `cargo fmt --all -- --check`: passed.
- `cargo test --locked --package kazeterm theme_gallery`: passed (2 tests).
- `cargo test --locked --package kazeterm settings_ --quiet`: passed (29 tests).
- `cargo test --locked --workspace --quiet`: passed, including all 126 application tests; one existing doctest remains ignored.
- `cargo build --locked --workspace --quiet`: passed on Windows.
- `pwsh -NoProfile -File scripts/check-workspace.ps1`: passed.
- `git diff --check`: passed. Reviewed the gallery module and integration diff.
- Existing remote-control dead-code warnings and fake-session unused import were not changed.

## Not run
- Manual GUI preview, Linux/macOS builds, release builds, and Clippy were not run.
