# crates/terminal-kernel-vte/

## Responsibility

Provides an alternative terminal kernel built on the `vte` parser, including emulator state, escape-sequence performance, a PTY I/O loop, and an adapter to the shared `TerminalBackend` contract.

## Design

- `VteTermInner` owns grid/history/cursor/mode/selection/palette state and implements `vte::Perform` to apply parsed terminal operations.
- `VteBackend` wraps shared `Arc<parking_lot::Mutex<VteTermInner>>` state and implements `terminal_kernel::TerminalBackend`.
- `VteEventLoop` is an Active Object: one thread multiplexes PTY reads with `Input`, `Resize`, and `Shutdown` messages and feeds bytes through `vte::Parser`.
- `VtePtySender` adapts the event-loop channel to `terminal::PtySender`; `create_terminal_session` is the assembly Factory.

## Flow

1. The factory builds terminal identity/environment and shell CWD/OSC 7 hooks from `config`.
2. It initializes shared VTE terminal state, creates a PTY via the common `terminal-kernel::tty` layer, and spawns `VteEventLoop`.
3. PTY output is parsed into `VteTermInner`; state changes emit common terminal events and OSC 7 updates.
4. GPUI input/resize operations reach the loop as `VteMsg`s through `VtePtySender`.
5. `VteBackend` exposes snapshots and mutations to `terminal::Terminal`; the factory returns that terminal with `SessionEvents`.

## Integration

- Depends on: `config`, `vte`, `terminal`, and `terminal-kernel` (including its PTY and common terminal primitives).
- Consumed by: `kazeterm` on Linux when the VTE kernel is selected; built into default Unix workspace builds through `terminal-kernel-unix-bundle`.
- Entry points: `src/lib.rs` session factory, `src/vte_event_loop.rs` I/O loop, and `src/vte_term.rs` emulator/backend.
