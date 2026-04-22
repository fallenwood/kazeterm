# Copilot Instructions for Kazeterm

## Build & Test Commands

```bash
# Build (debug)
cargo build

# Build (release, used by CI)
cargo build --profile=release-fast

# Run all tests
cargo test --workspace

# Run a single test
cargo test --package <crate> <test_name>
# Example: cargo test --package config test_load_config

# Generate coverage (requires cargo-llvm-cov)
cargo llvm-cov --workspace --all-features --lcov --output-path coverage/lcov.info
```

## Architecture Overview

Kazeterm is a cross-platform terminal emulator built with the GPUI framework (from Zed editor) and Alacritty's terminal emulation backend.

### Crate Structure

```
crates/
├── kazeterm-event-system/ # Shared AppEvent types, JSON parsing, readers, dispatch runtime
├── kazeterm/   # Main application entry point, window management, UI components
├── terminal/   # Terminal rendering, PTY management, input handling
├── config/     # Configuration loading, shell profiles, theme palette parsing
└── themeing/   # Theme management, zoom state, color system
```

**Dependency flow:** `kazeterm` → {`kazeterm-event-system`, `config`, `terminal`, `themeing`}, where `terminal` and `themeing` both depend on `config`.

### Key Technologies

- **GPUI 0.2.2**: UI framework from Zed (reactive, GPU-accelerated)
- **gpui-component 0.5.1**: Higher-level UI components (buttons, tabs, dialogs, etc.)
- **alacritty_terminal 0.25.1**: ANSI terminal emulation and parsing
- **Platform-specific**: Uses Windows APIs directly on Windows, XCB on Linux

---

## Configuration System (`crates/config/`)

### Module Structure

```
config/src/
├── lib.rs              # Config struct, load/save, defaults
├── migration.rs        # Versioned config migrations
├── keybinding.rs       # KeybindingConfig parsing
├── palette.rs          # Palette struct (59 color fields for UI/terminal)
├── shell.rs            # Shell/profile detection (platform-specific)
├── ssh.rs              # SSH host detection from ~/.ssh/config
├── alacritty_import.rs # Import alacritty.toml into Kazeterm config
└── theme/
    ├── mod.rs          # ThemeMode, ThemeFile, ThemeColors, theme loading
    └── colors.rs       # ThemeColors → Palette conversion
```

### Config Fields (kazeterm.toml)

All fields have defaults via `#[serde(default)]` on Config:

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `version` | String | CURRENT_CONFIG_VERSION | Config version for migration |
| `theme` | String | `"one"` | Theme name |
| `theme_mode` | ThemeMode | `Dark` | `dark`, `light`, or `system` |
| `themes_path` | Option<String> | None | Custom themes directory |
| `default_profile` | Option<String> | None | Default shell profile name |
| `profiles` | Vec<Profile> | auto-detected | Shell profiles |
| `font_size` | f32 | 18.0 | Terminal font size |
| `font_family` | String | `"Cascadia Code NF"` | Terminal font |
| `ui_font_family` | String | `"Segoe UI"` (Win) / `"Noto Sans"` | UI font |
| `ui_font_size` | f32 | 18.0 | UI font size |
| `window_width` | f32 | 800.0 | Initial window width |
| `window_height` | f32 | 600.0 | Initial window height |
| `minimap_enabled` | bool | false | Show terminal minimap |
| `vertical_tabs` | bool | false | Vertical tab sidebar |
| `close_on_last_tab` | bool | true | Close app on last tab close |
| `tab_switcher_popup` | bool | true | Show Ctrl+Tab switcher |
| `background_opacity` | f32 | 1.0 | Window opacity (0.0-1.0) |
| `background_blur` | bool | false | Blur behind window (requires opacity < 1.0) |
| `keybindings` | KeybindingConfig | see below | Custom keybindings |
| `long_running_threshold_secs` | u64 | 10 | Notification threshold |
| `notification_interval_secs` | u64 | 0 | Min interval between notifications |
| `scrollback_lines` | u32 | 10000 | Scrollback buffer size |
| `cursor_shape` | String | `"block"` | `block`, `underline`, or `beam` |
| `cursor_blink` | bool | true | Cursor blinking |
| `cursor_blink_interval` | u64 | 750 | Blink interval (ms) |
| `osc52` | String | `"copy_only"` | OSC 52 mode |
| `copy_on_select` | bool | false | Auto-copy selection |
| `hide_mouse_when_typing` | bool | false | Hide mouse pointer after terminal input until it moves again |
| `env` | HashMap<String,String> | empty | Extra env vars |
| `working_directory` | Option<String> | None | Default working dir |

