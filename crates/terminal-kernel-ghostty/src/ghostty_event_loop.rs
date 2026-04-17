//! Event loop that reads from the PTY, feeds bytes through `libghostty-vt`,
//! and handles input/resize/shutdown messages.

use std::borrow::Cow;
use std::io::{self, Read, Write};
use std::sync::Arc;
use std::thread;

use parking_lot::Mutex;
use terminal_kernel::event::WindowSize;
use terminal_kernel::index::{Column, Line, Point as AlacPoint};
use terminal_kernel::term::cell::{Cell, Flags as CellFlags};
use terminal_kernel::term::TermMode;
use terminal_kernel::vte::ansi::{Color, CursorShape, CursorStyle, NamedColor, Rgb};

use libghostty_vt::render::{CellIterator, CursorVisualStyle, RowIterator};
use libghostty_vt::screen::CellWide;
use libghostty_vt::style::{StyleColor, Underline};
use libghostty_vt::{RenderState, Terminal, TerminalOptions};

use crate::ghostty_term::GhosttyTermInner;

/// Messages sent to the ghostty event loop.
#[allow(dead_code)]
pub enum GhosttyMsg {
  Input(Cow<'static, [u8]>),
  Resize(WindowSize),
  Shutdown,
}

pub type GhosttyMsgSender = std::sync::mpsc::Sender<GhosttyMsg>;

/// Event loop that owns a ghostty `Terminal` (which is `!Send + !Sync`) and
/// drives it from a dedicated thread.  PTY I/O and channel messages are
/// interleaved using non-blocking reads.
pub struct GhosttyEventLoop {
  tx: GhosttyMsgSender,
  rx: std::sync::mpsc::Receiver<GhosttyMsg>,
  pty_reader: std::fs::File,
  pty_writer: std::fs::File,
  state: Arc<Mutex<GhosttyTermInner>>,
  #[cfg(unix)]
  pty_raw_fd: i32,
  /// Keeps the child process alive for the lifetime of the event loop.
  _pty: terminal_kernel::tty::Pty,
  /// Initial terminal dimensions.
  initial_cols: u16,
  initial_rows: u16,
  max_scrollback: usize,
}

impl GhosttyEventLoop {
  pub fn new(
    pty: terminal_kernel::tty::Pty,
    pty_reader: std::fs::File,
    pty_writer: std::fs::File,
    state: Arc<Mutex<GhosttyTermInner>>,
    #[cfg(unix)] pty_raw_fd: i32,
    initial_cols: u16,
    initial_rows: u16,
    max_scrollback: usize,
  ) -> Self {
    let (tx, rx) = std::sync::mpsc::channel();
    Self {
      tx,
      rx,
      pty_reader,
      pty_writer,
      state,
      #[cfg(unix)]
      pty_raw_fd,
      _pty: pty,
      initial_cols,
      initial_rows,
      max_scrollback,
    }
  }

  /// Get a clone of the sender for sending messages to this loop.
  pub fn channel(&self) -> GhosttyMsgSender {
    self.tx.clone()
  }

  /// Spawn the event loop on a dedicated thread.
  pub fn spawn(self) -> thread::JoinHandle<()> {
    thread::Builder::new()
      .name("ghostty-event-loop".into())
      .spawn(move || {
        self.run();
      })
      .expect("spawn ghostty event loop")
  }

