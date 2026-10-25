# Reorganize workspace and enforce terminal dependency boundaries

## Changes
- Moved the desktop executable from `crates/kazeterm` to `apps/kazeterm`.
- Grouped terminal packages under `crates/terminal/{core,view,backends/alacritty,backends/vte}` while retaining Cargo package names.
- Centralized internal dependency paths in root `[workspace.dependencies]`; root default builds/runs select the desktop app.
- Moved PTY process information, emulator event forwarding, OSC 7/7501 observers, keyboard/DSR filters, and raw graphics transport into headless `terminal-kernel`.
- Added `SessionOptions`, `TerminalSize`, and `TerminalSession`. The application translates configuration, backends return resources, and the presentation layer takes ownership without recreating a PTY/backend.
- Removed backend dependencies on `terminal`/`config` and core's dependency on `themeing`; removed the unused theme-color conversion helper from core. Existing protocol/PTY exports under `terminal` remain available through re-exports.
- Made VTE source/dependencies Linux-only, removed the obsolete Unix aggregation crate, and unified platform workspace build/test commands in CI.
- Added `scripts/check-workspace.ps1` and five dependency-free PowerShell regression groups in `scripts/test-workspace.ps1`; CI checks dependency direction and centralized paths.
- Added configuration-adapter, dimension-conversion, and session-resource-handoff tests; moved existing protocol tests with their implementations. Snapshot contents are unchanged; only their source-path metadata changed.
- Updated README, agent guidance, event-system references, codemaps/cartography state, and `docs/architecture.md`. Root assets and binary output paths are unchanged.

## Validation
Passed on Linux:
- `pwsh -NoProfile -File scripts/test-workspace.ps1`
- `pwsh -NoProfile -File scripts/check-workspace.ps1`
- `cargo fmt --all -- --check`
- `cargo check --workspace --locked`
- `cargo test -p terminal-kernel -p terminal-kernel-alacritty -p terminal-kernel-vte --locked`
- `cargo test -p kazeterm --bin kazeterm session_options_preserve_backend_settings_and_fallbacks --locked`
- `cargo test --workspace --locked --quiet` (one existing ignored doctest)
- `cargo build --workspace --locked --quiet`
- `cargo clippy -p terminal-kernel -p terminal-kernel-alacritty -p terminal-kernel-vte --all-targets --locked --quiet` (existing warnings)
- `cargo run --locked -- --help`
- `cargo tree -p terminal-kernel-vte --target x86_64-pc-windows-msvc --offline` and the equivalent macOS target check showed no VTE dependencies.
- `cargo check -p terminal-kernel-vte --target wasm32-wasip2 --locked` verified the non-Linux empty-crate path.
- Reviewed rename-aware diffs using a temporary Git index; `git diff --check` passed.

`cargo clippy --workspace --all-targets --locked --quiet` remains blocked by existing denied `redundant_clone` findings in unchanged `crates/kazeterm-ui-tree/src/{reducer,diff,node}.rs`; unrelated source was not modified.

Full Windows/macOS compilation, native packaging, and interactive GUI/real-PTY smoke tests were not run in this Linux environment. Changes are not committed.
