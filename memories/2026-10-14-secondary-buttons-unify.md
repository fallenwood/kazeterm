# Secondary button styling across application pages

## Current iteration
- Continued the filled-primary / outlined-secondary hierarchy established in Settings, using the existing GPUI Kit button variants and global theme tokens.
- Changed Cancel buttons in close, update, Alacritty import, tab rename, and group dialogs from ghost to primary outline.
- Changed Reset to Auto and Check for Updates to primary outline; kept confirmation/save/import/update actions filled.
- Changed Don't Save and Close Group Tabs to danger outline, retaining their destructive variant and existing handlers.
- Changed title-bar tab visibility, new-tab, and dropdown controls to primary outline; added a small gap between outlines. Existing mouse propagation and dropdown behavior are unchanged.
- Changed search previous/next/close controls to primary outline. Search option toggles are outlined while off and filled/selected while on, using native selected/toggled state instead of manually overriding the background.
- Did not alter custom tab-close elements, native window controls, terminal sessions, or the earlier global theme mapping in this iteration.

## Regression coverage
- Added `components::search_bar::tests::option_buttons_keep_theme_state_colors_on_hover`.
- Inspects actual painted backgrounds in a headless search view for off/on/off transitions and hovering each outlined or selected button.
- Temporarily restored the previous ghost/manual-background search option styling: regression test failed (3 outlined buttons instead of 6). Restored the new styling and verified the test passes.

## Validation
- `cargo test --package kazeterm option_buttons_keep_theme_state_colors_on_hover`: passed with new styling; failed as expected with old styling.
- `cargo fmt --all`
- `cargo fmt --all -- --check`: passed; stable rustfmt warns that the existing trailing_comma configuration needs nightly.
- `cargo test --package kazeterm --bin kazeterm`: 107 passed.
- `cargo test --workspace`: passed; existing unused `terminal::PtySender` import warning in `crates/terminal/tests/fake_session.rs`, and one ignored event-system doctest.
- `git diff --check`: passed (Git emitted an LF/CRLF normalization warning for search_bar.rs).
- Reviewed diffs; no dependency, lockfile, configuration schema, or platform-boundary changes.
- No manual screenshot review or Linux/macOS execution was performed.
