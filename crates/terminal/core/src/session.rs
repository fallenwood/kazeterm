//! GUI-independent inputs and resources shared by terminal session factories.

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicU32;
use std::sync::mpsc::Receiver;

use crate::event::WindowSize;
use crate::grid::Dimensions;
use crate::program_status::ProgramStatusTracker;
use crate::protocols::RawGraphicsEvent;
use crate::term::Osc52;
use crate::vte::ansi::CursorStyle;
use crate::{PtyProcessInfo, TerminalBackend};

/// Grid and cell dimensions without a dependency on GPUI pixel types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerminalSize {
  pub lines: u16,
  pub columns: u16,
  pub cell_width: u16,
  pub cell_height: u16,
}

impl Default for TerminalSize {
  fn default() -> Self {
    Self {
      lines: 6,
      columns: 100,
      cell_width: 5,
      cell_height: 5,
    }
  }
}

impl Dimensions for TerminalSize {
  fn total_lines(&self) -> usize {
    self.screen_lines()
  }

  fn screen_lines(&self) -> usize {
    self.lines as usize
  }

  fn columns(&self) -> usize {
    self.columns as usize
  }
}

impl From<TerminalSize> for WindowSize {
  fn from(size: TerminalSize) -> Self {
    Self {
      num_lines: size.lines,
      num_cols: size.columns,
      cell_width: size.cell_width,
      cell_height: size.cell_height,
    }
  }
}

/// Application configuration is translated into these options at the UI boundary.
/// Backends must not depend on the application's persisted configuration schema.
#[derive(Clone, Debug)]
pub struct SessionOptions {
  pub size: TerminalSize,
  pub scrollback_lines: usize,
  pub cursor_style: CursorStyle,
  pub osc52: Osc52,
  pub env: HashMap<String, String>,
}

/// Input/resize port implemented by each backend's PTY event loop.
pub trait PtySender: Send {
  fn send_input(&self, bytes: Cow<'static, [u8]>);
  fn send_resize(&self, size: WindowSize);
}

/// Resources produced by a backend, before a GPUI terminal is constructed.
/// The presentation layer takes ownership of these resources without restarting
/// the emulator or its child process.
pub struct TerminalSession {
  pub pty_tx: Box<dyn PtySender>,
  pub backend: Box<dyn TerminalBackend>,
  pub pty_info: PtyProcessInfo,
  pub graphics_rx: Option<futures::channel::mpsc::UnboundedReceiver<RawGraphicsEvent>>,
  pub pending_cnl: Option<Arc<AtomicU32>>,
  pub keyboard_protocol_flags: Arc<AtomicU32>,
  pub osc7_rx: Option<Receiver<PathBuf>>,
  pub cwd_file: Option<PathBuf>,
  pub program_status_tracker: ProgramStatusTracker,
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn terminal_size_preserves_grid_and_pty_dimensions() {
    for size in [
      TerminalSize::default(),
      TerminalSize {
        lines: 24,
        columns: 80,
        cell_width: 10,
        cell_height: 20,
      },
    ] {
      let window: WindowSize = size.into();
      assert_eq!(size.total_lines(), window.num_lines as usize);
      assert_eq!(size.screen_lines(), window.num_lines as usize);
      assert_eq!(size.columns(), window.num_cols as usize);
      assert_eq!(size.cell_width, window.cell_width);
      assert_eq!(size.cell_height, window.cell_height);
    }
  }
}
