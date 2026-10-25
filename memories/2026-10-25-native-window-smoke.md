# Native window smoke test

User requested a check that the actual window is usable, following the Kitty graphics layout iteration.

## Environment and isolation
- Linux COSMIC Wayland session (`wayland-1`), 2880x1800 display at 200% scale.
- Launched the current `target/debug/kazeterm` with a temporary HOME/XDG config/data/cache directory and `--event-source stdio`.
- Used a temporary PowerShell 7 profile with a real PTY; workspace restore and auto-update were disabled for this test only.
- Existing installed Kazeterm process PID 2610 was left untouched. The test instance quit and no production source/config was changed.

## Results
- Native window created and displayed successfully. A screenshot was read and visually inspected: window chrome, terminal text, PowerShell prompt, startup marker, and command-result marker were visible.
- GPU initialization selected Intel Iris Xe via Vulkan, passing its configuration test.
- Real PTY startup executed a PowerShell command that wrote a startup marker file.
- A `SendTextToTerminal` event traversed app input and the real PTY; PowerShell executed it and wrote a second marker file. This was not a physical keyboard-input test.
- `SnapshotUITree` returned a live window, active terminal tab, and focused terminal pane.
- `Quit` completed within the deadline with exit code 0. No panic or application error was observed.
- Nonfatal environment warnings: DejaVu Sans Mono unavailable (Noto Sans Mono fallback) and an unused Vulkan dzn ICD skipped. The selected Intel Vulkan adapter worked.

## Artifacts
- Temporary automation: `/tmp/kazeterm-window-smoke.ps1`
- Logs, screenshot, and marker files: `/tmp/kazeterm-window-smoke-c4752be96c7c49ed998b4fc3bab3ba69/`

This confirms basic native-window rendering, real-PTY execution, event-driven input, and graceful shutdown. It does not validate physical keyboard/mouse interactions, resizing, Kitty image rendering, alpha/z-order/crop visual parity, or Windows/macOS behavior. No Kitty graphics payload was sent in this smoke test.
