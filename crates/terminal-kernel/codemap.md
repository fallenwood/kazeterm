# crates/terminal-kernel/

## Responsibility

Defines the shared terminal-emulator boundary used by the UI layer and concrete kernels, while exposing a stable common vocabulary for events, grids, selections, PTYs, cells, colors, and renderable snapshots.

## Design

- `TerminalBackend` is an object-safe Adapter/Strategy interface whose value-returning methods hide backend state and locking.
- `RenderableSnapshot` packages cells, mode, viewport offset, cursor, and selection for renderer consumption; `SelectionDisplay` carries backend-neutral selection endpoints.
- `AlacrittyBackend` adapts `alacritty_terminal::Term` behind `Arc<FairMutex<_>>`, shared with the Alacritty I/O event loop.
- Public modules re-export selected `alacritty_terminal` primitives as the workspace's common terminal type dialect, including PTY facilities reused by both concrete kernels.
- Color constants and helpers translate emulator colors to `themeing` colors and identify default foreground/background slots.

## Flow

1. A concrete session factory creates emulator state and exposes it as `Box<dyn TerminalBackend>`.
2. `terminal::Terminal` invokes the interface for resize, scrolling, selection, hyperlink lookup, text extraction, and grid snapshots.
3. The renderer consumes `RenderableSnapshot` values and the standardized 269-slot color table without borrowing backend internals.
4. Backend mutations acquire their own locks, so callers do not coordinate implementation-specific synchronization.

## Integration

- Depends on: `alacritty_terminal` for shared primitives and the built-in adapter; `themeing` for neutral color conversion.
- Consumed by: `terminal`, `terminal-kernel-alacritty`, and `terminal-kernel-vte`.
- Entry points: `src/lib.rs` for re-exports/color helpers and `src/backend.rs` for the backend port and Alacritty adapter.
