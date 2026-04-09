mod apca_contrast;
mod background_region;
mod batched_text_run;
mod cursor_layout;
mod highlighted_range_line;
mod hover_target;
mod ime_state;
mod indexed_cell;
pub mod kitty_graphics;
mod layout_rect;
mod mappings;
pub mod minimap;
mod mouse;
pub mod osc7;
mod pty_info;
pub mod scrollbar;
mod terminal;
mod terminal_bounds;
mod terminal_content;
mod terminal_element;
mod terminal_hyperlinks;
mod terminal_input_handler;
mod terminal_view;

pub use pty_info::PtyProcessInfo;
pub use terminal::{SelectionPhase, Terminal, TerminalEventListener};
pub use terminal_bounds::TerminalBounds;
pub use terminal_view::{TerminalEvent, TerminalView};

use config::KeybindingConfig;
use gpui::{App, KeyBinding};
use terminal_view::{
  Copy, Paste, ScrollPageDown, ScrollPageUp, SendPageDown, SendPageUp, SendTab, SendTabPrev,
  ZoomIn, ZoomOut, ZoomReset,
};

pub fn init(cx: &mut App, keybindings: &KeybindingConfig) {
  // Initialize ZoomState global
  cx.set_global(themeing::ZoomState::default());

  bind_terminal_keys(cx, keybindings);
}

/// Register terminal keybindings from config.
///
/// This is also called during hot-reload to update bindings.
pub fn bind_terminal_keys(cx: &mut App, keybindings: &KeybindingConfig) {
  cx.bind_keys([
    KeyBinding::new("tab", SendTab, Some("Terminal")),
    KeyBinding::new("shift-tab", SendTabPrev, Some("Terminal")),
    KeyBinding::new(&keybindings.copy, Copy, Some("Terminal")),
    KeyBinding::new(&keybindings.paste, Paste, Some("Terminal")),
    // Page up/down are context-dependent and not customizable
    KeyBinding::new("pageup", SendPageUp, Some("Terminal && screen == alt")),
    KeyBinding::new("pagedown", SendPageDown, Some("Terminal && screen == alt")),
    KeyBinding::new("pageup", ScrollPageUp, Some("Terminal && screen == normal")),
    KeyBinding::new(
      "pagedown",
      ScrollPageDown,
      Some("Terminal && screen == normal"),
    ),
    KeyBinding::new("shift-pageup", ScrollPageUp, Some("Terminal")),
    KeyBinding::new("shift-pagedown", ScrollPageDown, Some("Terminal")),
    KeyBinding::new(&keybindings.zoom_in, ZoomIn, Some("Terminal")),
    KeyBinding::new(&keybindings.zoom_out, ZoomOut, Some("Terminal")),
    KeyBinding::new(&keybindings.zoom_reset, ZoomReset, Some("Terminal")),
    // Global copy/paste shortcuts that are always intercepted by the terminal
    KeyBinding::new(&keybindings.global_copy, Copy, Some("Terminal")),
    KeyBinding::new(&keybindings.global_paste, Paste, Some("Terminal")),
  ]);
}
