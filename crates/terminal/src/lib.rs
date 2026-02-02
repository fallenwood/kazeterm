mod apca_contrast;
mod background_region;
mod batched_text_run;
mod cursor_layout;
mod highlighted_range_line;
mod hover_target;
mod ime_state;
mod indexed_cell;
mod layout_rect;
mod mappings;
pub mod minimap;
mod mouse;
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

use gpui::{App, KeyBinding};
use terminal_view::{
  Copy, Paste, ScrollPageDown, ScrollPageUp, SendPageDown, SendPageUp, SendTab, SendTabPrev,
  ZoomIn, ZoomOut, ZoomReset,
};

pub fn init(cx: &mut App) {
  // Initialize ZoomState global
  cx.set_global(themeing::ZoomState::default());

  cx.bind_keys([
    KeyBinding::new("tab", SendTab, Some("Terminal")),
    KeyBinding::new("shift-tab", SendTabPrev, Some("Terminal")),
    KeyBinding::new("ctrl-shift-c", Copy, Some("Terminal")),
    KeyBinding::new("ctrl-shift-v", Paste, Some("Terminal")),
    // In alt screen mode (vim, less, etc.), send keys to the application
    KeyBinding::new("pageup", SendPageUp, Some("Terminal && screen == alt")),
    KeyBinding::new("pagedown", SendPageDown, Some("Terminal && screen == alt")),
    // In normal mode, scroll the scrollback buffer
    KeyBinding::new("pageup", ScrollPageUp, Some("Terminal && screen == normal")),
    KeyBinding::new(
      "pagedown",
      ScrollPageDown,
      Some("Terminal && screen == normal"),
    ),
    KeyBinding::new("shift-pageup", ScrollPageUp, Some("Terminal")),
    KeyBinding::new("shift-pagedown", ScrollPageDown, Some("Terminal")),
    // Zoom in/out
    KeyBinding::new("ctrl-=", ZoomIn, Some("Terminal")),
    KeyBinding::new("ctrl-+", ZoomIn, Some("Terminal")),
    KeyBinding::new("ctrl--", ZoomOut, Some("Terminal")),
    KeyBinding::new("ctrl-0", ZoomReset, Some("Terminal")),
  ]);
}
