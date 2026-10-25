# crates/terminal/core/ (`terminal-kernel`)

## Responsibility

Headless terminal contracts and session resources shared by the presentation
layer and concrete backends. No dependency on GPUI, config, or theming.

## Design and modules

- `backend.rs`: object-safe `TerminalBackend` port, value-returning snapshots,
  selection state, and the existing `AlacrittyBackend` lock-owning adapter.
- `session.rs`: GUI-independent `TerminalSize`, `SessionOptions`, `PtySender`, and
  `TerminalSession` resource bundle.
- `events.rs`: channel-based `TerminalEventListener` for emulator notifications.
- `pty_info.rs`: PTY foreground-process identity and cached process metadata.
- `protocols/`: OSC 7/7501 observers, keyboard/DSR negotiation, Unix graphics and
  Windows DSR PTY filters, and raw graphics command transport.
- `lib.rs`: common Alacritty grid/event/selection/PTY/ANSI vocabulary and color
  table constants/default-color predicates.

## Flow

1. A backend consumes `SessionOptions` and returns `TerminalSession` plus events.
2. The app transfers the resources into the GPUI terminal model.
3. Input/resize goes through `PtySender`; emulator updates produce snapshots and
   events; protocol observers share bounded status/keyboard state with the view.
4. Rendering-specific colors and GPU images remain outside core.

## Integration

Used by `terminal`, both concrete backends, and the app's session adapter.
Core/backend tests compile without the GPUI dependency graph.