`container_profiles` is `#[serde(skip)]` — auto-detected at runtime (Docker/Podman/distrobox).

### Config Loading Flow

1. Path: `%APPDATA%/kazeterm/kazeterm.toml` (Windows) or `~/.config/kazeterm/kazeterm.toml`
2. If missing → create with defaults + header comment
3. Parse TOML → run `migration::apply_migrations()` → deserialize to `Config`
4. If migrated → rewrite file to disk
5. On error → fall back to `Config::default()`

### Migration System

Migrations are a chain of `(from_version, to_version, migrate_fn)` in `migration.rs`.
Each migration modifies raw `toml::Value` before deserialization.

To add a new config field:
1. Add field to `Config` struct with serde default
2. Add to BOTH `Default` impls (main + test)
3. Bump `CURRENT_CONFIG_VERSION` in migration.rs
4. Add new `Migration` entry at end of `migrations()` list
5. Write migration function that inserts default value if key missing
6. Add test for the new migration

### Keybinding Defaults

| Action | Default |
|--------|---------|
| copy | `ctrl-shift-c` (macOS: `cmd-c`) |
| paste | `ctrl-shift-v` (macOS: `cmd-v`) |
| zoom_in | `ctrl-=` |
| zoom_out | `ctrl--` |
| zoom_reset | `ctrl-0` |
| next_tab | `ctrl-tab` |
| previous_tab | `ctrl-shift-tab` |
| toggle_search | `ctrl-shift-f` |
| split_horizontal | `ctrl-shift-d` |
| split_vertical | `ctrl-shift-e` |
| close_pane | `ctrl-shift-w` |
| focus_next_pane | `ctrl-shift-]` |
| focus_previous_pane | `ctrl-shift-[` |
| swap_split_panes | `ctrl-shift-x` |
| toggle_hidden_panes | `ctrl-shift-h` (macOS: `cmd-shift-h`) |
| toggle_fullscreen | `f11` (macOS: `f12`) |
| toggle_tab_bar | `ctrl-shift-b` |

Parsing: `ParsedKeybinding::parse("ctrl-shift-c")` extracts `{control, shift, alt, key}`.

### Theme System

- Theme files: TOML with `[dark]` and optional `[light]` sections
- Located in `assets/themes/` (embedded) or custom path
- `ThemeColors` fields: `background`, `foreground`, `accent`, `border`, 8 ANSI colors, 8 bright ANSI, `cursor`
- `ThemeColors::to_palette(is_dark)` converts to `Palette` (59 fields), auto-deriving dim/bright variants and UI surface colors
- Loading order: custom themes path → embedded themes → filesystem fallback

### Background Opacity

- `Config::get_background_opacity()` clamps to [0.0, 1.0]; on non-Linux it halves the value (hack)
- When opacity < 1.0, `config.rs::create_settings_store()` applies opacity to ~20 palette background colors
- Window uses `WindowBackgroundAppearance::Transparent` or `::Blurred` (if `background_blur` enabled)
- `config_watcher.rs` updates all windows on hot-reload

### Alacritty Import

Imports from `alacritty.toml`: font, window opacity, shell, colors, scrollback, cursor, OSC 52, env, working_directory.
Generates a `ThemeFile` from Alacritty colors. UI is in `import_alacritty_dialog.rs`.

---

## Main Application (`crates/kazeterm/`)

### Module Structure

