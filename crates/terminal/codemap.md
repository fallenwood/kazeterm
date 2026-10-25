# crates/terminal/

## Responsibility

Terminal stack, grouped by dependency direction instead of a flat list of
presentation and backend crates.

## Directory map

| Directory | Cargo package | Responsibility |
| --- | --- | --- |
| [`core/`](core/codemap.md) | `terminal-kernel` | Headless contracts, sessions, PTY/process support, and protocol observation |
| [`view/`](view/codemap.md) | `terminal` | GPUI terminal model, interaction, graphics storage, and painting |
| [`backends/alacritty/`](backends/alacritty/codemap.md) | `terminal-kernel-alacritty` | Cross-platform Alacritty session factory |
| [`backends/vte/`](backends/vte/codemap.md) | `terminal-kernel-vte` | Linux-only VTE emulator and session factory |

## Flow and boundaries

The app translates config into `core::SessionOptions`. A backend returns
`TerminalSession` and events; the app converts those resources into the view's
`Terminal`. View and backends depend on core, never on one another. Both backends
can be compiled/tested without GPUI or the persisted config schema.
