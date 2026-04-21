//! Event loop that reads from the PTY, feeds bytes through `libghostty-vt`,
//! and handles input/resize/shutdown messages.

use std::borrow::Cow;
use std::io::{self, Read, Write};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::thread;
use std::time::Duration;

use parking_lot::Mutex;
use terminal_kernel::event::{OnResize, WindowSize};
use terminal_kernel::index::{Column, Line, Point as AlacPoint};
use terminal_kernel::term::TermMode;
use terminal_kernel::term::cell::{Cell, Flags as CellFlags};
use terminal_kernel::tty::{ChildEvent, EventedPty, EventedReadWrite};
use terminal_kernel::vte::ansi::{Color, CursorShape, CursorStyle, NamedColor, Rgb};

use libghostty_vt::render::CursorVisualStyle;
use libghostty_vt::screen::CellWide;
use libghostty_vt::style::{StyleColor, Underline};
use libghostty_vt::terminal::Mode as GhosttyMode;
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
/// drives it from a dedicated thread. PTY I/O and channel messages are
/// interleaved using non-blocking reads.
pub struct GhosttyEventLoop {
  tx: GhosttyMsgSender,
  rx: std::sync::mpsc::Receiver<GhosttyMsg>,
  pty: terminal_kernel::tty::Pty,
  state: Arc<Mutex<GhosttyTermInner>>,
  event_tx: futures::channel::mpsc::UnboundedSender<terminal_kernel::event::Event>,
  #[cfg(unix)]
  pty_raw_fd: i32,
  /// Initial terminal dimensions.
  initial_cols: u16,
  initial_rows: u16,
  max_scrollback: usize,
  initial_cursor_blink: bool,
}

enum PtyReadStatus {
  Data(usize),
  WouldBlock,
  Eof,
}