```
kazeterm/src/
├── main.rs              # Entry point, CLI args, app bootstrap, window creation
├── config.rs            # apply_background_opacity(), create_settings_store()
├── config_watcher.rs    # Hot-reload via notify (200ms debounce)
├── app_icon.rs          # Platform icon setup
├── assets.rs            # Embedded assets (fonts, themes, icons)
├── event_system/
│   └── mod.rs           # Kazeterm-specific event handlers built on kazeterm-event-system
└── components/
    ├── main_window.rs                      # MainWindow state (active tab, items, dialogs)
    ├── main_window_render.rs               # Render impl: titlebar, tab bar, terminal, overlays
    ├── main_window_tab_management.rs       # Tab CRUD, profile resolution, terminal events
    ├── main_window_tab_item.rs             # TabItem model (index, title, split_container)
    ├── main_window_tab_switcher_logic.rs   # Tab switcher overlay logic
    ├── main_window_dialog_handlers.rs      # Dialog show/handle (rename, close, about, import)
    ├── main_window_search.rs               # Search toggle/connect
    ├── main_window_split_pane_actions.rs   # Split/close/focus/swap pane actions
    ├── menu_builder.rs                     # Tab context menu + new tab dropdown
    ├── terminal_window.rs                  # Terminal+PTY creation, shell hooks, env setup
    ├── split_pane.rs                       # SplitContainer tree (Terminal|Split{dir,first,second,ratio})
    ├── search_bar.rs                       # SearchBar component (Render + EventEmitter)
    ├── tab_switcher.rs                     # TabSwitcher overlay (Render)
    ├── terminal_tab_bar.rs                 # TerminalTabBar/TerminalTab (RenderOnce)
    ├── tab_button.rs                       # TabButton close button (RenderOnce)
    ├── tab_rename_dialog.rs                # TabRenameDialog (Render + EventEmitter)
    ├── close_confirm_dialog.rs             # CloseConfirmDialog (Render + EventEmitter)
    ├── about_dialog.rs                     # AboutDialog (Render + EventEmitter)
    ├── import_alacritty_dialog.rs          # ImportAlacrittyDialog (Render + EventEmitter)
    ├── dragged_tab.rs                      # DraggedTab payload + DraggedTabView
    └── shell_icon.rs                       # ShellIcon enum (exe icon extraction on Windows)
```

## Shared Event System (`crates/kazeterm-event-system/`)

Holds the reusable event-system plumbing extracted from the app crate:

```
kazeterm-event-system/src/
├── lib.rs            # Global sender, GPUI dispatch loop, public re-exports
├── app_event.rs      # AppEvent enum + discriminants
├── event_bus.rs      # Generic EventBus<T>
├── event_sources.rs  # stdin/socket readers
└── json_event.rs     # JSON event protocol + source config
```

### Startup Flow (main.rs)

1. Parse CLI args (`--event-source stdio|socket`, `--event-socket <path>`)
2. Init tracing
3. `Config::load()`
4. Init theme system (register loaders, set custom path)
5. Create GPUI `Application` with embedded assets
6. In `app.run()`: load fonts, init gpui-component, init terminal crate, set globals (Config, SettingsStore), start config watcher, set platform icon, register actions, open first window
7. Window creation: build `WindowOptions` (size, titlebar, background appearance), open window, create `MainWindow::view()`, wrap in `gpui_component::Root`, start event system

### Component Patterns

**Entities with Render trait** (GPUI managed state):
- `MainWindow`, `SearchBar`, `TabSwitcher`, `AboutDialog`, `CloseConfirmDialog`, `TabRenameDialog`, `ImportAlacrittyDialog`, `DraggedTabView`

**RenderOnce** (stateless UI elements):
- `TerminalTabBar`, `TerminalTab`, `TabButton`

**Pure logic** (no Render):
- `TabItem`, `SplitContainer/SplitPane`, `terminal_window.rs`, `menu_builder.rs`, all `main_window_*.rs` extension files

### Split Pane Architecture

`SplitContainer` wraps a tree of `SplitPane`:
- `Terminal { id, terminal }` — leaf node
- `Split { direction, first, second, ratio }` — branch node

Operations: split active → replaces terminal leaf with split node; close pane → collapses to sibling; swap → swaps children of innermost split.

### Hot Reload (config_watcher.rs)

- Watches config file parent dir + themes dir via `notify::RecommendedWatcher`
- Filters to Modify/Create/Remove events, debounces 200ms
- On config change: full reload (Config::load → rebuild SettingsStore → rebind keys → update window backgrounds)
- On theme-only change: rebuild SettingsStore only