  fn run(mut self) {
    #[cfg(unix)]
    {
      unsafe {
        let flags = libc::fcntl(self.pty_raw_fd, libc::F_GETFL);
        libc::fcntl(self.pty_raw_fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
      }
    }

    // All ghostty objects are !Send + !Sync, so they must be created here on
    // the event-loop thread.
    let mut terminal = match Terminal::new(TerminalOptions {
      cols: self.initial_cols,
      rows: self.initial_rows,
      max_scrollback: self.max_scrollback,
    }) {
      Ok(t) => t,
      Err(e) => {
        eprintln!("ghostty: failed to create terminal: {e:?}");
        self
          .state
          .lock()
          .sync_from_ghostty(vec![], AlacPoint::default(), CursorStyle::default(), TermMode::empty(), [None; 256], vec![]);
        return;
      }
    };

    let mut render_state = match RenderState::new() {
      Ok(rs) => rs,
      Err(e) => {
        eprintln!("ghostty: failed to create render state: {e:?}");
        return;
      }
    };

    let mut row_iter = match RowIterator::new() {
      Ok(r) => r,
      Err(e) => {
        eprintln!("ghostty: failed to create row iterator: {e:?}");
        return;
      }
    };

    let mut cell_iter = match CellIterator::new() {
      Ok(c) => c,
      Err(e) => {
        eprintln!("ghostty: failed to create cell iterator: {e:?}");
        return;
      }
    };

    // Wire up PTY write-back for query responses.
    {
      let mut writer = self
        .pty_writer
        .try_clone()
        .expect("clone pty writer for ghostty effect");
      let _ = terminal.on_pty_write(move |_term, data| {
        let _ = writer.write_all(data);
        let _ = writer.flush();
      });
    }

    // Track scrollback for delta computation.
    let mut prev_scrollback_count: usize = 0;

    let mut buf = [0u8; 4096];

    loop {
      // Drain the message channel (non-blocking).
      loop {
        match self.rx.try_recv() {
          Ok(GhosttyMsg::Input(bytes)) => {
            let _ = self.pty_writer.write_all(&bytes);
            let _ = self.pty_writer.flush();
          }
          Ok(GhosttyMsg::Resize(size)) => {
            #[cfg(unix)]
            {
              let win = libc::winsize {
                ws_row: size.num_lines,
                ws_col: size.num_cols,
                ws_xpixel: size.cell_width.saturating_mul(size.num_cols),
                ws_ypixel: size.cell_height.saturating_mul(size.num_lines),
              };
              unsafe {
                libc::ioctl(self.pty_raw_fd, libc::TIOCSWINSZ, &win as *const _);
              }
            }
            let _ = terminal.resize(
              size.num_cols,
              size.num_lines,
              size.cell_width as u32,
              size.cell_height as u32,
            );
            sync_to_inner(
              &terminal,
              &mut render_state,
              &mut row_iter,
              &mut cell_iter,
              &self.state,
              &mut prev_scrollback_count,
            );
          }
          Ok(GhosttyMsg::Shutdown) => return,
          Err(std::sync::mpsc::TryRecvError::Empty) => break,
          Err(std::sync::mpsc::TryRecvError::Disconnected) => return,
        }
      }

      // Read from PTY (non-blocking on Unix).
      match self.pty_reader.read(&mut buf) {
        Ok(0) => {
          // EOF — child process exited.
          sync_to_inner(
            &terminal,
            &mut render_state,
            &mut row_iter,
            &mut cell_iter,
            &self.state,
            &mut prev_scrollback_count,
          );
          return;
        }
        Ok(n) => {
          terminal.vt_write(&buf[..n]);
          sync_to_inner(
            &terminal,
            &mut render_state,
            &mut row_iter,
            &mut cell_iter,
            &self.state,
            &mut prev_scrollback_count,
          );
        }
        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
          thread::sleep(std::time::Duration::from_millis(2));
        }
        Err(_) => {
          sync_to_inner(
            &terminal,
            &mut render_state,
            &mut row_iter,
            &mut cell_iter,
            &self.state,
            &mut prev_scrollback_count,
          );
          return;
        }
      }
    }
  }
}

// ---------------------------------------------------------------------------
// Sync ghostty render state → GhosttyTermInner
// ---------------------------------------------------------------------------

fn sync_to_inner<'a>(
  terminal: &Terminal<'a, 'a>,
  render_state: &mut RenderState<'a>,
  row_iter: &mut RowIterator<'a>,
  cell_iter: &mut CellIterator<'a>,
  shared_state: &Arc<Mutex<GhosttyTermInner>>,
  prev_scrollback_count: &mut usize,
) {
  let snapshot = match render_state.update(terminal) {
    Ok(s) => s,
    Err(_) => return,
  };

  let num_cols = snapshot.cols().unwrap_or(80) as usize;
  let num_rows = snapshot.rows().unwrap_or(24) as usize;

  // Build visible rows.
  let mut visible_rows: Vec<Vec<Cell>> = Vec::with_capacity(num_rows);

  if let Ok(mut row_iteration) = row_iter.update(&snapshot) {
    while let Some(row) = row_iteration.next() {
      let mut row_cells = Vec::with_capacity(num_cols);

      let is_wrapped = row
        .raw_row()
        .ok()
        .and_then(|r| r.is_wrapped().ok())
        .unwrap_or(false);

      if let Ok(mut cell_iteration) = cell_iter.update(&row) {
        while let Some(cell) = cell_iteration.next() {
          let alac_cell = convert_ghostty_cell(&cell, is_wrapped && row_cells.len() == num_cols.saturating_sub(1));
          row_cells.push(alac_cell);
        }
      }

      // Pad to num_cols if needed.
      while row_cells.len() < num_cols {
        row_cells.push(Cell::default());
      }

      // Mark last cell of wrapped rows.
      if is_wrapped {
        if let Some(last) = row_cells.last_mut() {
          last.flags.insert(CellFlags::WRAPLINE);
        }
      }

      visible_rows.push(row_cells);
    }
  }

  // Cursor.
  let cursor_point = snapshot
    .cursor_viewport()
    .ok()
    .flatten()
    .map(|cv| AlacPoint::new(Line(cv.y as i32), Column(cv.x as usize)))
    .unwrap_or_default();

  let cursor_shape = snapshot
    .cursor_visual_style()
    .ok()
    .map(|vs| match vs {
      CursorVisualStyle::Block | CursorVisualStyle::BlockHollow => CursorShape::Block,
      CursorVisualStyle::Underline => CursorShape::Underline,
      CursorVisualStyle::Bar => CursorShape::Beam,
      _ => CursorShape::Block,
    })
    .unwrap_or(CursorShape::Block);

  let cursor_blinking = snapshot.cursor_blinking().unwrap_or(true);
  let cursor_style = CursorStyle {
    shape: cursor_shape,
    blinking: cursor_blinking,
  };

  // Terminal modes.
  let mut mode = TermMode::empty();
  let cursor_visible = snapshot.cursor_visible().unwrap_or(true);
  if cursor_visible {
    mode.insert(TermMode::SHOW_CURSOR);
  }
  // Check for alt screen mode via terminal query.
  if terminal
    .mode(libghostty_vt::terminal::Mode::ALT_SCREEN_SAVE)
    .unwrap_or(false)
  {
    mode.insert(TermMode::ALT_SCREEN);
  }
  if terminal
    .mode(libghostty_vt::terminal::Mode::BRACKETED_PASTE)
    .unwrap_or(false)
  {
    mode.insert(TermMode::BRACKETED_PASTE);
  }
  if terminal
    .mode(libghostty_vt::terminal::Mode::FOCUS_EVENT)
    .unwrap_or(false)
  {
    mode.insert(TermMode::FOCUS_IN_OUT);
  }
  if terminal
    .mode(libghostty_vt::terminal::Mode::SGR_MOUSE)
    .unwrap_or(false)
  {
    mode.insert(TermMode::SGR_MOUSE);
  }
  if terminal
    .mode(libghostty_vt::terminal::Mode::NORMAL_MOUSE)
    .unwrap_or(false)
  {
    mode.insert(TermMode::MOUSE_REPORT_CLICK);
  }
  if terminal
    .mode(libghostty_vt::terminal::Mode::BUTTON_MOUSE)
    .unwrap_or(false)
  {
    mode.insert(TermMode::MOUSE_DRAG);
  }
  if terminal
    .mode(libghostty_vt::terminal::Mode::ANY_MOUSE)
    .unwrap_or(false)
  {
    mode.insert(TermMode::MOUSE_MOTION);
  }

  // Colors (256-color palette).
  let mut palette = [None; 256];
  if let Ok(colors) = snapshot.colors() {
    for (i, c) in colors.palette.iter().enumerate() {
      palette[i] = Some(Rgb {
        r: c.r,
        g: c.g,
        b: c.b,
      });
    }
  }

  // Scrollback delta: check if new scrollback lines appeared.
  let current_scrollback = terminal.scrollback_rows().unwrap_or(0);
  let scrollback_delta = if current_scrollback > *prev_scrollback_count {
    let new_count = current_scrollback - *prev_scrollback_count;
    let mut delta_rows = Vec::with_capacity(new_count);
    // Read the newly-added scrollback lines via grid_ref (History coordinates).
    // History y=0 is the oldest scrollback line.
    let start_y = *prev_scrollback_count;
    for y in start_y..current_scrollback {
      let mut row_cells = Vec::with_capacity(num_cols);
      for x in 0..num_cols {
        let cell = terminal
          .grid_ref(libghostty_vt::terminal::Point::History(
            libghostty_vt::terminal::PointCoordinate {
              x: x as u16,
              y: y as u32,
            },
          ))
          .ok()
          .map(|gr| convert_grid_ref_to_cell(&gr))
          .unwrap_or_default();
        row_cells.push(cell);
      }
      delta_rows.push(row_cells);
    }
    delta_rows
  } else {
    vec![]
  };
  *prev_scrollback_count = current_scrollback;

  // Commit to shared state.
  shared_state.lock().sync_from_ghostty(
    visible_rows,
    cursor_point,
    cursor_style,
    mode,
    palette,
    scrollback_delta,
  );
}

