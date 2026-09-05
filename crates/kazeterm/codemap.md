# Crate Atlas: `crates/kazeterm/`

## Responsibility

The `kazeterm` binary crate is the desktop application composition layer for the terminal emulator. It combines GPUI, the configuration/theme crates, terminal front-end and kernel implementations, the serializable UI tree, external event ingress, multi-window management, workspace persistence, platform integration, and self-update behavior into one executable.

## Entry Points and Build Assets

| Path | Responsibility |
| --- | --- |
| `Cargo.toml` | Declares the GPUI application and platform-specific dependency graph. The Alacritty terminal kernel is cross-platform; the VTE kernel and desktop notification/X11 support are Linux-only; Win32 APIs/resources are Windows-only. |
| `build.rs` | Embeds target/build metadata, release tag, timestamp, and Git commit into the binary; configures runtime library search paths on Linux/macOS and the icon, product metadata, and larger stack on Windows. |
| `src/main.rs` | Process entry point and composition root. Loads configuration, initializes themes/fonts/terminal keybindings, installs application actions, starts hot reload, and opens the first window. |

## Directory Map

| Directory | Responsibility | Detailed map |
| --- | --- | --- |
| `src/` | Application lifecycle, global configuration/theme state, update service, UI-tree reconciliation, and window registry. | [View map](src/codemap.md) |
| `src/components/` | Stateful GPUI presentation layer for windows, tabs, split panes, terminals, dialogs, menus, search, drag/drop, and UI transitions. | [View map](src/components/codemap.md) |
| `src/event_system/` | Kazeterm-specific adapter from shared `AppEvent` values to `MainWindow`, `UIAction`, application, and window operations. | [View map](src/event_system/codemap.md) |

## Design

- **Composition root and global stores:** startup creates the application through GPUI Kit, installs a global `config::Config` and `themeing::SettingsStore`, and initializes `terminal` and GPUI Kit against those stores.
- **Tree-backed presentation model:** `MainWindow` owns live GPUI entities while `UITreeStore` holds a serializable `kazeterm_ui_tree::UITree`. Most high-level mutations go through `UIAction -> TreeDiff -> reconcile` before concrete component methods update the live view.
- **Observer/event-driven UI:** GPUI subscriptions, terminal events, configuration file notifications, and the shared event bus all converge on `MainWindow` updates and `Context::notify()` rerenders.
- **Platform adapters:** icons, native notifications/bell sounds, window backgrounds, terminal-kernel selection, packaging metadata, and update helper scripts are selected with target-specific implementations.
- **Detached asynchronous work:** terminal kernel events, file watching, release checks/downloads, UI transition frame timers, and update preparation run on GPUI/background executors while weak entity/window handles protect against closed UI objects.

## Application Flow

1. `main()` parses `--event-source`/`--event-socket`, initializes tracing, and loads `config::Config`.
2. Embedded and optional custom themes are registered, then GPUI starts with embedded assets.
3. The application callback loads fonts, initializes shared UI/terminal services, publishes configuration and theme globals, starts the configuration watcher, and registers application/menu actions.
4. `window_manager::open_kazeterm_window()` builds `WindowOptions` from the current configuration and creates a `MainWindow` inside `gpui_kit::component::Root`.
5. Window initialization registers the window for cross-window tab drops/config transitions, then defers external event-system and auto-update startup.
6. `MainWindow` restores a saved UI tree when requested, otherwise creates an initial terminal tab. Terminal session creation is delegated to the configured kernel crate.
7. Keyboard, menu, mouse/drag, terminal, and external events mutate the UI. Tree-backed operations generate diffs which `reconciler` applies to GPUI entities; direct presentation changes call `cx.notify()`.
8. On configuration/theme changes, global stores and keybindings are replaced, background appearance is updated, and every registered window receives a presentation transition.

## UI Transition Surface

Transition mechanics are centralized under `src/components/transitions.rs` and driven by the top-level `[animation]` configuration:

- `enabled` globally switches intermediate animation frames on/off; `duration_ms`, `frame_interval_ms`, `easing`, and `fade_start_opacity` control their behavior.
- Defaults are enabled, 180 ms total, a requested 15 ms frame interval, `ease_in_out`, and 1.0 fade-start opacity. Timing/frame count and opacity are clamped by `AnimationConfig`; disabled or zero-duration transitions apply their final state immediately.
- Vertical tab-bar show/hide and orientation changes interpolate rendered width between zero and the remembered expanded width.
- Visible UI-tree diffs and direct presentation changes (dialogs, tab switcher/reorder/transfer, hidden-pane/focus changes, workspace replacement) fade the full `MainWindow` root toward opacity `1.0`; initial content creation is not faded.
- `TreeDiff::WindowResized` interpolates the native window size.

The transition state and tasks live on `MainWindow`, configuration reload fan-out is in `config_watcher.rs`/`window_manager.rs`, and `TransitionSpec` converts validated config into per-frame loops in `components/main_window_transitions.rs` and `reconciler.rs`. The config migration to version `20260901.1` adds the default `[animation]` table to existing files.

## Integration

- **Configuration and themes:** `config`, `themeing`, embedded `assets/themes`, and GPUI global stores.
- **Terminal runtime:** `terminal`, `terminal-kernel`, `terminal-kernel-alacritty`, and Linux `terminal-kernel-vte`.
- **UI state/actions:** `kazeterm-ui-tree` supplies nodes, actions, diffs, and serialization.
- **External automation:** `kazeterm-event-system` supplies event types, bus, stdio/socket sources, and send APIs.
- **Desktop framework:** `gpui` and `gpui-kit` provide entities, rendering, focus, windows, menus, tasks, and input dispatch.
- **Operating system:** Win32, Objective-C/macOS APIs, X11/Linux desktop conventions, and platform update/extraction commands.
