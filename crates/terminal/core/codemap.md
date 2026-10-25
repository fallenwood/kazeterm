# crates/terminal/core/ (`terminal-kernel`)

## Responsibility

Headless terminal contracts and session resources shared by the presentation
layer and concrete backends. No dependency on GPUI, config, or theming.

## Design and modules

- `backend.rs`: object-safe `TerminalBackend` port, value-returning snapshots,
  selection state, and the existing `AlacrittyBackend` lock-owning adapter.
- `session.rs`: GUI-independent `TerminalSize`, `SessionOptions`, `PtySender`, and
  `TerminalSession` resource bundle.
- `shell.rs`: shared `ShellLaunch` preparation for terminal/backend environment
  defaults, user overrides, shell prompt hooks, and PTY options; no process or
  file is created during preparation.
- `events.rs`: channel-based `TerminalEventListener` for emulator notifications.
- `pty_info.rs`: PTY foreground-process identity and cached process metadata.
- `protocols/`: OSC 7/7501 observers, keyboard/DSR negotiation, Unix graphics and
  Windows DSR PTY filters, and raw graphics command transport. Unix APC capture
  is bounded; cursor feedback comes from validated placements, never a height
  estimate on the first upload chunk. A typed async `RawGraphicsEvent` FIFO
  orders commands, clear/reset markers, and deferred filter replies, so DA does
  not overtake a graphics probe; no GPUI work or UI waits occur in the filter.
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
