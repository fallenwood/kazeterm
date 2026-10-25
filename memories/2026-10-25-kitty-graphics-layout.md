# Kitty graphics: static placement geometry and image layers

## Scope and reference
Continued on top of the uncommitted graphics foundation and ordered-response work, preserving it. Reference remains Kitty `617ddee75b02e28c0958e18752c78e22d53299da`: `update_src_rect`, `update_dest_rect`, `grman_update_layers`, `grman_resize`, `find_or_create_image`, and the pinned `test_image_put` / `test_graphics_put_with_pixel_offsets` tests. Current status is in `docs/kitty-graphics.md`.

## Changes
- Added `crates/terminal/view/src/kitty_graphics/geometry.rs` to resolve static crop intersections, native pixel sizes, single-axis aspect-ratio scaling, explicit rectangle sizing, and in-cell offsets. Zero crop extents select the remaining source area; empty intersections are invisible without division by zero. f64 intermediates avoid u32 control-value multiplication overflow, and effective counts saturate on conversion.
- Retained requested automatic axes separately from effective cell coverage. Placement application no longer rounds native image pixels to cell dimensions or shrinks wide images to terminal width. Effective coverage follows the pinned update_dest_rect calculation (including its different offset arithmetic from pixel rendering). Existing deferred row-only cursor feedback is not claimed to be fixed.
- Terminal resize events recompute automatic coverage and clamp retained offsets, without scaling native-size image pixels. Fractional logical cell metrics are retained rather than truncated; this is not a claim of verified HiDPI parity.
- Visible placements carry a cropped destination rectangle and full-image transform relative to the viewport, reusing the same RenderImage. GPUI samples the selected crop without allocating cropped copies. Painting clips to complete terminal cells rather than the whole element/gutter/chrome, while keeping the source transform unchanged for partially visible images and smooth-scroll offsets.
- Added internal image creation identities independent of client/storage IDs and LRU order. Explicit replacement retains the surviving identity, deletion/reupload creates a new identity, and identity exhaustion fails without evicting data. Equal-z sorting follows the upstream implementation's internal IDs, not arbitrary numeric client IDs. Anonymous/client-addressable storage namespaces are still not separated.
- Image painting now has three passes: below non-default cell backgrounds for z < INT32_MIN/2, after cell backgrounds but below text for INT32_MIN/2 <= z < 0, and above text for z >= 0. The default terminal background precedes every image layer. Selection, IME, and cursor policies were otherwise preserved.
- Updated graphics status, architecture, and the terminal view codemap. No dependency, persisted format, config, keyboard, core/backend, or platform-interception changes were needed in this iteration.

## Reference discrepancies
The pinned protocol prose promises letterboxing for both explicit axes, but its grman_update_layers and test_image_put use the supplied rectangle directly. We follow the actual implementation/test (possibly nonuniform scaling), not invented letterboxing. Similarly, equal-z ordering uses upstream internal IDs despite ambiguous prose about the lower image ID. These differences are documented explicitly.

## Regression coverage
- A fake-session regression failed before the fix: c=3 produced a 50-column automatic layout. It now covers c-only, r-only, cropped, and oversized-native placements.
- A second regression failed when sorting by client ID rather than Kitty internal creation order; it now passes with reversed numeric client IDs.
- Pure tests cover native/fractional pixel sizes, both explicit axes, crop/source transforms, default/oversized/out-of-bounds crop regions, offset clamping, auto-axis coverage, resize behavior, extreme u32 controls, internal identity replacement/deletion/LRU/exhaustion, viewport visibility, all z-layer thresholds, and paint clipping without changing source sampling.
- These tests use fake sessions or pure geometry, not a real shell, PTY, or native image window.

## Validation
Passed on Linux:
- `cargo fmt --all` and `cargo fmt --all -- --check`
- `cargo test -p terminal --lib graphics --locked --quiet` (41 focused tests)
- `cargo test --workspace --locked --quiet` (one existing ignored doctest)
- `cargo build --workspace --locked --quiet`
- `cargo clippy -p terminal --all-targets --locked --quiet`
- `pwsh -NoProfile -File scripts/check-workspace.ps1`
- `pwsh -NoProfile -File scripts/test-workspace.ps1` (all five groups)
- `git diff --check` and source/caller review.

Existing unused-import/dead-code/style warnings remain; a newly exposed items-after-test-module warning was fixed by moving the new bounds tests to the file end. Native Windows/macOS builds, interactive GUI/real-PTY/alpha/HiDPI checks, and Kitty's own Python suite were not run. All graphics work remains uncommitted.

## Remaining work
Full emulator/graphics serialization, cursor row/column movement, alternate-screen lifetime, reflow, scrolling/margins, selection/cursor composition parity, global raw-queue limits, anonymous/client-ID namespace separation, Windows/VTE interception, additional media, placeholders, relative placements, and animation are still follow-up phases. Do not describe the new static geometry and basic image-layer passes as complete Kitty graphics or visual compatibility.
