# Kitty graphics: static transfers, layout, and ordered replies

This is an incremental implementation, **not full Kitty graphics compatibility**.
The reference for these iterations is Kitty commit
[`617ddee75b02`](https://github.com/kovidgoyal/kitty/tree/617ddee75b02e28c0958e18752c78e22d53299da):

- [Official protocol specification](https://github.com/kovidgoyal/kitty/blob/617ddee75b02e28c0958e18752c78e22d53299da/docs/graphics-protocol.rst)
- [`kitty/graphics.c`](https://github.com/kovidgoyal/kitty/blob/617ddee75b02e28c0958e18752c78e22d53299da/kitty/graphics.c), particularly loading, image-number lookup, deletion, `update_src_rect`, `update_dest_rect`, `grman_update_layers`, and `grman_resize`
- [`kitty_tests/graphics.py`](https://github.com/kovidgoyal/kitty/blob/617ddee75b02e28c0958e18752c78e22d53299da/kitty_tests/graphics.py), particularly `test_load_images`, `test_gr_operations_with_numbers`, `test_gr_delete`, `test_image_put`, and `test_graphics_put_with_pixel_offsets`

## Implemented

- Default action is `a=t` (transmit without displaying). `a=T` transmits and
  displays; `a=p` places an existing image.
- `i` is an image ID, `I` is a non-unique image number, and `p` is a placement ID.
  Uploading with a number creates a new image; subsequent number-based commands
  select the newest surviving upload, independently of LRU access order.
  Number-based uploads receive the smallest free nonzero client ID, as in
  Kitty's `get_free_client_id`; deleted client IDs can be reused.
- Direct `t=d` transfers decode RGB (`f=24`), RGBA (`f=32`, the default), and PNG
  (`f=100`). PNG commands do not fall back to accepting another image format.
- `o=z` supports zlib-compressed raw pixels and PNG. Raw dimensions determine
  the exact inflated size; compressed PNG requires nonzero `S`. Truncation,
  corrupt streams, trailing compressed data, and incorrect sizes are rejected.
- Each base64 chunk is limited to 4096 bytes. A completed upload uses metadata
  from its first chunk and the cursor position from its final chunk. Continuation
  chunks carry only `m` and optional `q`. No image is produced from a partial or
  invalid transfer; failed uploads are drained until their final chunk or a new
  explicit command. Delete commands abort partial uploads.
- The cumulative decoded transfer, inflated data, and per-image pixel allocation
  are bounded at 320 MiB. The Unix PTY filter retains at most 8 KiB per APC,
  consumes excess bytes through ST, and lets the decoder reject the command.
  These are per-command/image limits, not a global bound on the raw event queue.
- Lowercase deletion removes matching placements and retains named image data.
  Anonymous data is freed when its last placement is removed, because it has no
  client-addressable reuse path. Uppercase deletion also frees affected named
  data when no placements remain.
  Supported selectors are `a`, `i`, `n`, `c`, `p`, `q`, `r`, `x`, `y`, `z`
  and their uppercase variants. Cell selectors are one-based and test rectangle
  intersection, not just the placement origin. Delete-all affects visible
  placements rather than erasing scrollback and all cached images.
- Replacing an explicit image ID removes its old placements. Number-assigned IDs
  skip occupied client IDs. Replacing a stored allocation accounts for the released
  memory before evicting unrelated images; evicted image placements are collected
  by internal identity so reusing a client ID cannot revive stale placements.
- Images with neither `i` nor `I` keep client ID zero and distinct internal storage
  identities. They receive no replies and ignore placement IDs. Guessed nonzero
  IDs, `i=0`, and ID-range deletion cannot select them; explicit uploads using
  the same numeric value as their internal identity cannot overwrite them.
  Unplaced anonymous data is pruned before the next completed upload, following
  Kitty's `add_trim_predicate`. Named/numbered/anonymous allocations share the
  same storage quota and LRU, but not the client lookup namespace.
- `C=1` does not request cursor advancement. The filter no longer guesses image
  height or advances the cursor on an unvalidated first chunk, and no longer adds
  two non-protocol padding rows.

### Query, acknowledgement, and error responses

- `a=q` validates the complete payload with the same bounded decoder as uploads,
  then replies without creating a render image, storing/replacing an image,
  changing LRU order, creating placements, or advancing the cursor. A query
  requires a nonzero explicit image ID; its response carries the probe ID.
- Identified uploads acknowledge only after storage succeeds. Placement replies
  report the resolved image ID, optional image number `I`, and placement ID `p`;
  missing images report `ENOENT`. Storage quota failures report `ENOSPC` rather
  than `OK`. Anonymous uploads and successful deletion commands do not reply.
- `q=1` suppresses success responses; `q=2` suppresses both success and errors.
  Errors retain the first chunk's identity/quiet policy, with later nonzero quiet
  updates honored even when a chunk is invalid. Invalid header values do not
  discard valid reply IDs or a quiet key appearing later in the header.
- Decode/parse errors preserve error codes instead of being hardcoded as
  `ENOENT`. Response messages are bounded to 256 printable ASCII bytes and cannot
  reflect escape sequences or control characters to the client.

### Static placement layout and painting

- `x,y,w,h` select the intersection with the source image; zero extents mean
  the remaining source area. Empty intersections produce no draw command and
  do not divide by zero. Cropping reuses the stored texture through separate
  destination-clip and full-image transforms, not another decoded allocation.
- With neither `c` nor `r`, the selected source is drawn at native pixel size,
  without rounding the image itself up to whole cells or shrinking wide images
  to the terminal width. Rendering clips to the cell grid, excluding the gutter,
  scrollbar, minimap, and incomplete trailing cells. Viewport clipping keeps the
  source transform unchanged, including images beginning above the viewport.
- A single `c` or `r` scales the other pixel axis by the cropped source aspect
  ratio. `X,Y` are clamped inside the starting cell; explicitly sized axes end
  at the cell rectangle boundary rather than extending it by those offsets.
  Effective cell coverage follows the pinned implementation's calculation for
  deletion and existing cursor feedback; it is separate from pixel draw size.
- Requested automatic axes are retained. Font/cell-size changes recompute cell
  coverage and clamp offsets without resizing native pixels. This does not
  implement text reflow, screen-buffer switching, or scroll-margin integration.
- Images are sorted by z-index and then **internal creation identity**, as in
  `grman_update_layers`, not by client IDs or LRU access. Replacing a surviving
  explicit image retains this identity; deleting and uploading it again creates
  a new identity. Storage, placements, rendering, and LRU use internal identities;
  client IDs are resolved separately for selectors and acknowledgements.
- `z < -1073741824` paints before non-default cell backgrounds;
  `-1073741824 <= z < 0` paints after those backgrounds but before text;
  `z >= 0` paints above text. The terminal's default background precedes all
  image layers. Existing selection, IME, and cursor policies are retained.

**Reference/documentation differences:** The pinned specification says both
explicit axes should letterbox/pillarbox, but its `grman_update_layers` and
`test_image_put` use the supplied destination rectangle directly. This
implementation follows that code/test behavior (including potentially unequal
horizontal/vertical scale), not automatic letterboxing. Likewise the prose's
same-z "lower id" ordering is implemented upstream using internal image IDs,
not the arbitrary numeric client IDs. No full visual/HiDPI parity is claimed.

### Ordered transport and lifecycle

The core sends a typed `RawGraphicsEvent` FIFO containing commands, clear/reset
markers, and deferred filter-generated replies. Once graphics work is queued,
subsequent DA, XTVERSION, keyboard, private DSR, and OSC 7501 detection replies
from that filter use the same FIFO. The decoder and terminal consumer preserve
this order, so the standard `a=q` followed by `CSI c` detection probe returns the
graphics result before DA, both within one read and across reads. Before graphics
work starts, ordinary standalone filter replies remain synchronous; a closed
graphics consumer also falls back without waiting on the UI.

The raw receiver is an async channel, and decoding runs as a cancellable task on
GPUI's background executor. The terminal model owns both decoder and consumer
tasks. `TerminalView::new` starts them before any paint, including background or
unpainted views. View recreation/window rebinding does not recreate the decoder
or reset a partial upload. Response-only batches do not invalidate render state.
The same production future runs on GPUI's deterministic test scheduler; ordinary
Rust worker tests drive it on smol without a window.

Clear/reset markers are emitted when their escape sequence completes, rather
than by scanning the end of an entire read. Thus a split `CSI 2 J`/`CSI 3 J` or
RIS clears at its proper position before later graphics commands in the same
read, and cancels an incomplete upload in the decoder.

## Boundaries and remaining work

- Graphics interception is still the Unix Alacritty path. The Windows ConPTY
  filter and the Linux VTE backend do not yet provide this graphics pipeline.
- Ordered replies cover the Unix filter's replies and graphics results. Normal
  passthrough text/emulator mutation is **not** fully serialized with graphics;
  replies generated by the emulator itself (for example regular DSR, color or
  size queries) still travel through `SessionEvents`. Full FIFO across all VT
  operations requires a coordinated emulator/graphics boundary. This iteration
  fixes the graphics-query/DA detection ordering, not every VT operation.
- `t=f/t/s`, animation actions, indexed-color formats, virtual placements, and
  relative placements are rejected rather than silently treated as direct static
  RGBA transfers. No arbitrary file or shared-memory access has been added.
- Static crop/size geometry and basic three-layer z-index painting are now
  implemented. A native Linux Wayland smoke test visually confirmed direct PNG,
  chunked RGBA, zlib RGB, source crops, single-axis sizing, offsets, alpha blending,
  and three image layers (see below). Broader image sampling/fractional-metric
  and HiDPI parity still need validation. Selection/cursor composition and screen
  reflow/scroll lifecycle are not claimed to match Kitty. Letterboxing is
  deliberately not added where the pinned implementation stretches the requested
  rectangle.
- Cursor movement still uses the existing deferred row-only CNL feedback. Full
  row/column placement movement, ordered processing relative to subsequent text,
  screen-buffer switching and scrolling at margins still need that coordinated
  boundary, not more metadata guesses in the PTY reader.

## Native image smoke test

On Linux COSMIC/Wayland with Intel Iris Xe/Vulkan and a 200% display scale,
PowerShell 7 sent actual Kitty APCs through a real PTY to the current debug build.
Two native-window screenshots were captured and visually inspected:

- A chunked PNG of the Kazeterm icon, an uncompressed RGBA quadrant fixture,
  and a zlib-compressed RGB fixture displayed with the expected colors.
- Native sizing, red/blue source crops, row-only aspect-ratio sizing, in-cell
  offsets, and a 50% white-alpha overlay were visible as expected.
- The three z-index groups correctly composed with text and a non-default cell
  background. At equal z, newly created blue image `i=2` covered older red image
  `i=900`, confirming internal-image ordering rather than numeric client order.
- An actual query followed by DA returned `ESC_Gi=31;OK ST` before
  `ESC[?62;22c`. DSR before and after a `C=1` placement returned `ESC[21;58R`
  both times. The application survived 139 encoded chunks across the two scenes
  and quit with exit code 0, without observed graphics errors or panics.

The test used an isolated temporary HOME/config and did not change existing
sessions. It deliberately waited between cursor positioning and image output
because full text/graphics serialization remains unfinished. This is a visual
smoke test at one display configuration, not a pixel-diff comparison with Kitty,
a scrolling/resize test, or comprehensive HiDPI/platform validation. Details and
temporary artifact paths are recorded in
[`memories/2026-10-25-native-kitty-image-smoke.md`](../memories/2026-10-25-native-kitty-image-smoke.md).

A subsequent native run also verified image-identity isolation: two anonymous
red/blue images coexisted with a named green `i=1` image, a guessed put before
that named upload returned `ENOENT`, and an `I=9` upload received client ID 2.
Zero-ID/range selectors preserved all images; an uppercase full client-ID range
removed only the green/numbered images while the anonymous red/blue images
remained visible. The expanded four-scene run sent 155 chunks and retained the
query-before-DA and `C=1` checks. See the
[identity iteration record](../memories/2026-10-25-kitty-graphics-identities.md)
for validation and temporary artifacts.

## Code and tests

- `crates/terminal/core/src/protocols/pty_filter.rs`: bounded APC capture and
  ordered filter replies/clear markers; no speculative graphics cursor movement.
  Tests use Unix socket pairs, not a PTY.
- `crates/terminal/view/src/kitty_graphics/`: checked parsing, bounded decoding,
  numbered storage/internal creation identity, placement geometry in `geometry.rs`,
  viewport resolution/sorting, and off-thread preparation.
- `crates/terminal/view/src/terminal/graphics.rs`: application of validated
  commands to terminal resources, with ordinary Rust tests using fake sessions.
- `crates/terminal/view/src/terminal_element/element_impl.rs`: three-layer image
  painting and grid clipping; pure bounds tests do not require a window.

```sh
cargo test -p terminal-kernel --locked
cargo test -p terminal --lib graphics --locked
cargo test -p kazeterm --bin kazeterm unpainted_terminal --locked
cargo test --workspace --locked
pwsh -NoProfile -File scripts/check-workspace.ps1
```

The tests are local regressions derived from the referenced protocol semantics;
they are not an execution of Kitty's own test suite or a claim of visual parity.
