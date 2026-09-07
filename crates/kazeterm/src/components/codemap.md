# `crates/kazeterm/src/components/`

## Responsibility

This directory is Kazeterm's live GPUI presentation and interaction layer. It owns the `MainWindow` aggregate, terminal tabs and recursive split-pane trees, search and overlays, menus, drag/drop across panes and windows, desktop notifications, workspace restoration, and the frame-driven transition helpers used when visible UI state changes.

## Component Map

| Area | Modules | Responsibility |
| --- | --- | --- |
| Window aggregate | `main_window.rs`, `main_window_render.rs`, `main_window_tab_item.rs` | Defines all per-window state, constructs/restores it, captures/dispatches UI-tree state, handles input, and renders title/tab bars, terminal content, and overlays. |
| Tabs | `main_window_tab_management.rs`, `terminal_tab_bar.rs`, `tab_button.rs`, `tab_switcher.rs`, `main_window_tab_switcher_logic.rs` | Resolves profiles/shell launches; creates, activates, pins, closes, reorders, and cycles tabs; stores per-tab search and terminal subscriptions. |
| Split panes | `split_pane.rs`, `main_window_split_pane_actions.rs`, `split_pane_context_menu.rs` | Maintains a recursive binary split tree, ratios, pane IDs/focus, hidden-pane state, divider dragging, pane drop targets, and UI-tree-backed split/focus/close/swap actions. |
| Terminal bridge | `terminal_window.rs` | Chooses a configured kernel, creates a terminal session and `TerminalView`, then forwards/batches kernel events into the GPUI terminal entity. |
| Search | `search_bar.rs`, `main_window_search.rs` | Owns query/flags/match navigation/drag position and persists that state independently per tab; routes show/hide through `UIAction`. |
| Drag and transfer | `dragged_tab.rs`, `main_window_tab_transfer.rs` | Uses a one-shot claim token to reorder locally, move/merge across windows, or create a detached window without duplicating a tab. |
| Dialogs/overlays | `about_dialog.rs`, `close_confirm_dialog.rs`, `import_alacritty_dialog.rs`, `tab_rename_dialog.rs`, `shell_error_dialog.rs`, `update_confirm_dialog.rs`, `main_window_dialog_handlers.rs` | Render focused modal overlays, emit typed GPUI events, update the UI tree where applicable, save/close workspaces, import config, and confirm prepared updates. |
| Menus and decoration | `menu_builder.rs`, `shell_icon.rs` | Builds tab/new-terminal/terminal context menus from config and extracts/caches platform shell icons. |
| Persistence | `workspace_state.rs` | Serializes `UITree` to `workspace.json`, restores tabs/panes/search, reuses terminals during pane-tree reconciliation, and migrates the legacy workspace format. |
| Feedback | `notifications.rs` | Throttles long-running-command/bell notifications and uses native platform toast/sound mechanisms. |
| Transitions | `transitions.rs`, `main_window_transitions.rs` | Converts `Config::animation` into a validated `TransitionSpec`, supplies interpolation, and owns vertical tab-bar/general UI fade tasks. Window-size transition consumption is in the parent `reconciler.rs`. |

## Design

### `MainWindow` aggregate

`MainWindow` is the entity root and coordination boundary. Its state includes tabs/active tab, one shared search entity plus per-tab saved search state, tab switcher and modal entities, vertical tab-bar geometry, retained vertical/fade/window-resize tasks, root transition opacity, key-debug history, update state, notification throttle, `UITreeStore`, the reconciliation guard, event-source config, and active tab drag payload.

Large responsibilities are split into sibling `impl MainWindow` modules rather than nested controller objects. GPUI typed subscriptions connect child components and terminal views back to the aggregate; `Context::notify()` is the common invalidation mechanism.

### Tree-first command path with guarded concrete mutations

Public interaction methods generally check `reconciling_ui_tree`:

1. Outside reconciliation they capture live state, build a `UIAction`, and call `dispatch_default_ui_action()`.
2. `UITreeStore` applies/diffs the canonical tree and sets the guard.
3. The same method or a restore/rebuild helper performs the concrete entity mutation while guarded.

Hidden-pane operations and some presentation-only actions remain direct because hidden visibility is not represented in the UI tree. Drag transfers explicitly resynchronize the tree after moving live terminal entities.

### Recursive split composite

`SplitPane` is a Composite: each node is either `Terminal { id, entity }` or `Split { direction, first, second, ratio }`. `SplitContainer` adds active/next pane IDs and optional hidden-pane visibility. Rendering recursively builds flex layouts and draggable dividers. Directional focus derives normalized pane bounds and selects a candidate by direction, overlap, distance, and tie-break ordering.

