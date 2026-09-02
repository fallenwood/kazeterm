# Configurable UI transitions

## Configuration

- Added top-level `[animation]` settings: `enabled`, `duration_ms`, `frame_interval_ms`, `easing`, and `fade_start_opacity`.
- Animation timing and opacity are clamped before use; frame count is bounded to avoid runaway repaint loops.
- Added `linear`, `ease_in`, `ease_out`, and `ease_in_out` curves.
- Config migration `20260512.1 -> 20260901.1` adds the default animation table without overwriting existing values.

## Transition coverage

- `TransitionSpec` converts runtime configuration into cancellable GPUI frame loops.
- Window resize and vertical tab-bar width interpolation now use configured timing and easing.
- Visible UI-tree diffs share a root content fade, covering tabs, panes, search visibility, overlays, window state, and reordering at the common reconciliation boundary.
- Direct presentation changes also trigger the shared fade: dialogs, tab switcher, tab context operations and transfers, hidden/focused/swapped panes, and loaded UI-tree replacement.
- Initial window content is not faded. With animation disabled or duration zero, geometry and opacity move directly to final values.

## Verification

- Added configuration clamping/default tests, migration coverage, custom timing/easing/fade coverage, and disabled-animation immediate-state coverage.
- `cargo fmt --all -- --check`, `cargo metadata --no-deps --offline --format-version 1`, and `git diff --check` passed.
- Cargo tests could not run because the sandbox denied creation of the user-level `adler2` crate-cache directory; the escalation request was rejected by an automatic approval service error before tests started.
