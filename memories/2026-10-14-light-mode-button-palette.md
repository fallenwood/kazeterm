# Theme-derived neutral button colors in light mode

## Current iteration
- Updated `crates/themeing/src/lib.rs` at the shared GPUI Kit theme bridge rather than adding Settings-only color overrides.
- Light-mode primary/filled variants and outline primary ink now derive from the theme's text color instead of its blue accent. Filled text uses the theme background color with full opacity for readable contrast.
- Light-mode default/dropdown buttons use opaque theme-derived light surfaces with dark text. Ghost hover and pressed states also use light surfaces, because GPUI Kit ghost buttons share the default button's pressed token while retaining dark text.
- Kept the previous dark filled-button accent palette and hover/pressed formulas.
- Synchronized GPUI Kit's resolved light/dark mode before applying our palette, including System mode, and refreshed windows after token/Base synchronization. Switching modes cannot overwrite the mapped colors with component defaults.
- Preserved borderless Settings categories, Back/Open TOML file outlines, Back label/alignment, and the main-toolbar source rollback. No persisted config, terminal palette, platform branches, or dependencies were changed.

## Regression coverage
- Added `settings_button_colors_follow_theme_mode` in `crates/kazeterm/src/components/settings_page/tests.rs`.
- Checks One and Kanagawa through explicit Light/Dark, both resolved System appearances, and switching back to Light.
- Covers token consistency, opaque foreground/light button surfaces at 0.8 app background opacity, actual selected-category repaint, and text contrast of at least 4.5:1 for the tested light primary/default normal, hover, pressed, and ghost-hover color pairs.
- The initial test failed against the earlier accent-based light palette and passed with the fix.
- This does not exhaustively validate every custom theme, outline/disabled/focus state, or composited transparent-window appearance.

## Validation
- `cargo test --package kazeterm settings_button_colors_follow_theme_mode`: expected initial failure, then passed.
- `cargo fmt --all` and `cargo fmt --all -- --check`: passed.
- `cargo test --package themeing`: 3 passed.
- `cargo test --package kazeterm settings_`: 23 passed.
- `cargo test --workspace`: passed; Kazeterm 110 tests passed; one existing event-system doctest ignored. Repeated after making light default surfaces opaque.
- Reviewed the theme/test diffs, confirmed `main_window_render.rs` has no diff, and ran `git diff --check`.
- Nonblocking warnings: stable-rustfmt trailing_comma option; existing unused PtySender import; LF/CRLF warnings; Windows incremental cache finalization reported Access is denied without failing builds/tests.
- Manual GUI/screenshot verification and Linux/macOS execution were not performed.
