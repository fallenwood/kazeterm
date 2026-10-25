# crates/terminal/backends/vte/ (`terminal-kernel-vte`)

## Responsibility

Linux-only VTE-parser emulator, PTY loop, and headless session factory. Source
and dependency declarations are target-gated; other targets build an empty crate.

## Design

- `VteTermInner` owns grid/history/cursor/mode/selection/palette state and
  implements `vte::Perform`.
- `VteBackend` wraps shared mutex-protected state behind `TerminalBackend`.
- `VteEventLoop` multiplexes PTY reads with input/resize/shutdown messages and
  observes the shared OSC 7501 protocol.
- `VtePtySender` adapts that message channel to the core's `PtySender` port.

## Flow

1. `create_terminal_session` consumes shell launch data and `SessionOptions`.
2. It initializes the emulator, PTY, CWD/status observers, and event loop.
3. PTY output updates the emulator; snapshots/events are consumed by the view.
4. The factory returns `(TerminalSession, SessionEvents)`, never a GPUI model.

## Integration

Its only workspace dependency is `terminal-kernel`; no config/view/theme/GPUI
imports. The executable selects this backend only on Linux. Workspace builds on
other targets no longer need an aggregation crate or manual VTE exclusion.
