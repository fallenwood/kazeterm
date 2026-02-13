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
├── kazeterm/   # Main application entry point, window management, UI components
├── terminal/   # Terminal rendering, PTY management, input handling
├── config/     # Configuration loading, shell profiles, theme palette parsing
└── themeing/   # Theme management, zoom state, color system
```

**Dependency flow:** `kazeterm` → {`config`, `terminal`, `themeing`}, where `terminal` and `themeing` both depend on `config`.

### Key Technologies

- **GPUI**: UI framework from Zed (reactive, GPU-accelerated)
- **gpui-component**: Higher-level UI components (buttons, tabs, dialogs, etc.)
- **alacritty_terminal**: ANSI terminal emulation and parsing
- **Platform-specific**: Uses Windows APIs directly on Windows, XCB on Linux

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
  // Keep subscriptions alive
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

## Theme System

Themes are TOML files in `assets/themes/` with `[dark]` and `[light]` sections defining colors. Custom themes can be added to `~/.config/kazeterm/themes/` (Linux/macOS) or `%APPDATA%/kazeterm/themes/` (Windows).

Required colors: `background`, `foreground`, `accent`, `border`, plus ANSI colors (`black`, `red`, `green`, `yellow`, `blue`, `magenta`, `cyan`, `white`).

## Code Style

- **Indentation**: 2 spaces (see `.rustfmt.toml`, `.editorconfig`)
- **Trailing commas**: Always
- **Edition**: Rust 2024

### Clippy Configuration

These lints are **denied** (will fail CI):
- `dbg_macro` - Remove debug macros before committing
- `todo` - No TODO macros in committed code
- `declare_interior_mutable_const`
- `redundant_clone`

Style lints are allowed to avoid blocking development.

## Platform-Specific Code

Use `#[cfg(target_os = "...")]` for platform-specific implementations. Key areas:
- Shell detection (`config/src/shell.rs`)
- PTY process info (`terminal/src/pty_info.rs`)
- System dark mode detection (`kazeterm/src/main.rs`)
- Window management and icons

## Configuration

User config file: `kazeterm.toml` at:
- Windows: `%APPDATA%/kazeterm/kazeterm.toml`
- Linux/macOS: `~/.config/kazeterm/kazeterm.toml`

Hot-reload is supported for config and theme changes.

## Developing

DO NOT use release-fast profile when developing or debugging
