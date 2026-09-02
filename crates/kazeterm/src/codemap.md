# `crates/kazeterm/src/`

## Responsibility

This directory implements the executable's application/service layer around the GPUI component tree: startup, global configuration and theme creation, file-based hot reload, window registry/lifecycle, UI-tree reconciliation, build metadata, platform icons, and release updating.

## Module Map

| Module | Responsibility |
| --- | --- |
| `main.rs` | CLI and process entry point; installs globals, services, keybindings, application menus/actions, platform icon hooks, and opens the initial window. |
| `config.rs` | Converts `config::Config` plus system appearance into `themeing::SettingsStore`; applies configured background opacity across the palette. |
| `config_watcher.rs` | Watches config parents and custom-theme directories, debounces content events, reloads global stores/keybindings, updates window transparency, and broadcasts configuration transitions. |
| `window_manager.rs` | Creates/registers windows, maintains front-to-back weak window references, transfers detached tabs, propagates configuration transitions, and coordinates last-window shutdown. |
| `reconciler.rs` | Owns `UITreeStore`; captures live `MainWindow` state, applies `UIAction`, diffs old/new trees, and reconciles diffs into concrete component operations. |
| `auto_update.rs` | Checks GitHub releases, selects the target asset, downloads/extracts a staged package, asks the user for confirmation, persists the workspace, and launches a platform helper that replaces/restarts the binary. |
| `assets.rs` | Exposes rust-embedded fonts/icons/themes to GPUI and provides embedded theme loader/list callbacks to the configuration crate. |
| `build_info.rs` | Reads build-script environment metadata and exposes application/commit/release/target and terminal protocol version strings. |
| `app_icon.rs` | Installs or applies native application/window icons on macOS and Linux/X11. |
| `components/` | Live GPUI presentation and interaction layer. See [components/codemap.md](components/codemap.md). |
| `event_system/` | Shared-event-bus integration and Kazeterm handlers. See [event_system/codemap.md](event_system/codemap.md). |

## Design

### Composition root and global state

`main.rs` loads configuration before GPUI starts, registers theme asset callbacks, then publishes clones of `Config` and `SettingsStore` as GPUI globals. Components read these globals during rendering and behavior. Hot reload atomically replaces the globals and rebinds terminal keybindings rather than mutating individual fields throughout the tree.

### Canonical UI tree plus live entities

`MainWindow` owns a `UITreeStore` beside its live tabs, terminals, dialogs, and presentation state. The normal mutation path is:

`input/event -> UIAction -> UITreeStore::apply_action -> diff_trees -> UITreeStore::reconcile -> MainWindow/component mutation`

`MainWindow::reconciling_ui_tree` prevents reconciliation from recursively dispatching another action. `capture_from_main_window()` is the reverse adapter used before external snapshots, persistence, and actions that need stable tree/window/tab/pane IDs.

### Multi-window registry

`window_manager` stores weak `MainWindow` entities with native `AnyWindowHandle`s. It orders them with GPUI's window stack for cross-window tab drop targeting and uses the same registry to fan configuration transitions to every live window. A detached tab window is initialized only after the source tab is successfully claimed.

### Background services

- The file watcher converts `notify` callbacks into an async channel, filters to create/modify/remove content changes, coalesces them for 200 ms, reloads once, and resynchronizes watched paths when config locations change.
- Terminal sessions stream kernel events into `terminal::Terminal`, batching non-wakeup events in short windows in `components/terminal_window.rs`.
- The updater runs blocking network/archive work off the UI executor, returns a `PreparedUpdate`, and marshals confirmation back through a weak `MainWindow` and window handle.

## Data and Control Flow

### Startup and window creation

1. `Config::load()` and `init_theme_system()` establish the source configuration and theme search paths.
2. `Application::run()` initializes fonts, GPUI component styles, terminal keybindings, and global stores.
3. `open_kazeterm_window()` derives bounds, decorations, and opaque/transparent/blurred background from config.
4. `MainWindow::view_with_event_source()` creates/restores the component state and installs close interception/focus behavior.
5. `initialize_window()` registers the window and defers event input plus update checks.

### Configuration and theme reload

1. `notify` event -> `FileChangeType::{Config, Theme}` -> debounce.
2. A config reload calls `Config::load()`, rebuilds both globals, rebinds terminal keys, and recalculates native background appearance. A theme-only reload reuses the current config and rebuilds only theme state.
3. `window_manager::transition_configuration_change()` invokes every `MainWindow` so dimensions/orientation-dependent presentation can settle and fade into the new settings.
4. `TransitionSpec::from_config()` derives bounded frame count/duration, easing, and fade start from `Config::animation`. Vertical tab-bar width moves to its new target and root opacity rises to 1.0; disabled or zero-duration animation snaps both to their target state.

### UI action reconciliation

1. Before the first action, live state is captured into `UITree` and assigned stable `win-*`, `tab-*`, and `pane-*` IDs.
2. `UITree::apply()` changes the serializable model; `diff_trees()` emits semantic `TreeDiff`s.
3. `reconcile()` maps tab/pane/search/tab-bar/overlay/focus changes to `MainWindow` methods. Pane tree diffs rebuild the affected split tree while reusing terminal entities whose pane IDs remain present.
4. After non-initial visible diffs, reconciliation starts the configured root fade. `TreeDiff::WindowResized` additionally starts the configured native resize task; disabled animation applies the final size immediately.
5. Workspace save and JSON snapshot serialize this same tree. Restore recreates terminals and split containers, restores active tab/search/tab-bar state, adopts the loaded tree as canonical, and transitions a user-requested tree replacement.

### Update lifecycle

Automatic checks are skipped for local builds, `Never` policy, already-started process checks, or an unexpired once-daily interval. Release metadata and assets are fetched with `curl`; semantic release builds choose a newer tag, while WIP builds compare commit/timestamp. On confirmation the current UI tree is saved, a one-shot restore flag is persisted, a helper waits for process exit, backs up/replaces files, restarts Kazeterm, and the app quits.

## Integration Points

- `components::MainWindow` is the live mutation target for the reconciler, event adapter, updater prompts, and window manager.
- `config::Config` owns persistent application settings; this crate adapts them to native window options, themes, terminal launch, update policy, and component layout.
- Its top-level `AnimationConfig` is the shared policy for visible UI fades and geometry transitions; the configuration migration layer installs defaults for older files.
- `kazeterm_ui_tree` is the serialization/action/diff contract used by the event API and workspace files.
- `kazeterm_event_system` connects optional stdio/socket sources to a per-window GPUI event bus.
- Terminal creation selects `terminal-kernel-alacritty` everywhere and `terminal-kernel-vte` on Linux, then wraps the kernel session in `terminal::TerminalView`.
- GPUI weak entities/window handles are the cross-task and cross-window addressing mechanism; failed upgrades naturally stop work for closed UI objects.
