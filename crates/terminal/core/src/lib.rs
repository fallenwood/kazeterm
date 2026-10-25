//! Headless terminal contracts, session resources, PTY support, and protocols.
//! This crate must not depend on configuration, theming, or GPUI presentation.

use futures::channel::mpsc::UnboundedReceiver;

mod events;
pub mod protocols;
pub mod pty_info;
mod session;
mod shell;

pub use events::TerminalEventListener;
pub use protocols::{osc7, program_status};
pub use pty_info::PtyProcessInfo;
pub use session::{PtySender, SessionOptions, TerminalSession, TerminalSize};
pub use shell::ShellLaunch;

pub use alacritty_terminal::Term;

pub mod event {
  pub use alacritty_terminal::event::*;
}

pub mod event_loop {
  pub use alacritty_terminal::event_loop::*;
}

pub mod grid {
  pub use alacritty_terminal::grid::*;
}

pub mod index {
  pub use alacritty_terminal::index::*;
}

pub mod selection {
  pub use alacritty_terminal::selection::*;
}

pub mod sync {
  pub use alacritty_terminal::sync::*;
}

pub mod term {
  pub use alacritty_terminal::term::*;
}

pub mod tty {
  pub use alacritty_terminal::tty::*;
}

pub mod vte {
  pub mod ansi {
    pub use alacritty_terminal::vte::ansi::*;
  }
}

pub type SessionEvents = UnboundedReceiver<event::Event>;

// ---------------------------------------------------------------------------
// TerminalBackend trait
// ---------------------------------------------------------------------------

mod backend;
pub use backend::{AlacrittyBackend, RenderableSnapshot, SelectionDisplay, TerminalBackend};

// ---------------------------------------------------------------------------
// ANSI color table helpers
// ---------------------------------------------------------------------------

/// Total number of color slots exposed by Alacritty's `Colors` table.
///
/// Slots `0..=255` are the indexed palette. Slots `256..=258` are the
/// effective foreground, background, and cursor colors. The remaining entries
/// are the dim/bright compatibility slots used by the renderer fallback path.
pub const ANSI_COLOR_COUNT: usize = 269;
pub const FOREGROUND_COLOR_INDEX: usize = 256;
pub const BACKGROUND_COLOR_INDEX: usize = 257;
pub const CURSOR_COLOR_INDEX: usize = 258;

pub fn is_default_background(color: &vte::ansi::Color) -> bool {
  matches!(
    color,
    vte::ansi::Color::Named(vte::ansi::NamedColor::Background)
  )
}

pub fn is_default_foreground(color: &vte::ansi::Color) -> bool {
  matches!(
    color,
    vte::ansi::Color::Named(vte::ansi::NamedColor::Foreground)
  )
}
