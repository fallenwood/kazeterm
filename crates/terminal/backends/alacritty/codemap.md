# crates/terminal/backends/alacritty/ (`terminal-kernel-alacritty`)

## Responsibility

Creates cross-platform Alacritty sessions from headless `SessionOptions`,
returning `TerminalSession` resources and an event stream rather than GPUI state.

## Design

- `create_terminal_session` assembles environment/shell hooks, emulator, PTY,
  shared protocol observers, event loop, backend, and event channel.
- `AlacrittyPtySender` adapts `EventLoopSender` to the core's `PtySender` port.
- `AlacrittyBackend` from core exposes snapshots and emulator mutations.
- Core PTY filters provide graphics/CWD support on Unix and DSR/keyboard
  negotiation on Windows.

## Flow

1. The app supplies shell/working directory, `SessionOptions`, and version.
2. The factory applies scrollback, cursor, OSC 52, and environment settings.
3. It starts the PTY/event loop and packages resources in `TerminalSession`.
4. The app moves those resources into `terminal::Terminal`; the backend never
   imports the view or application configuration.

## Integration

Its only workspace dependency is `terminal-kernel`. It remains the default
backend on Linux, macOS, and Windows.