### Actions

Only one GPUI action in main crate: `actions!(kazeterm, [NewWindow])`.
Terminal crate defines its own actions in `terminal_view.rs`.
Most operations use direct method calls through the event system or keybinding dispatch.

---

## Terminal Crate (`crates/terminal/`)

### Module Structure

```
terminal/src/
├── lib.rs                    # Crate root, init(), re-exports
├── terminal_view.rs          # TerminalView (Render) — top-level GPUI entity
├── terminal/
│   ├── mod.rs                # Terminal struct (wraps Arc<FairMutex<Term>>)
│   ├── events.rs             # Event/InternalEvent enums
│   ├── input.rs              # Keystroke → PTY byte translation
│   └── mouse_scroll.rs       # Mouse scroll + alt-screen handling
├── terminal_element/
│   ├── mod.rs                # TerminalElement struct + LayoutState
│   ├── element_impl.rs       # Element trait impl (request_layout/prepaint/paint)
│   ├── grid_layout.rs        # Cell → LayoutRect + BatchedTextRun conversion
│   ├── helpers.rs            # Background region merging, cursor display
│   └── mouse_handlers.rs     # Click/drag/scroll handlers for terminal+scrollbar+minimap
├── kitty_graphics/
│   ├── mod.rs                # Re-exports
│   ├── command.rs            # Protocol types (KittyCommand, StoredImage, ImagePlacement)
│   ├── parser.rs             # APC payload parser (handles chunked base64)
│   ├── storage.rs            # KittyImageStorage with LRU eviction (320MB)
│   ├── placement.rs          # PlacementManager (grid-coordinate placements)
│   └── pty_filter.rs         # Unix PTY filter (intercepts \x1b_G before alacritty)
├── mappings/
│   ├── mod.rs
│   ├── colors.rs             # Alacritty → GPUI color mapping
│   ├── keys.rs               # GPUI keystroke → alacritty key mapping
│   └── mouse.rs              # Mouse button/event mapping
├── scrollbar.rs              # ScrollbarState + paint_scrollbar()
├── minimap.rs                # MinimapState + paint_minimap()
├── terminal_content.rs       # TerminalContent (cached render state)
├── terminal_bounds.rs        # TerminalBounds (visible area geometry)
├── terminal_hyperlinks.rs    # URL detection and hover
├── terminal_input_handler.rs # IME InputHandler impl
├── cursor_layout.rs          # CursorLayout (shape, position, paint)
├── indexed_cell.rs           # IndexedCell (cell + grid position)
├── layout_rect.rs            # LayoutRect (background rectangle)
├── batched_text_run.rs       # BatchedTextRun (styled text segment)
├── background_region.rs      # BackgroundRegion merging
├── highlighted_range_line.rs # Selection/search highlight rendering
├── hover_target.rs           # HoverTarget for hyperlinks
├── ime_state.rs              # ImeState for input method
├── mouse.rs                  # MouseState (click, drag, touch)
├── osc7.rs                   # OSC 7 CWD extraction
├── pty_info.rs               # PtyProcessInfo (pid, cwd, fg process)
└── apca_contrast.rs          # APCA contrast ratio calculation
```

### Key Types (exported from crate root)

- `Terminal` — core terminal state wrapping alacritty_terminal
- `TerminalView` — GPUI `Render` entity (produces div containing TerminalElement)
- `TerminalElement` — GPUI `Element` impl (actual canvas: layout → prepaint → paint)
- `PtyProcessInfo` — PTY process metadata
- `TerminalBounds` — visible area geometry
- `SelectionPhase` — selection state tracking

### Terminal Rendering Pipeline

1. `TerminalView::render()` → creates `TerminalElement` inside a div
2. `TerminalElement::request_layout()` → sets size constraints
3. `TerminalElement::prepaint()` → compute font metrics, sync terminal state, collect cells, build LayoutState
4. `TerminalElement::paint()` → paint background → selection rects → kitty images (behind) → text runs → IME → cursor → kitty images (above) → scrollbar → minimap

### PTY Integration

