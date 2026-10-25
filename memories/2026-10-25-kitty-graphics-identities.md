# Kitty graphics: anonymous/client identity isolation

## Scope and reference
Continued from committed graphics implementation `0bef9f7`. The user's general "kitty control protocol" request was interpreted in the ongoing graphics-control context, not as authorization to add Kitty's DCS remote-control command API. This iteration fixes image lookup/replacement/deletion semantics and anonymous resource lifetime. No core/backend, keyboard, remote-control API, dependency, persisted configuration, or platform-interception changes.

Reference remains Kitty `617ddee75b02e28c0958e18752c78e22d53299da`, specifically `find_or_create_image`, `get_free_client_id`, `handle_add_command`/`add_trim_predicate`, `id_filter_func`, `id_range_filter_func`, `filter_refs`, `handle_delete_command`, and the image/placement identity sections of the pinned specification.

## Changes
- `KittyImageStorage` now keys allocations by existing u64 internal creation identities, with a separate nonzero client-ID index. Anonymous uploads retain client ID zero and get distinct storage identities, rather than silently occupying client IDs that could be guessed.
- Numbered uploads obtain the smallest free nonzero client ID, matching upstream `get_free_client_id`. Freed IDs can be reused; anonymous allocations do not reserve them. Number lookup, client lookup, and LRU eviction are independent, while all allocations share the same storage quota.
- `store_prepared` returns the internal storage identity (u64); terminal acknowledgements explicitly resolve the client ID (u32). The existing `store` convenience API returns the client ID, including zero for anonymous uploads. Public client get/peek/remove never match zero.
- Placements retain internal identity separately from their client ID. Replacement, resource lookup, GC, visibility, and resize use internal identity. Reusing a deleted/evicted client ID cannot revive stale placements. Anonymous placement IDs remain ignored.
- ID/number/range deletion resolves only the client namespace; zero-ID and ID ranges including zero do not select anonymous images. Geometry/z/cell selectors still operate on anonymous placements. Matched anonymous allocations are freed when their last placement disappears even for lowercase deletion, matching Kitty `filter_refs`.
- Unplaced anonymous data is pruned before another completed upload, following the resource-lifetime intent of `add_trim_predicate`. Partial transfers remain parser-owned, not stored render resources.
- Updated `docs/kitty-graphics.md`, architecture documentation, and the terminal view codemap to remove the previous shared-ID namespace limitation while preserving the remaining compatibility boundaries.

## Regression coverage
The first new test failed before implementation: after an anonymous transmit-and-display, a guessed `a=p,i=1` wrongly returned OK rather than ENOENT. It now passes.

Added/expanded coverage includes coexistence with named ID 1, zero-ID and ID-range deletion, missing number selectors, anonymous p-ID isolation, reply client IDs distinct from internal identities, numbered minimum-free-ID allocation/reuse including an occupied u32::MAX ID, anonymous/named quota and LRU eviction bookkeeping, last-placement anonymous cleanup, unplaced anonymous pruning, named replacement, and stale placement GC after deleting/reusing a client ID.

## Native smoke validation
Built and ran the current debug binary on Linux COSMIC/Wayland, Intel Iris Xe/Vulkan, 200% display scale, with temporary HOME/XDG configuration and a dedicated PowerShell 7 real-PTY sender. Existing installed Kazeterm PID 2610 was left untouched.

Four screenshot stages were captured and read/visually inspected:
1. Prior PNG/RGBA/zlib RGB/crop/offset/single-axis/alpha scene still rendered correctly.
2. Prior three-z-layer and equal-z internal ordering scene still rendered correctly.
3. Anonymous red and blue images with the same supplied p=7 coexisted with named green i=1,p=7. A guessed put before named upload returned exactly ENOENT. The named upload acknowledged i=1, not its internal identity; I=9 acknowledged assigned i=2. Zero-ID/range selectors preserved the scene.
4. Full uppercase client-ID range deletion removed named green and the numbered image while anonymous red and blue remained visible.

The strict query-before-DA probe and C=1 DSR-position check also passed. The expanded run sent 155 encoded chunks, no graphics error/panic was observed in final logs, and the app quit with exit code 0. Positioning waits intentionally isolate these tests from the known text/graphics serialization limitation.

Temporary scripts: `/tmp/kazeterm-kitty-identities-smoke.ps1` and `/tmp/kazeterm-kitty-identities-sender.ps1` (PowerShell 7, reused local Linux PTY interop from prior smoke tooling; not repository/CI integration tests).
Artifacts: `/tmp/kazeterm-kitty-identities-smoke-3aad52263a2147c1a515a7264ac1d054/`, containing `basic.png`, `layers.png`, `identities.png`, `identity-delete.png`, protocol JSON results, wire log, and application stdout/stderr.

## Validation
Passed on Linux:
- `cargo fmt --all` and `cargo fmt --all -- --check`
- `cargo test -p terminal --lib graphics --locked --quiet` (46 focused tests)
- `cargo test --workspace --locked --quiet` (92 terminal library tests; one existing ignored doctest)
- `cargo build --workspace --locked --quiet`
- `cargo clippy -p terminal --all-targets --locked --quiet`
- `pwsh -NoProfile -File scripts/check-workspace.ps1`
- `pwsh -NoProfile -File scripts/test-workspace.ps1` (all five regression groups)
- The native four-scene smoke run above, source/caller review, and `git diff --check`.

Existing unused-import/dead-code/style warnings remain. Native Windows/macOS builds, Kitty's own Python suite, resize/scroll/alternate-screen image smoke tests, and full visual/HiDPI parity were not validated. Changes remain uncommitted.

## Remaining work
Full emulator/graphics serialization and row/column cursor movement, screen buffers/reflow/scroll margins, bounded raw FIFO, quota eviction preference for unplaced named images, Windows/VTE interception, other transmission media, placeholders, relative placements, and animation remain separate phases. The namespace fix is not full Kitty graphics compatibility and does not implement Kitty remote control.
