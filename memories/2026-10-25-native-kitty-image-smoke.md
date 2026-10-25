# Native Kitty image transmission smoke test

## Request and scope
User asked to actually send Kitty images after the native-window startup/input test. No production Rust, dependency, or configuration change was needed. Existing uncommitted graphics work was preserved; only status documentation and this memory were added/updated.

## Environment and isolation
- Linux COSMIC/Wayland, 2880x1800 output at 200% scale, Intel Iris Xe selected via Vulkan.
- Current `target/debug/kazeterm`, Alacritty backend, isolated temporary HOME/XDG directories, workspace restore/update checks disabled only in temporary config.
- A dedicated PowerShell 7 `-File` profile emitted raw Kitty APCs over a real PTY. Direct payloads were base64-chunked at 4096 characters, with first-chunk metadata and continuation m fields; placements used q=2 and C=1.
- Cursor movement was separated from image output by 160 ms waits, deliberately avoiding any claim that the known text/graphics ordering boundary is fixed.
- The already running installed Kazeterm PID 2610 was not touched. Each test-owned window/process was closed afterward.

## Successful checks
Final strict smoke run and preceding successful run both completed:
- Actual graphics query i=31 followed by DA received exactly `ESC_Gi=31;OK ST` then `ESC[?62;22c`.
- Real DSR responses before/after a placement with C=1 both returned `ESC[21;58R`.
- Actual PNG (repository Kazeterm icon), uncompressed RGBA four-color quadrants, and zlib-compressed RGB displayed with correct visible colors.
- Visually inspected native-size placement, explicit rectangle scaling, red and blue source crops, r-only aspect-ratio sizing, X/Y offsets, and a 50% white-alpha overlay.
- Visually inspected three z-index layers: below non-default backgrounds, above backgrounds/below text, and above text. The colored background occluded the first layer; text remained above the second; the third covered text.
- Equal-z overlap displayed newer blue client image i=2 over older red client image i=900, confirming internal creation order despite reversed numeric client IDs.
- 139 encoded chunks were sent across two scenes; images remained visible, no graphics parse/decode/storage error or panic was observed in the final run, and the application quit with exit code 0.

## Test harness diagnostics
Initial optional reply-reader attempts failed, while images were subsequently confirmed to display. PowerShell boxing of value-type array elements meant assigning fields on `$fds[0]` did not initialize the native poll descriptor correctly. The helper now initializes a local struct before storing it in the array, explicitly reads `/dev/tty`, and configures raw termios mode. Its native libc interop is local Linux smoke-test plumbing embedded in PowerShell, not a new application dependency. After correcting the harness, strict query-order and C=1 checks passed twice. These earlier harness failures were not counted as successful protocol checks.

## Artifacts
Temporary scripts (not repository scripts/CI tests):
- `/tmp/kazeterm-kitty-image-smoke.ps1` — launches/owns the isolated app, waits for stage markers, captures scenes, validates exit, cleans up its process.
- `/tmp/kazeterm-kitty-image-sender.ps1` — fixtures, bounded chunk emission, query/DSR verification, and two staged image scenes.

Final verified artifact directory:
`/tmp/kazeterm-kitty-image-smoke-79c7bbe6c02e41cbad0cdefa806db71b/`

Contains `basic.png`, `layers.png` (window-region captures, both read and visually inspected), `probe.json`, `cursor.json`, `sender-results.json`, stdout/stderr logs, and emitted-wire JSON log. An earlier successful full-desktop capture run is at `/tmp/kazeterm-kitty-image-smoke-04afead6288243ef82853c3f43f2977c/`; prefer the final cropped images when sharing.

## Validation and limits
Ran the PowerShell 7 native smoke test repeatedly, inspected both final screenshot files and response JSON, searched final logs for graphics errors/panics, and confirmed the original app remains running and test-owned processes exited. Updated `docs/kitty-graphics.md`; `git diff --check` passed. No Rust changes were made, so Cargo build/unit/workspace checks were not rerun for this test-only documentation iteration (they passed in the preceding layout iteration).

This is one native visual smoke configuration, not Kitty's Python suite or a pixel-diff comparison. It does not establish physical keyboard/mouse input, arbitrary DPI correctness, exact physical/native-pixel equivalence, resize/reflow, scrolling/margins, alternate-screen behavior, full VT ordering, Windows/VTE interception, or unsupported media/animation compatibility.
