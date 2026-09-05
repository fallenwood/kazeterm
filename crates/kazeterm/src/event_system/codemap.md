# `crates/kazeterm/src/event_system/`

## Responsibility

This module is Kazeterm's application adapter for the shared `kazeterm-event-system` crate. It builds the per-window `EventBus<MainWindow>`, maps named `AppEvent` messages to tree-backed UI commands or direct GPUI/application operations, and starts the configured stdio/socket event source against a weak window entity.

## Design

- **Adapter:** shared transport/event types are translated into `MainWindow`, `Window`, and `App` operations without placing Kazeterm UI knowledge in the reusable event-system crate.
- **Observer/event bus:** `build_default_event_bus()` registers named handlers; the shared runtime dispatches incoming JSON events to the matching subscribers.
- **Command pipeline:** structural UI handlers construct `kazeterm_ui_tree::UIAction` values and call `MainWindow::dispatch_default_ui_action()`, preserving the same tree/diff/reconcile path used by local UI input.
- **Stable-ID translation:** helper functions synchronize the live tree and convert active runtime tab/pane selection into serializable `window_id`, `tab_id`, and `pane-*` identifiers.
- **Fallback for presentation-only state:** when hidden panes are active, pane commands call `MainWindow` directly because hidden-pane visibility is intentionally outside the canonical UI tree.

## Built-in Handler Map

| Event group | Event names | Target behavior |
| --- | --- | --- |
| Tab creation/closure | `NewTerminalWithDefaultProfile`, `NewTerminalWithProfile`, `CloseActiveTab`, `CloseTab` | Builds add/close actions, including profile and working-directory data. |
| Tab navigation | `NextTab`, `PreviousTab`, `SwitchToTab` | Dispatches next/previous/activate actions against the active window tree. |
| Pane structure | `SplitHorizontal`, `SplitVertical`, `CloseActivePane`, `SwapSplitPanes` | Synchronizes actual focus, captures shell/args/current working directory, and dispatches pane actions; hidden-pane mode falls back to direct component methods. |
| Pane focus | `FocusNextPane`, `FocusPreviousPane`, `FocusPaneUp`, `FocusPaneDown`, `FocusPaneLeft`, `FocusPaneRight` | Dispatches cycle/focus actions using active or geometrically selected pane IDs; hidden-pane mode uses direct focus logic. |
| Presentation | `ToggleSearch`, `ToggleTabBar`, `ToggleFullscreen`, `ShowAboutDialog`, `ShowImportAlacrittyDialog`, `FocusActiveTerminal` | Uses UI actions for serializable search/tab-bar/overlay state and direct native/fullscreen/focus calls where no tree mutation is required. |
| Process/window | `NewWindow`, `Quit`, `ReloadConfig` | Opens another window with the same event-source configuration, quits GPUI, or invokes the standard config/theme reload pipeline. |
| Terminal I/O | `SendTextToTerminal` | Writes bytes directly to the active terminal kernel input. |
| Tree API | `DispatchUIAction`, `SnapshotUITree` | Parses arbitrary action JSON for dispatch or logs a serialized snapshot of synchronized UI state. |
| Extension | `Custom` | Logs application-defined name/data payloads for consumers that extend the event vocabulary. |

## Data and Control Flow

1. CLI parsing in `main.rs` produces `EventSourceConfig::{None, Stdio, Socket { path }}`.
2. `window_manager::initialize_window()` defers `start_event_system()` with a weak `MainWindow`, native window handle, and that source configuration.
3. This module creates the default handler bus, then delegates transport/task startup to `kazeterm_event_system::start_event_system()`.
4. Incoming JSON is decoded by the shared crate into `AppEvent` and dispatched by event name on the window's GPUI context.
5. For tree-backed changes, handlers call `sync_ui_tree_and_window_id()`, derive active IDs and command parameters, build `UIAction`, and enter `UITreeStore` dispatch/reconciliation.
6. Reconciliation mutates live GPUI components and notifies the window. Direct handlers update fullscreen, focus, terminal input, application lifetime, or configuration services immediately.

## Configuration and Transition Integration

- `ReloadConfig` calls `config_watcher::reload_config_and_theme_from_event()`, so external reloads use the same global replacement, keybinding rebind, native background update, and per-window transition fan-out as filesystem changes.
- `ToggleTabBar` is tree-backed. Its `TreeDiff::TabBarVisibilityChanged` reaches `MainWindow::toggle_tab_bar()` under the reconciliation guard, which starts the configured vertical tab-bar width/opacity transition when vertical tabs are enabled.
- Arbitrary `DispatchUIAction` can request `WindowResized`; reconciliation starts the configured native window-size transition.
- The adapter does not duplicate animation logic. Geometry transitions read the global `AnimationConfig`; disabled or zero-duration animation applies final states immediately.

## Integration Points

- Re-exports `AppEvent`, `EventSourceConfig`, `JsonEvent`, `send_event`, and `try_send_event` from `kazeterm-event-system` for crate consumers.
- Depends on `MainWindow` for active selection, tree synchronization, action dispatch, focus, pane fallbacks, and terminal access.
- Depends on `kazeterm-ui-tree` for `UIAction`, overlay nodes, and split direction values.
- Calls `window_manager` for new windows and `config_watcher` for reloads; uses GPUI `Window`/`App` for fullscreen and process lifetime.