- Created in `terminal_window.rs`: builds `alacritty_terminal::Term`, creates PTY, wraps in `GraphicsPtyFilter` (Unix), spawns IO event loop
- **Input**: UI → `Terminal::input()` → `Notifier` → `EventLoop` → PTY slave
- **Output**: child → PTY → `EventLoop` → `TerminalEventListener` channel → batched `process_event()` calls
- **Kitty graphics**: intercepted by `GraphicsPtyFilter` before alacritty sees them (Unix); on Windows uses named pipe side-channel (`KAZETERM_GRAPHICS_PIPE` env var)

### Kitty Graphics Flow

1. Shell writes APC `\x1b_G...` sequence
2. PTY filter intercepts and sends `RawGraphicsCommand`
3. `Terminal::process_graphics_commands()` parses via `KittyParser`
4. `KittyImageStorage` stores decoded images (LRU, 320MB cap)
5. `PlacementManager` records grid-coordinate placements
6. `TerminalElement::paint()` renders visible placements via `paint_image()`

---

## Themeing Crate (`crates/themeing/`)

Bridges config `Palette` to GPUI's theme system.

### Key Types

- `SettingsStore` — holds `Palette`, `ZoomState`, font sizes/families, theme name
- `ZoomState` — tracks zoom level (bounded)

### Key Functions

- `create_settings_store()` (in kazeterm/config.rs) — builds SettingsStore from Config + Palette
- `SettingsStore::init_gpui_component_theme(cx)` — applies palette to gpui-component's theme system
- Color conversion: `Palette` → gpui-component `ColorScheme` mappings

---

## UI Component Patterns

### Dialog Components

Dialogs follow a consistent pattern (see `TabRenameDialog`, `CloseConfirmDialog`):

1. Define an event enum/struct for dialog results
2. Implement `EventEmitter<YourEvent>` for the dialog
3. Store subscriptions as struct fields (prefix with `_` to keep alive)
4. Parent subscribes with `cx.subscribe_in(&dialog, window, Self::handler)`

```rust
// Event definition
#[derive(Clone)]
pub enum MyDialogEvent {
  Confirm(String),
  Cancel,
}

// Dialog struct
pub struct MyDialog {
  focus_handle: FocusHandle,
  _subscription: Subscription,
}

impl EventEmitter<MyDialogEvent> for MyDialog {}
```

### Modal Overlay Pattern

```rust
div()
  .absolute()
  .inset_0()
  .flex()
  .items_center()
  .justify_center()
  .bg(gpui::black().opacity(0.5))
  .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| {
    cx.stop_propagation();
  })
  .child(/* dialog content */)
```

### State Management in MainWindow

- Store dialog entities as `Option<Entity<T>>` with paired subscription fields
- Use `.when(self.dialog.is_some(), |this| ...)` for conditional rendering
- Call `cx.notify()` after state changes to trigger re-render
- Focus management: `window.focus(&handle)` and `.track_focus(&handle)`

## Code Style

- **Indentation**: 2 spaces (see `.rustfmt.toml`, `.editorconfig`)
- **Trailing commas**: Always
- **Edition**: Rust 2024 — raw strings containing `#` must use `r##"..."##`

### Clippy Configuration

These lints are **denied** (will fail CI):
- `dbg_macro` - Remove debug macros before committing
- `todo` - No TODO macros in committed code
- `declare_interior_mutable_const`
- `redundant_clone`

Style lints are allowed to avoid blocking development.

## Platform-Specific Code

Use `#[cfg(target_os = "...")]` for platform-specific implementations. Key areas:
- Shell detection (`config/src/shell.rs`) — candidate order differs per OS
- PTY process info (`terminal/src/pty_info.rs`)
- System dark mode detection (`kazeterm/src/main.rs`)
- Window management and icons
- Kitty graphics: Unix uses PTY filter, Windows uses named pipe side-channel
- UI font: `Segoe UI` (Windows), `Noto Sans` (others)

## Configuration

User config file: `kazeterm.toml` at:
- Windows: `%APPDATA%/kazeterm/kazeterm.toml`
- Linux/macOS: `~/.config/kazeterm/kazeterm.toml`

Hot-reload is supported for config and theme changes (200ms debounce).

## Developing

DO NOT use release-fast profile when developing or debugging
