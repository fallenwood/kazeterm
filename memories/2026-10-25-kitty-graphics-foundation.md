# Kitty graphics: static direct-transfer foundation

## Reference and scope
- User requested Kitty protocol improvements and selected graphics first.
- Read Kitty's official `docs/graphics-protocol.rst`, relevant loading/deletion/response sections of `kitty/graphics.c`, and the corresponding `kitty_tests/graphics.py` cases. Reference commit: `617ddee75b02e28c0958e18752c78e22d53299da`; fetched pinned copies and verified that they match the inspected master files.
- This iteration improves static direct transfers, parsing, resource bounds, identifiers, and deletion. It does not claim full Kitty graphics compatibility. Detailed status and follow-up boundaries are in `docs/kitty-graphics.md`.

## Changes
- Corrected the default action to transmit-only (`a=t`), distinguished image ID `i`, image number `I`, and placement ID `p`, and corrected the existing response encoder's placement key to `p` (the response encoder is not yet connected to PTY replies).
- Replaced permissive parsing with checked `Result<Option<KittyCommand>, String>` parsing. Invalid control values, malformed base64, unsupported actions/media/formats, virtual/relative placements, and ID/number conflicts no longer silently become default RGBA/display commands.
- Added bounded chunk accumulation, 4096-byte encoded chunks, first-chunk metadata preservation, continuation validation, failed-upload draining, and delete/reset cancellation. Worker output keeps the final chunk's captured cursor position.
- Added `o=z` via an explicit workspace-inherited `flate2` dependency (already present transitively). Compressed RGB/RGBA and PNG validate exact inflated size, stream completion, trailing bytes, and memory quota; compressed PNG requires `S`. PNG decoding no longer falls back to other formats.
- Enforced the 320 MiB quota before raw pixel conversion/decompression allocations, bounded Unix APC retention at 8 KiB, and tested oversized APC consumption through a split terminator without leaking payload into terminal text. These bounds do not bound the existing raw event queue globally.
- Added newest-surviving image-number lookup independent of LRU order. Auto IDs skip occupied IDs, including after wraparound. Replacement releases its old allocation before quota eviction, and number indexes are cleaned up on removal/eviction.
- Extracted terminal graphics application into `crates/terminal/view/src/terminal/graphics.rs`. Replacement removes old placements, eviction collects stale placements, and anonymous placements ignore client placement IDs.
- Implemented scoped deletion selectors `a/i/n/c/p/q/r/x/y/z` and uppercase variants: rectangle intersection, one-based cells, placement-ID filtering, inclusive ID ranges, visible-only delete-all, lowercase data retention, and uppercase freeing only when no placements remain. ID/number/range uppercase deletion also handles unplaced images.
- Removed speculative first-chunk cursor movement and invented padding from the Unix PTY filter. `C=1` requests no feedback; successful placement still uses the existing deferred row-only CNL mechanism, not full protocol cursor positioning.
- Updated affected codemaps and architecture documentation. No keyboard protocol or low-level renderer changes.

## Regression coverage
Before implementation, four new regressions failed against the previous code: incorrect default action/placement key, order-dependent deletion target, auto-ID collisions, and replacement evicting an unrelated image.

Current tests additionally cover zlib/raw/PNG pixel equivalence, malformed/truncated/trailing/oversized compressed data, format strictness, image-number creation order versus LRU, failed upload recovery, compressed worker chunk handling/final-cursor capture, scoped/uppercase deletion, and Unix APC bounds. Terminal tests use fake sessions; transport tests use Unix socket pairs, not shells or PTYs.

## Validation
Passed on Linux:
- `cargo fmt --all`
- `cargo fmt --all -- --check`
- `cargo test -p terminal-kernel --locked --quiet` (33 tests)
- `cargo test -p terminal --lib graphics --locked --quiet` (26 focused tests)
- `cargo test --workspace --locked --quiet` (one existing ignored doctest)
- `cargo build --workspace --locked --quiet`
- `cargo clippy -p terminal-kernel -p terminal --all-targets --locked --quiet` (warnings, no errors)
- `pwsh -NoProfile -File scripts/check-workspace.ps1`
- `pwsh -NoProfile -File scripts/test-workspace.ps1` (all five groups)
- `git diff --check` and review of modified/new source and documentation.

The initial broad focused-test invocation timed out during compilation; subsequent `--lib` focused checks and full workspace checks completed successfully. Existing Clippy/unused-import/dead-code warnings remain. Full-workspace Clippy, native Windows/macOS builds, interactive image/real-PTY smoke tests, and Kitty's own Python suite were not run. Changes are not committed.

## Next priorities / do not overclaim
1. Ordered query, acknowledgement, error, and quiet responses: `a=q` must complete before a following DA reply. The PTY currently answers DA synchronously while graphics decoding/application is asynchronous. A UI-only asynchronous reply would falsely claim protocol detection compatibility. Background-tab/worker startup also needs coordination. Currently errors are logged/dropped and no graphics responses are sent.
2. Rendering parity: crop regions, single-axis sizing, native pixel sizing, letterboxing, full z-order semantics, and exact cursor row/column movement ordered relative to subsequent text.
3. Ordered clear/reset, alternate-screen lifetime, scroll margins/history, bounded whole-pipeline queues, and separation of anonymous internal IDs from client-addressable IDs.
4. Windows and VTE graphics interception, followed by security-conscious file/shared-memory media, virtual/relative placements, animation, and indexed colors as separate work.
