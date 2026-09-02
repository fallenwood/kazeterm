# crates/terminal-kernel-alacritty/

## Responsibility

Assembles Alacritty-backed terminal sessions, translating application configuration into emulator/PTY settings and returning the backend-neutral `terminal::Terminal` plus its event stream.

## Design

- `create_terminal_session` is a Factory that owns session wiring: environment, shell hooks, Alacritty `Term`, PTY, event loop, backend adapter, and event channel.
- `AlacrittyPtySender` adapts Alacritty `EventLoopSender` messages to the `terminal::PtySender` port.
- `AlacrittyBackend` from `terminal-kernel` adapts shared `Term` state to `TerminalBackend`.
- Platform-specific PTY filters add Kitty graphics/CWD processing on Unix and DSR handling on non-Unix targets.

## Flow

1. The caller supplies a shell command, working directory, application config, and version.
2. The factory sets terminal identity, true-color, user environment, OSC 7/CWD hooks, cursor behavior, scrollback, and OSC 52 policy.
3. It creates the PTY and shared Alacritty `Term`, then spawns Alacritty's event loop with the appropriate PTY filter.
4. Input and resize messages travel through `AlacrittyPtySender`; emulator notifications travel through `SessionEvents`.
5. The factory wraps the shared term in `AlacrittyBackend` and returns the assembled `Terminal` and receiver.

## Integration

- Depends on: `config`, `alacritty_terminal`, `terminal`, and `terminal-kernel`.
- Consumed by: the `kazeterm` executable as the general/default concrete terminal session factory.
- Entry point: `src/lib.rs::create_terminal_session`.