// ---------------------------------------------------------------------------
// Cell conversion helpers
// ---------------------------------------------------------------------------

/// Convert a ghostty render-state cell iteration entry to an alacritty `Cell`.
fn convert_ghostty_cell(
  cell: &libghostty_vt::render::CellIteration<'_, '_>,
  _is_last_wrapped: bool,
) -> Cell {
  let mut alac_cell = Cell::default();

  // Character.
  let graphemes = cell.graphemes().unwrap_or_default();
  if let Some(&ch) = graphemes.first() {
    alac_cell.c = ch;
  }

  // Style.
  if let Ok(style) = cell.style() {
    // Foreground color.
    alac_cell.fg = convert_style_color(&style.fg_color, true);
    // Background color.
    alac_cell.bg = convert_style_color(&style.bg_color, false);

    // Flags.
    if style.bold {
      alac_cell.flags.insert(CellFlags::BOLD);
    }
    if style.italic {
      alac_cell.flags.insert(CellFlags::ITALIC);
    }
    if style.faint {
      alac_cell.flags.insert(CellFlags::DIM);
    }
    if style.inverse {
      alac_cell.flags.insert(CellFlags::INVERSE);
    }
    if style.invisible {
      alac_cell.flags.insert(CellFlags::HIDDEN);
    }
    if style.strikethrough {
      alac_cell.flags.insert(CellFlags::STRIKEOUT);
    }
    match style.underline {
      Underline::None => {}
      Underline::Single => alac_cell.flags.insert(CellFlags::UNDERLINE),
      Underline::Double => alac_cell.flags.insert(CellFlags::DOUBLE_UNDERLINE),
      Underline::Curly => alac_cell.flags.insert(CellFlags::UNDERCURL),
      Underline::Dotted => alac_cell.flags.insert(CellFlags::DOTTED_UNDERLINE),
      Underline::Dashed => alac_cell.flags.insert(CellFlags::DASHED_UNDERLINE),
      _ => {}
    }
  }

  // Wide character flags.
  if let Ok(raw) = cell.raw_cell() {
    if let Ok(wide) = raw.wide() {
      match wide {
        CellWide::Wide => alac_cell.flags.insert(CellFlags::WIDE_CHAR),
        CellWide::SpacerTail => alac_cell.flags.insert(CellFlags::WIDE_CHAR_SPACER),
        _ => {}
      }
    }
  }

  alac_cell
}