### Modal event emitters

Dialogs are focused GPUI entities implementing `EventEmitter<T>`. `main_window_dialog_handlers.rs` owns their lifetime and subscriptions, translates confirm/cancel/close events into tree actions or services, removes the overlay entity, and restores terminal focus.

## Data and Control Flow

### Input to render

1. `main_window_render.rs` reads global `Config` and `SettingsStore`, derives layout/profile/keybinding data, and builds horizontal or vertical tab UI plus the active `SplitContainer`.
2. Key, menu, click, context-menu, and drop handlers call a `MainWindow` operation.
3. Structural operations usually flow through `UIAction -> UITreeStore -> TreeDiff`; reconciliation updates tabs/panes/search/overlays and notifies GPUI after applying diffs.
4. Rerender reads the updated entities. The active terminal is focused unless search or a modal owns focus.

### Tab and terminal lifecycle

1. Profile name, container/SSH target, working directory, and configuration resolve to shell path/args/title.
2. `new_terminal_window_with_shell()` creates the selected kernel session and a `TerminalView`; its event stream is processed immediately for the first event, then short-batched to lower render overhead.
3. A `TabItem` contains stable UI-tree ID, runtime index, titles/pin state, launch data, `SplitContainer`, terminal subscriptions, and saved search state.
4. Terminal `Wakeup`, `CommandFinished`, `UpdateTab`, and `CloseTerminal` events clear/update UI state, sound/notify, update titles, collapse panes, or close the containing tab.

### Split-pane lifecycle

Split/focus/close/swap commands synchronize active pane ID from actual GPUI focus, encode stable tab/pane IDs into a `UIAction`, then rebuild or focus the affected live tree during reconciliation. Divider drags update ratios directly. A tab dropped on a pane merges its complete split tree vertically after offsetting pane IDs and rebinding transferred terminal views to the destination window.

### Tab transfer lifecycle

`DraggedTab` contains source weak entity/window, runtime tab index, and shared atomic claim state. A successful drop can reorder in-place or atomically detach from another window and rebind the terminal views. Mouse release outside accepted targets tries registered destination windows front-to-back, otherwise opens a new window at the drop position. Empty source windows close after a successful transfer.

### Search and overlays

Only the active pane renders the shared `SearchBar`. Switching tabs saves the old search state and restores the new tab's query/flags/position/visibility. Modal entities render over terminal content in a fixed precedence order; their handlers close the entity and refocus the terminal. UI-tree dump/load uses native file prompts, while workspace close/update paths serialize the canonical tree.

## Transition and Change Handling

The transition implementation is task-based and globally configurable:

- Top-level `Config::animation` exposes `enabled`, `duration_ms`, `frame_interval_ms`, and `easing` (`linear`, `ease_in`, `ease_out`, or `ease_in_out`). Defaults are enabled/180 ms/15 ms/ease-in-out.
- `TransitionSpec::from_config()` returns `None` when disabled or duration is zero. Otherwise it uses the config crate's bounded frame count and exact effective frame duration and carries the selected easing curve.
- `set_tab_bar_visible()` and configuration reloads animate `vertical_tabbar_render_width`. Rendering derives tab-bar opacity from rendered/preferred width, so geometry and fade move together.
- A user drag cancels the vertical tab-bar task by replacing it with `Task::ready(())`, then updates preferred and rendered width immediately.
- `MainWindow` retains task handles so replacing a task cancels an in-flight transition. Window resize consumes the same `TransitionSpec` and interpolation helpers from `reconciler.rs`.
- When animation is disabled, width and window-size paths cancel their retained task, apply the final target immediately, and notify as needed.

## Integration Points

- Parent modules: `reconciler` drives concrete changes and consumes size interpolation; `window_manager` creates/transfers windows and broadcasts config transitions; `auto_update` provides `PreparedUpdate` and receives confirmation.
- `config::Config` supplies profiles, shells, tab geometry/orientation, pane divider/inactive styling, keybindings, notification thresholds, terminal kernel, theme, transparency, workspace behavior, and global animation policy/parameters.
- `terminal::TerminalView` is the leaf entity rendered in every pane and the source of terminal lifecycle events.
- `kazeterm_ui_tree` supplies `UIAction`, stable serializable nodes, and workspace representation.
- `gpui`/`gpui-kit` supply entities, focus/subscriptions, tasks/timers, rendering primitives, drag/drop, menus, dialogs, and window operations.
