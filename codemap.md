# kazeterm/

<!-- Explorer: Fill in this section with architectural understanding -->

## Responsibility

<!-- What is this folder's job in the system? -->

## Design

<!-- Key patterns, abstractions, architectural decisions -->

## Flow

<!-- How does data/control flow through this module? -->

## Integration

<!-- How does it connect to other parts of the system? -->
# Repository Atlas: Kazeterm

## Project Responsibility

Kazeterm is a Rust 2024, GPUI-based cross-platform terminal emulator. The workspace separates persisted configuration, theme adaptation, serializable UI state, terminal presentation, backend-neutral terminal contracts, concrete PTY/emulator kernels, external event ingress, and desktop application composition.

## System Entry Points

| Path | Responsibility |
| --- | --- |
| `Cargo.toml` | Defines workspace membership, shared dependency versions, default platform-safe members, lint policy, and release profiles. |
| `crates/kazeterm/src/main.rs` | Loads configuration, initializes GPUI/themes/keybindings, starts watchers and event sources, and opens the initial window. |
| `crates/kazeterm/src/components/main_window.rs` | Owns the live application UI, terminal tabs, overlays, transition tasks, and the serializable UI-tree bridge. |
| `crates/kazeterm/src/reconciler.rs` | Applies validated `UIAction` values to the UI tree and translates semantic diffs into GPUI mutations. |
| `README.MD` | Documents setup and user-facing configuration, including terminal backends, updates, and animation controls. |
| `Agent.md` | Records repository architecture, editing constraints, and validation expectations for contributors. |

## Repository Directory Map

| Directory | Responsibility | Detailed map |
| --- | --- | --- |
| `crates/` | Layered Rust workspace containing all runtime libraries and the executable. | [View map](crates/codemap.md) |
| `crates/config/` | Versioned configuration schema, overlays, migrations, palettes, profiles, shells, and keybindings. | [View map](crates/config/codemap.md) |
| `crates/kazeterm/` | GPUI application composition, windows/components, workspace state, transitions, update support, and platform integration. | [View map](crates/kazeterm/codemap.md) |
| `crates/kazeterm-event-system/` | Typed and JSON event ingress with GPUI main-thread dispatch. | [View map](crates/kazeterm-event-system/codemap.md) |
| `crates/kazeterm-ui-tree/` | Serializable UI aggregate, validated reducer actions, semantic diffs, and reconciliation port. | [View map](crates/kazeterm-ui-tree/codemap.md) |
| `crates/themeing/` | Active GPUI theme/zoom globals and ANSI-to-HSLA color adaptation. | [View map](crates/themeing/codemap.md) |
| `crates/terminal/` | Backend-neutral terminal model, GPUI interaction, custom rendering, search, scroll, CWD/process state, and Kitty graphics. | [View map](crates/terminal/codemap.md) |
| `crates/terminal-kernel/` | Object-safe emulator contract, snapshots, shared primitives, and the common Alacritty adapter. | [View map](crates/terminal-kernel/codemap.md) |
| `crates/terminal-kernel-alacritty/` | Default Alacritty-backed PTY/session factory and event-loop adapter. | [View map](crates/terminal-kernel-alacritty/codemap.md) |
| `crates/terminal-kernel-vte/` | Alternative VTE-parser emulator and PTY loop for Linux. | [View map](crates/terminal-kernel-vte/codemap.md) |
| `crates/terminal-kernel-unix-bundle/` | Target-gated build aggregator that includes the VTE backend on Unix without affecting Windows. | [View map](crates/terminal-kernel-unix-bundle/codemap.md) |
| `assets/` | Embedded fonts, icons, themes, desktop metadata, and screenshots consumed by the application/build. | — |
| `memories/` | Iteration notes and verification summaries required by the repository workflow. | — |

## Cross-Crate Control Flow

1. `kazeterm` loads `config::Config`, installs `themeing::SettingsStore`, and initializes GPUI and terminal keybindings.
2. Window creation selects a concrete terminal kernel, which returns `terminal::Terminal` behind the shared `terminal-kernel` ports.
3. User and external commands become `kazeterm-ui-tree::UIAction` values; the reducer validates them and the application reconciler applies resulting diffs to `MainWindow`.
4. Structural UI changes use the shared `[animation]` configuration and `TransitionSpec`; disabling animation applies final state synchronously.
5. `terminal` maps GPUI input to PTY messages and renders backend snapshots; kernel events flow back into terminal views and application tabs.
6. Configuration hot reload replaces config/theme globals, refreshes bindings and platform appearance, and transitions all registered windows.
