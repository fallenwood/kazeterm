# Kitty graphics: ordered probes, acknowledgements, and errors

## Scope and reference
Continued on top of the existing uncommitted static-transfer foundation; previous work was preserved. Reference remains Kitty `617ddee75b02e28c0958e18752c78e22d53299da`, particularly the query/acknowledgement/quiet specification and `finish_command_response` in `graphics.c`. Current compatibility status is documented in `docs/kitty-graphics.md`.

## Changes
- Introduced core's typed `RawGraphicsEvent` FIFO for raw commands, clear/reset markers, and deferred filter replies. The raw graphics receiver now uses the already-available futures async channel rather than blocking std mpsc; no additional dependency or manifest changes were needed in this iteration.
- Once graphics work is queued, subsequent Unix-filter DA, XTVERSION, keyboard, private DSR, and OSC 7501 detection replies join that FIFO. This prevents the standard graphics query followed by DA probe from receiving DA first, across a single read or multiple reads. Before any graphics work, standalone replies remain synchronous; a closed graphics consumer falls back without waiting for UI work.
- Moved program-status observation into byte-stream order instead of answering later OSC probes in a pre-pass. Clear/reset markers now occur when CSI 2J/3J or RIS completes, fixing split-sequence recognition and ordering before later graphics commands in the same read.
- Added full-payload query validation through the shared bounded decoder, without render-image creation, image storage/LRU mutation, ID allocation, placements, or cursor feedback. Queries respond with their explicit nonzero probe ID.
- Connected upload/placement acknowledgements to actual successful resource application; missing images return ENOENT and storage failures return ENOSPC instead of an OK. Number-based responses include the resolved image ID, image number I, and placement p where applicable. Anonymous uploads and successful deletion commands remain silent.
- Implemented q=1 success suppression and q=2 full suppression for successful commands and errors. Parse errors retain bounded numeric response identity and quiet metadata even if invalid fields precede them, or a later continuation chunk fails. A failed upload produces one error, not one per discarded continuation.
- Added correlated error contexts and bounded printable-ASCII response encoding. Codes such as EINVAL, ENOTSUP, ENODATA, E2BIG, ENOENT and ENOSPC are not all mislabeled ENOENT; response message bodies cannot contain control/escape bytes and are capped at 256 bytes.
- Converted the decoder into an async production future scheduled on GPUI's background executor. The Terminal model retains both decoder and foreground-consumer task handles. TerminalView starts processing before painting; rebuilding a view/window binding does not recreate the decoder or lose a partial upload. Response-only batches do not invalidate render state.
- Updated architecture/status documentation and affected codemaps. Runtime session-port types changed, but persisted formats, configuration, keyboard encoding, renderer code, and backend selection were not changed.

## Regression coverage
Two new tests failed before implementation: the official-style graphics probe received DA before graphics processing, and a valid query produced no reply. Both now pass.

Added/expanded tests cover query validation without image replacement, upload/placement/number reply identity, quiet success/error behavior, invalid-header metadata order, continuation error correlation, storage-failure responses, printable bounded encoding, deferred replies/clears across reads, and consumer-disconnect fallback. A headless GPUI app test owns but never paints a TerminalView, verifies query-before-DA replies, then recreates the view during an upload and verifies the single completion acknowledgement and retained image.

The first version of that GPUI test exposed nondeterministic cross-thread wakes from the old private OS decoder thread. The production worker was changed to an executor-owned async future, not worked around with sleeps, scheduler exemptions, or an ignored test. Ordinary Rust tests drive the identical future on smol; the GPUI test drives it on the deterministic scheduler.

## Validation
Passed on Linux:
- `cargo fmt --all` and `cargo fmt --all -- --check`
- `cargo test -p terminal-kernel --locked --quiet` (36 tests)
- `cargo test -p terminal --lib graphics --locked --quiet` (33 focused tests)
- `cargo test -p kazeterm --bin kazeterm unpainted_terminal --locked --quiet`
- `cargo test --workspace --locked --quiet` (one existing ignored doctest)
- `cargo build --workspace --locked --quiet`
- `cargo clippy -p terminal-kernel -p terminal --all-targets --locked --quiet` (existing warning categories, no errors)
- `pwsh -NoProfile -File scripts/check-workspace.ps1`
- `pwsh -NoProfile -File scripts/test-workspace.ps1` (all five regression groups)
- `git diff --check` and review of changed source, tests, runtime session ports, and documentation.

Native Windows/macOS compilation, interactive real-PTY/image smoke tests, Kitty's own Python suite, and full-workspace Clippy were not run. Existing unused-import/dead-code/style warnings remain. Changes, including the preceding foundation, are not committed.

## Remaining boundaries
The ordering guarantee covers graphics work and Unix-filter replies, including the graphics-query/DA detection sequence. Passthrough text/emulator mutations and replies produced by the emulator itself (regular DSR, color/size queries through SessionEvents) are not yet globally serialized with graphics. Deferred row-only cursor movement, alternate-screen/scroll-margin integration, full rendering/cropping/sizing/z-order parity, anonymous/client-ID separation, a global bounded raw queue, Windows/VTE interception, other media, placeholders, relative placements, animation, and indexed colors remain follow-up work. Do not describe this as full Kitty or VT FIFO compatibility.