/// Convert a ghostty `GridRef` (used for scrollback reads) to an alacritty `Cell`.
fn convert_grid_ref_to_cell(gr: &libghostty_vt::screen::GridRef<'_>) -> Cell {
  let mut alac_cell = Cell::default();

  let mut buf = ['\0'; 32];
  if let Ok(n) = gr.graphemes(&mut buf) {
    if n > 0 {
      alac_cell.c = buf[0];
    }
  }

  if let Ok(style) = gr.style() {
    alac_cell.fg = convert_style_color(&style.fg_color, true);
    alac_cell.bg = convert_style_color(&style.bg_color, false);

    if style.bold {
      alac_cell.flags.insert(CellFlags::BOLD);
    }
    if style.italic {
      alac_cell.flags.insert(CellFlags::ITALIC);
    }
    if style.faint {
      alac_cell.flags.insert(CellFlags::DIM);
    }
    if style.inverse {
      alac_cell.flags.insert(CellFlags::INVERSE);
    }
    if style.invisible {
      alac_cell.flags.insert(CellFlags::HIDDEN);
    }
    if style.strikethrough {
      alac_cell.flags.insert(CellFlags::STRIKEOUT);
    }
    match style.underline {
      Underline::None => {}
      Underline::Single => alac_cell.flags.insert(CellFlags::UNDERLINE),
      Underline::Double => alac_cell.flags.insert(CellFlags::DOUBLE_UNDERLINE),
      Underline::Curly => alac_cell.flags.insert(CellFlags::UNDERCURL),
      Underline::Dotted => alac_cell.flags.insert(CellFlags::DOTTED_UNDERLINE),
      Underline::Dashed => alac_cell.flags.insert(CellFlags::DASHED_UNDERLINE),
      _ => {}
    }
  }

  if let Ok(raw_cell) = gr.cell() {
    if let Ok(wide) = raw_cell.wide() {
      match wide {
        CellWide::Wide => alac_cell.flags.insert(CellFlags::WIDE_CHAR),
        CellWide::SpacerTail => alac_cell.flags.insert(CellFlags::WIDE_CHAR_SPACER),
        _ => {}
      }
    }
  }

  alac_cell
}

fn convert_style_color(color: &StyleColor, is_fg: bool) -> Color {
  match color {
    StyleColor::None => {
      if is_fg {
        Color::Named(NamedColor::Foreground)
      } else {
        Color::Named(NamedColor::Background)
      }
    }
    StyleColor::Palette(idx) => Color::Indexed(idx.0 as u8),
    StyleColor::Rgb(rgb) => Color::Spec(Rgb {
      r: rgb.r,
      g: rgb.g,
      b: rgb.b,
    }),
  }
}
