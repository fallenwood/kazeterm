# Share shell launch preparation between terminal backends

## Changes
- Added `terminal_kernel::ShellLaunch` in `crates/terminal/core/src/shell.rs`. It prepares `tty::Options` and the CWD-reporting path without creating files, opening a PTY, or spawning a process.
- Both Alacritty and VTE factories now use the shared preparation path instead of duplicating environment defaults, user overrides, OSC 7/133 prompt hooks, PowerShell initialization, and PTY options.
- Preserved existing precedence: terminal/backend defaults, then user environment overrides, then integration variables and shell-specific hooks. VTE still supplies an overridable `VTE_VERSION=7600`; Alacritty supplies no backend defaults.
- Preserved Bash/sh chaining of the inherited `PROMPT_COMMAND`, zsh's hook-only behavior, untouched arguments for ordinary shells, and PowerShell initialization only when no explicit arguments are supplied. Windows argument escaping, PTY error messages, and backend/session ownership are unchanged.
- Added four grouped, ordinary Rust tests covering environment precedence/LANG fallback, POSIX prompt behavior, PowerShell name/argument handling, and constructor wiring without a real shell or PTY. Pure helpers accept inherited environment information explicitly, so branch tests never mutate process-global environment variables.
- Updated `docs/architecture.md` and the affected core/backend codemaps. No dependencies, manifests, lockfile, persisted formats, or UI code changed.

## Validation
Passed on Linux:
- `cargo fmt --all`
- `cargo fmt --all -- --check`
- `cargo test -p terminal-kernel -p terminal-kernel-alacritty -p terminal-kernel-vte --locked`
- `cargo test --workspace --locked --quiet` (one existing ignored doctest)
- `cargo build --workspace --locked --quiet`
- `cargo clippy -p terminal-kernel -p terminal-kernel-alacritty -p terminal-kernel-vte --all-targets --locked --quiet` (warnings only in unchanged protocol/VTE code)
- `pwsh -NoProfile -File scripts/check-workspace.ps1`
- `pwsh -NoProfile -File scripts/test-workspace.ps1` (all five regression groups)
- `cargo check -p terminal-kernel-vte --target wasm32-wasip2 --locked --quiet` (non-Linux empty-crate path)
- `git diff --check` and review of the changed implementation, new module/tests, and documentation.

Workspace tests/build still report existing unused-import/dead-code warnings in the terminal view. Full-workspace Clippy, native Windows/macOS compilation, and interactive GUI/real-PTY smoke tests were not run. Changes are not committed.
