# Distinguish same-name theme cards

## Cause
The screenshot shows two different theme IDs, `kanagawa` and `two or three`, with different palettes. Both theme files use the display name `Kanagawa`. The gallery showed only that metadata name, so the cards looked like duplicate entries.

## Changes
- Updated `apps/kazeterm/src/components/settings_page/theme_gallery.rs` to group loaded themes by lowercase display name and append the filename-stem ID only for collisions.
- The reported pair now displays as `Kanagawa (kanagawa)` and `Kanagawa (two or three)`.
- Unique names stay unchanged. Both distinct palettes remain available; no themes are merged or removed.
- The existing card text and accessibility labels use the cached disambiguated name. Tooltips, selection, and saved configuration still use the original ID.
- Name processing runs once in the existing background catalog load, not during rendering.
- No user theme/config files, dependencies, Cargo.lock, or config schema changed. Earlier gallery, icon, and directory-reload fixes remain intact.

## Regression and ablation control
- Added one grouped pure test for exact duplicate names, case variants, unique names, unavailable files, stable IDs/order, and unchanged palettes.
- Negative control: with name disambiguation omitted, the new test failed on the two unchanged Kanagawa labels.
- Enabled disambiguation and reran the same test and gallery/UI suites; they passed. Distinct palette values were retained.

## Validation
- Read the supplied screenshot and reviewed the changed catalog preparation and regression test.
- `cargo fmt --all` and `cargo fmt --all -- --check`: passed.
- `cargo test --locked --package kazeterm theme_gallery`: passed (3 tests).
- `cargo test --locked --package kazeterm settings_ --quiet`: passed (30 tests).
- `cargo test --locked --workspace --quiet`: passed, including 128 application tests; one existing doctest remains ignored.
- `cargo build --locked --workspace --quiet`: passed on Windows.
- `pwsh -NoProfile -File scripts/check-workspace.ps1`: passed.
- `git diff --check`: passed.
- Existing remote-control dead-code warnings and fake-session unused import remain unchanged.
- Manual GUI verification of the rebuilt application, Linux/macOS builds, release builds, and Clippy were not run.
