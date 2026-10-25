# Repository Atlas: Kazeterm

## Responsibility

Kazeterm is a Rust 2024, GPUI-based cross-platform terminal emulator. The
workspace separates desktop composition, configuration/theme integration,
serializable UI state, event ingress, GPUI terminal presentation, and headless
terminal contracts/backends. See [architecture](docs/architecture.md) for the
complete dependency policy and session lifecycle.

## Entry points

| Path | Responsibility |
| --- | --- |
| `Cargo.toml` | Workspace membership, centralized internal/external dependencies, profiles, and lints |
| `apps/kazeterm/src/main.rs` | Desktop bootstrap, configuration/globals, GPUI setup, and first window |
| `apps/kazeterm/src/components/terminal_window.rs` | Config-to-session adapter, backend selection, and GPUI terminal creation |
| `apps/kazeterm/src/reconciler.rs` | UI actions and semantic diffs applied to live entities |
| `scripts/check-workspace.ps1` | Enforced workspace dependency boundaries (PowerShell 7) |
| `Agents.md` | Contributor constraints and validation requirements |

## Directory map

| Directory | Responsibility | Detailed map |
| --- | --- | --- |
| `apps/` | Executable composition roots | [Map](apps/codemap.md) |
| `apps/kazeterm/` | GPUI application, windows, services, persistence, and platform integration | [Map](apps/kazeterm/codemap.md) |
| `crates/` | Reusable libraries and terminal stack | [Map](crates/codemap.md) |
| `crates/config/` | Configuration schema, imports, migration, profiles, and palette | [Map](crates/config/codemap.md) |
| `crates/themeing/` | GPUI theme globals and ANSI color resolution | [Map](crates/themeing/codemap.md) |
| `crates/kazeterm-event-system/` | JSON ingress and GPUI event dispatch | [Map](crates/kazeterm-event-system/codemap.md) |
| `crates/kazeterm-ui-tree/` | Serializable UI aggregate, reducer, and diffs | [Map](crates/kazeterm-ui-tree/codemap.md) |
| `crates/terminal/` | Headless core/backends and GPUI view | [Map](crates/terminal/codemap.md) |
| `crates/terminal/core/` | Backend/session ports, PTY process information, and protocols | [Map](crates/terminal/core/codemap.md) |
| `crates/terminal/view/` | GPUI terminal model, interaction, and rendering | [Map](crates/terminal/view/codemap.md) |
| `crates/terminal/backends/alacritty/` | Default cross-platform session factory | [Map](crates/terminal/backends/alacritty/codemap.md) |
| `crates/terminal/backends/vte/` | Linux-only VTE emulator and factory | [Map](crates/terminal/backends/vte/codemap.md) |
| `assets/` | Embedded fonts, icons, themes, and platform metadata | — |
| `memories/` | Iteration summaries and validation records | — |

## Control flow

1. The app loads `config`, installs `themeing` globals, and initializes GPUI.
2. The app maps terminal settings into headless `SessionOptions`; a backend
   returns `TerminalSession` resources and `SessionEvents`.
3. The app moves those resources into `terminal::Terminal` and a `TerminalView`.
4. Input flows through `PtySender`; emulator/protocol events update the GPUI
   model and renderer, which consumes `TerminalBackend` snapshots.
5. User/external commands mutate `kazeterm-ui-tree`; the app reconciles diffs.
6. Hot reload updates global config/theme state and registered windows without
   recreating running terminal sessions.