impl GhosttyEventLoop {
  pub fn new(
    pty: terminal_kernel::tty::Pty,
    state: Arc<Mutex<GhosttyTermInner>>,
    event_tx: futures::channel::mpsc::UnboundedSender<terminal_kernel::event::Event>,
    #[cfg(unix)] pty_raw_fd: i32,
    initial_cols: u16,
    initial_rows: u16,
    max_scrollback: usize,
    initial_cursor_blink: bool,
  ) -> Self {
    let (tx, rx) = std::sync::mpsc::channel();
    Self {
      tx,
      rx,
      pty,
      state,
      event_tx,
      #[cfg(unix)]
      pty_raw_fd,
      initial_cols,
      initial_rows,
      max_scrollback,
      initial_cursor_blink,
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

  fn read_pty(&mut self, buf: &mut [u8]) -> io::Result<PtyReadStatus> {
    let read_result = self.pty.reader().read(buf);
    match read_result {
      Ok(0) => {
        #[cfg(windows)]
        {
          return Ok(match self.pty.next_child_event() {
            Some(ChildEvent::Exited(_)) => PtyReadStatus::Eof,
            None => PtyReadStatus::WouldBlock,
          });
        }

        #[cfg(not(windows))]
        {
          Ok(PtyReadStatus::Eof)
        }
      }
      Ok(n) => Ok(PtyReadStatus::Data(n)),
      Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Ok(PtyReadStatus::WouldBlock),
      Err(error) => {
        #[cfg(windows)]
        {
          return match self.pty.next_child_event() {
            Some(ChildEvent::Exited(_)) => Ok(PtyReadStatus::Eof),
            None => Err(error),
          };
        }

        #[cfg(not(windows))]
        {
          Err(error)
        }
      }
    }
  }

  fn write_pty(&mut self, mut bytes: &[u8]) -> io::Result<()> {
    while !bytes.is_empty() {
      let write_result = self.pty.writer().write(bytes);
      match write_result {
        Ok(0) => {
          #[cfg(windows)]
          {
            if matches!(self.pty.next_child_event(), Some(ChildEvent::Exited(_))) {
              return Err(io::Error::new(io::ErrorKind::BrokenPipe, "pty exited"));
            }

            thread::sleep(Duration::from_millis(1));
            continue;
          }

          #[cfg(not(windows))]
          {
            return Err(io::Error::new(
              io::ErrorKind::WriteZero,
              "pty write returned zero bytes",
            ));
          }
        }
        Ok(written) => {
          bytes = &bytes[written..];
        }
        Err(ref error) if error.kind() == io::ErrorKind::WouldBlock => {
          thread::sleep(Duration::from_millis(1));
        }
        Err(error) => {
          #[cfg(windows)]
          {
            if matches!(self.pty.next_child_event(), Some(ChildEvent::Exited(_))) {
              return Err(io::Error::new(io::ErrorKind::BrokenPipe, "pty exited"));
            }
          }

          return Err(error);
        }
      }
    }

    self.pty.writer().flush()
  }

  fn drain_pty_writebacks(&mut self, pty_write_rx: &Receiver<Vec<u8>>) -> io::Result<()> {
    loop {
      match pty_write_rx.try_recv() {
        Ok(bytes) => self.write_pty(&bytes)?,
        Err(TryRecvError::Empty | TryRecvError::Disconnected) => return Ok(()),
      }
    }
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
        self.state.lock().sync_from_ghostty(
          vec![],
          AlacPoint::default(),
          CursorStyle::default(),
          TermMode::empty(),
          [None; 256],
          vec![],
        );
        return;
      }
    };

    if let Err(e) = terminal.set_mode(GhosttyMode::CURSOR_BLINKING, self.initial_cursor_blink) {
      eprintln!("ghostty: failed to set cursor blinking mode: {e:?}");
    }

    let mut render_state = match RenderState::new() {
      Ok(rs) => rs,
      Err(e) => {
        eprintln!("ghostty: failed to create render state: {e:?}");
        return;
      }
    };

    let (pty_write_tx, pty_write_rx) = std::sync::mpsc::channel::<Vec<u8>>();

    // Queue PTY write-back for query responses so the loop can flush them using
    // the platform-specific PTY writer it already owns.
    {
      let pty_write_tx = pty_write_tx.clone();
      if let Err(error) = terminal.on_pty_write(move |_term, data| {
        let _ = pty_write_tx.send(data.to_vec());
      }) {
        eprintln!("ghostty: failed to register PTY write callback: {error:?}");
      }
    }

    // Bell → alacritty Bell event.
    {
      let event_tx = self.event_tx.clone();
      if let Err(error) = terminal.on_bell(move |_term| {
        let _ = event_tx.unbounded_send(terminal_kernel::event::Event::Bell);
      }) {
        eprintln!("ghostty: failed to register bell callback: {error:?}");
      }
    }

    // XTVERSION → respond with kazeterm identification.
    {
      if let Err(error) =
        terminal.on_xtversion(|_term| Some(concat!("kazeterm ", env!("CARGO_PKG_VERSION"))))
      {
        eprintln!("ghostty: failed to register xtversion callback: {error:?}");
      }
    }

    // ENQ → respond with empty string (standard).
    {
      if let Err(error) = terminal.on_enquiry(|_term| Some("")) {
        eprintln!("ghostty: failed to register enquiry callback: {error:?}");
      }
    }

    // Device attributes → respond as VT220-compatible terminal.
    {
      use libghostty_vt::terminal::{
        ConformanceLevel, DeviceAttributeFeature, DeviceAttributes, DeviceType,
        PrimaryDeviceAttributes, SecondaryDeviceAttributes, TertiaryDeviceAttributes,
      };
      if let Err(error) = terminal.on_device_attributes(|_term| {
        Some(DeviceAttributes {
          primary: PrimaryDeviceAttributes::new(
            ConformanceLevel::VT220,
            [
              DeviceAttributeFeature::COLUMNS_132,
              DeviceAttributeFeature::SELECTIVE_ERASE,
              DeviceAttributeFeature::ANSI_COLOR,
            ],
          ),
          secondary: SecondaryDeviceAttributes {
            device_type: DeviceType::VT220,
            firmware_version: 1,
            rom_cartridge: 0,
          },
          tertiary: TertiaryDeviceAttributes { unit_id: 0 },
        })
      }) {
        eprintln!("ghostty: failed to register device attributes callback: {error:?}");
      }
    }

    // Color scheme → report dark scheme.
    {
      use libghostty_vt::terminal::ColorScheme;
      if let Err(error) = terminal.on_color_scheme(|_term| Some(ColorScheme::Dark)) {
        eprintln!("ghostty: failed to register color scheme callback: {error:?}");
      }
    }

    // Track scrollback for delta computation.
    let mut prev_scrollback_count: usize = 0;
    let mut last_title = String::new();

    let mut buf = [0u8; 4096];

    loop {
      // Drain the message channel (non-blocking).
      loop {
        match self.rx.try_recv() {
          Ok(GhosttyMsg::Input(bytes)) => {
            if let Err(error) = self.write_pty(&bytes) {
              eprintln!("ghostty: failed to write input to PTY: {error}");
              sync_to_inner(
                &terminal,
                &mut render_state,
                &self.state,
                &mut prev_scrollback_count,
              );
              let _ = self
                .event_tx
                .unbounded_send(terminal_kernel::event::Event::Exit);
              return;
            }
          }
          Ok(GhosttyMsg::Resize(size)) => {
            self.pty.on_resize(size);
            if let Err(error) = terminal.resize(
              size.num_cols,
              size.num_lines,
              size.cell_width as u32,
              size.cell_height as u32,
            ) {
              eprintln!("ghostty: failed to resize terminal: {error:?}");
            }
            sync_to_inner(
              &terminal,
              &mut render_state,
              &self.state,
              &mut prev_scrollback_count,
            );
            emit_title_event_if_changed(&terminal, &self.event_tx, &mut last_title);
          }
          Ok(GhosttyMsg::Shutdown) => {
            return;
          }
          Err(TryRecvError::Empty) => break,
          Err(TryRecvError::Disconnected) => {
            return;
          }
        }
      }

      if let Err(error) = self.drain_pty_writebacks(&pty_write_rx) {
        eprintln!("ghostty: failed to flush PTY writeback: {error}");
        sync_to_inner(
          &terminal,
          &mut render_state,
          &self.state,
          &mut prev_scrollback_count,
        );
        let _ = self
          .event_tx
          .unbounded_send(terminal_kernel::event::Event::Exit);
        return;
      }

      // Read from PTY (non-blocking on Unix).
      match self.read_pty(&mut buf) {
        Ok(PtyReadStatus::Eof) => {
          // EOF — child process exited.
          sync_to_inner(
            &terminal,
            &mut render_state,
            &self.state,
            &mut prev_scrollback_count,
          );
          let _ = self
            .event_tx
            .unbounded_send(terminal_kernel::event::Event::Exit);
          return;
        }
        Ok(PtyReadStatus::Data(n)) => {
          terminal.vt_write(&buf[..n]);
          if let Err(error) = self.drain_pty_writebacks(&pty_write_rx) {
            eprintln!("ghostty: failed to flush PTY writeback: {error}");
            sync_to_inner(
              &terminal,
              &mut render_state,
              &self.state,
              &mut prev_scrollback_count,
            );
            let _ = self
              .event_tx
              .unbounded_send(terminal_kernel::event::Event::Exit);
            return;
          }
          sync_to_inner(
            &terminal,
            &mut render_state,
            &self.state,
            &mut prev_scrollback_count,
          );
          emit_title_event_if_changed(&terminal, &self.event_tx, &mut last_title);
          let _ = self
            .event_tx
            .unbounded_send(terminal_kernel::event::Event::Wakeup);
        }
        Ok(PtyReadStatus::WouldBlock) => {
          thread::sleep(Duration::from_millis(2));
        }
        Err(error) => {
          eprintln!("ghostty: PTY read failed: {error}");
          sync_to_inner(
            &terminal,
            &mut render_state,
            &self.state,
            &mut prev_scrollback_count,
          );
          let _ = self
            .event_tx
            .unbounded_send(terminal_kernel::event::Event::Exit);
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
  shared_state: &Arc<Mutex<GhosttyTermInner>>,
  prev_scrollback_count: &mut usize,
) {
  let snapshot = match render_state.update(terminal) {
    Ok(s) => s,
    Err(error) => {
      eprintln!("ghostty: render state update failed: {error:?}");
      return;
    }
  };

  let num_cols = snapshot.cols().unwrap_or(80) as usize;
  let num_rows = snapshot.rows().unwrap_or(24) as usize;

  // Build visible rows.
  let mut visible_rows: Vec<Vec<Cell>> = Vec::with_capacity(num_rows);
  for row_index in 0..num_rows {
    let is_wrapped = terminal
      .grid_ref(libghostty_vt::terminal::Point::Viewport(
        libghostty_vt::terminal::PointCoordinate {
          x: 0,
          y: row_index as u32,
        },
      ))
      .ok()
      .and_then(|gr| gr.row().ok())
      .and_then(|row| row.is_wrapped().ok())
      .unwrap_or(false);

    let mut row_cells = Vec::with_capacity(num_cols);
    for col_index in 0..num_cols {
      let cell = terminal
        .grid_ref(libghostty_vt::terminal::Point::Viewport(
          libghostty_vt::terminal::PointCoordinate {
            x: col_index as u16,
            y: row_index as u32,
          },
        ))
        .ok()
        .map(|gr| convert_grid_ref_to_cell(&gr))
        .unwrap_or_default();
      row_cells.push(cell);
    }

    if is_wrapped {
      if let Some(last) = row_cells.last_mut() {
        last.flags.insert(CellFlags::WRAPLINE);
      }
    }

    visible_rows.push(row_cells);
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

fn emit_title_event_if_changed(
  terminal: &Terminal<'_, '_>,
  event_tx: &futures::channel::mpsc::UnboundedSender<terminal_kernel::event::Event>,
  last_title: &mut String,
) {
  // libghostty only guarantees the updated title is readable after the
  // title_changed callback returns, so query it after vt_write/sync instead.
  let title = terminal.title().unwrap_or("").to_string();
  if title != *last_title {
    *last_title = title.clone();
    let _ = event_tx.unbounded_send(terminal_kernel::event::Event::Title(title));
  }
}

#[cfg(test)]
mod tests {
  use futures::{FutureExt as _, StreamExt as _, channel::mpsc::unbounded, executor::block_on};

  use super::emit_title_event_if_changed;
  use libghostty_vt::{Terminal, TerminalOptions};
  use terminal_kernel::event::Event;

  fn register_effects(terminal: &mut Terminal<'_, '_>) {
    use libghostty_vt::terminal::{
      ColorScheme, ConformanceLevel, DeviceAttributeFeature, DeviceAttributes, DeviceType,
      PrimaryDeviceAttributes, SecondaryDeviceAttributes, TertiaryDeviceAttributes,
    };

    terminal
      .on_pty_write(|_, _| {})
      .expect("register on_pty_write");
    terminal.on_bell(|_| {}).expect("register on_bell");
    terminal
      .on_xtversion(|_| Some(concat!("kazeterm ", env!("CARGO_PKG_VERSION"))))
      .expect("register on_xtversion");
    terminal
      .on_enquiry(|_| Some(""))
      .expect("register on_enquiry");
    terminal
      .on_device_attributes(|_| {
        Some(DeviceAttributes {
          primary: PrimaryDeviceAttributes::new(
            ConformanceLevel::VT220,
            [
              DeviceAttributeFeature::COLUMNS_132,
              DeviceAttributeFeature::SELECTIVE_ERASE,
              DeviceAttributeFeature::ANSI_COLOR,
            ],
          ),
          secondary: SecondaryDeviceAttributes {
            device_type: DeviceType::VT220,
            firmware_version: 1,
            rom_cartridge: 0,
          },
          tertiary: TertiaryDeviceAttributes { unit_id: 0 },
        })
      })
      .expect("register on_device_attributes");
    terminal
      .on_color_scheme(|_| Some(ColorScheme::Dark))
      .expect("register on_color_scheme");
  }

  fn pwsh_title_block() -> &'static [u8] {
    b"\x1b[?25l\x1b[2J\x1b[m\x1b[H\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\x1b[H\x1b]0;C:\\Program Files\\PowerShell\\7\\pwsh.exe\x07\x1b[?25h"
  }

  #[test]
  fn emits_title_after_vt_write() {
    let mut terminal = Terminal::new(TerminalOptions {
      cols: 80,
      rows: 24,
      max_scrollback: 0,
    })
    .expect("ghostty terminal should initialize");
    let (event_tx, mut event_rx) = unbounded();
    let mut last_title = String::new();

    terminal.vt_write(b"\x1b]2;Kazeterm Ghostty\x1b\\");
    emit_title_event_if_changed(&terminal, &event_tx, &mut last_title);

    assert_eq!(last_title, "Kazeterm Ghostty");
    match block_on(event_rx.next()) {
      Some(Event::Title(title)) => assert_eq!(title, "Kazeterm Ghostty"),
      other => panic!("expected title event, got {other:?}"),
    }
  }

  #[test]
  fn skips_duplicate_title_events() {
    let mut terminal = Terminal::new(TerminalOptions {
      cols: 80,
      rows: 24,
      max_scrollback: 0,
    })
    .expect("ghostty terminal should initialize");
    let (event_tx, mut event_rx) = unbounded();
    let mut last_title = String::new();

    terminal.vt_write(b"\x1b]2;Kazeterm Ghostty\x1b\\");
    emit_title_event_if_changed(&terminal, &event_tx, &mut last_title);
    emit_title_event_if_changed(&terminal, &event_tx, &mut last_title);

    match block_on(event_rx.next()) {
      Some(Event::Title(title)) => assert_eq!(title, "Kazeterm Ghostty"),
      other => panic!("expected first title event, got {other:?}"),
    }
    assert!(event_rx.next().now_or_never().is_none());
  }

  #[cfg(target_os = "windows")]
  #[test]
  fn vt_write_handles_windows_line_endings() {
    let mut terminal = Terminal::new(TerminalOptions {
      cols: 58,
      rows: 26,
      max_scrollback: 10_000,
    })
    .expect("ghostty terminal should initialize");

    terminal.vt_write(b"\r");
    terminal.vt_write(b"\n");
    terminal.vt_write(b"\r\n");
    terminal.vt_write(b"\r\n\r\n");
  }

  #[cfg(target_os = "windows")]
  #[test]
  fn vt_write_handles_pwsh_startup_block_with_effects() {
    let mut terminal = Terminal::new(TerminalOptions {
      cols: 58,
      rows: 26,
      max_scrollback: 10_000,
    })
    .expect("ghostty terminal should initialize");

    register_effects(&mut terminal);
    terminal.vt_write(pwsh_title_block());
  }
}

// ---------------------------------------------------------------------------
// Cell conversion helpers
// ---------------------------------------------------------------------------

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
