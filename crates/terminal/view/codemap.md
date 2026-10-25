# crates/terminal/view/ (`terminal`)

## Responsibility

Implements the backend-neutral terminal application and presentation layer: PTY input, emulator state coordination, selection/search/scroll behavior, GPUI input handling, custom terminal painting, process metadata, OSC 7 tracking, and Kitty graphics.

## Design

- `Terminal` is the mutable domain model. It owns a `dyn TerminalBackend`, a `dyn PtySender`, interaction state, process/CWD metadata, search state, and graphics storage.
- `TerminalBackend` and `PtySender` are Strategy/Port boundaries: concrete kernel crates supply emulator reads/mutations and PTY writes without leaking implementation-specific locking into the renderer.
- `TerminalView` is a GPUI entity that binds focus, actions, keyboard/IME/mouse/touch events, blink timing, and application-facing `TerminalEvent`s.
- `TerminalElement` is a custom GPUI element. It snapshots terminal content during layout, shapes/batches cells, and paints text, backgrounds, selection, cursor, hyperlinks, scrollbar, minimap, and images.
- `kitty_graphics` owns command decoding, GPU image storage, and placement; PTY filtering and OSC observation live in `terminal-kernel` and are re-exported here for compatibility. `mappings` converts GPUI keys/mouse/colors into terminal protocol representations.

## Flow

1. A concrete kernel returns headless `TerminalSession` resources. The application converts them with `Terminal::from(session)` and wraps the result in a `TerminalView` entity.
2. GPUI actions, key events, IME commits, mouse gestures, and paste are mapped to terminal bytes or queued `InternalEvent`s, then sent through `PtySender`.
3. The kernel event loop updates emulator state and emits terminal events; `Terminal` refreshes content, title, selection, search, graphics, and CWD/process state and notifies the view.
4. `TerminalView::render` creates `TerminalElement`; layout obtains a value snapshot from the backend and produces paint data, then GPUI renders the current grid and overlays.
5. View-level events propagate tab-title, wakeup, close, and command-finished changes to the application.

## Integration

- Depends on: `terminal-kernel` contracts/types, `config` keybindings and terminal settings, `themeing` colors/zoom, and GPUI.
- Constructed by: the application composition root from a backend's `TerminalSession`; backend crates do not depend on this crate.
- Consumed by: `kazeterm` tabs, split panes, search UI, notifications, and terminal session lifecycle.
- Entry point: `src/lib.rs` exports `Terminal`, `PtySender`, `TerminalBounds`, `TerminalView`, and initialization/keybinding registration.
