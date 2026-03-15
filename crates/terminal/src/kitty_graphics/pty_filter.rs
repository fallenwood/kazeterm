//! PTY wrapper that filters Kitty graphics protocol APC sequences.
//!
//! The VTE parser used by alacritty_terminal silently discards APC sequences
//! (`\x1b_...\x1b\\`). This module provides a transparent PTY wrapper that
//! intercepts APC graphics commands in the `Read` implementation, extracts
//! them to a channel, and returns only non-APC bytes to alacritty.
//!
//! ## Architecture
//!
//! ```text
//!   Shell ──► PTY slave ──► PTY master
//!                               │
//!                        FilteringReader::read()
//!                         ┌─────┴─────┐
//!                     APC bytes    Normal bytes
//!                         │            │
//!                   graphics_tx    returned to caller
//!                         │            │
//!                   Terminal::sync()   VTE parser
//! ```
//!
//! No pipe or filter thread is needed. The filtering happens inline in
//! `Read::read()`, using the same PTY master fd that alacritty expects.
//! The wrapper delegates all poll registration, writing, signal handling,
//! and resize to the original `Pty`.

use std::io;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{mpsc, Arc};

use super::command::RawGraphicsCommand;

/// Callback that tries to get the cursor position from the terminal.
/// Returns `Some((absolute_line, column))` on success, `None` if lock unavailable.
pub type CursorFn = Box<dyn Fn() -> Option<(i32, i32)> + Send + Sync>;

/// Extra blank lines inserted below an image before the next prompt.
pub(super) const IMAGE_BOTTOM_PADDING: u32 = 2;

/// APC filter state machine states.
#[derive(Debug, Clone, Copy, PartialEq)]
enum FilterState {
  /// Normal passthrough mode.
  Normal,
  /// Saw ESC (0x1B), waiting for next byte.
  Escape,
  /// Inside APC sequence (\x1b_G...), collecting bytes.
  ApcCollect,
  /// Inside APC, saw ESC — waiting for '\' to end sequence.
  ApcEscape,
}

/// Parsed APC parameters relevant to cursor advancement.
pub(super) struct ApcParams {
  pub display_rows: u32,
  pub source_height: u32,
  pub format: u32,
  pub more_chunks: bool,
  pub is_display: bool,
}

/// Parse APC params relevant to cursor advancement and image dimensions.
pub(super) fn parse_apc_params(apc_content: &[u8]) -> ApcParams {
  let params_end = apc_content
    .iter()
    .position(|&b| b == b';')
    .unwrap_or(apc_content.len());
  let params = std::str::from_utf8(&apc_content[..params_end]).unwrap_or("");
  let mut result = ApcParams {
    display_rows: 0,
    source_height: 0,
    format: 32, // default: RGBA
    more_chunks: false,
    is_display: true, // default action is TransmitAndDisplay
  };
  for pair in params.split(',') {
    if let Some((key, value)) = pair.split_once('=') {
      match key.trim() {
        "r" => result.display_rows = value.parse().unwrap_or(0),
        "v" => result.source_height = value.parse().unwrap_or(0),
        "f" => result.format = value.parse().unwrap_or(32),
        "m" => result.more_chunks = value == "1",
        "a" => result.is_display = matches!(value, "T" | "p"),
        _ => {}
      }
    }
  }
  result
}

/// Try to extract image height from PNG header in base64-encoded payload.
/// PNG IHDR: sig(8) + len(4) + "IHDR"(4) + width(4) + height(4) = 24 bytes.
/// 24 bytes needs 32 base64 chars.
pub(super) fn try_png_height_from_payload(apc_content: &[u8]) -> u32 {
  let sep = match apc_content.iter().position(|&b| b == b';') {
    Some(p) => p,
    None => return 0,
  };
  let b64 = &apc_content[sep + 1..];
  if b64.len() < 32 {
    return 0;
  }

  // Decode first 32 base64 chars → 24 bytes.
  let mut decoded = [0u8; 24];
  let mut di = 0;
  let mut buf = [0u8; 4];
  let mut bi = 0;

  for &ch in &b64[..32] {
    let val = match ch {
      b'A'..=b'Z' => ch - b'A',
      b'a'..=b'z' => ch - b'a' + 26,
      b'0'..=b'9' => ch - b'0' + 52,
      b'+' => 62,
      b'/' => 63,
      b'=' => 0,
      _ => continue,
    };
    buf[bi] = val;
    bi += 1;
    if bi == 4 {
      if di < 24 {
        decoded[di] = (buf[0] << 2) | (buf[1] >> 4);
      }
      if di + 1 < 24 {
        decoded[di + 1] = (buf[1] << 4) | (buf[2] >> 2);
      }
      if di + 2 < 24 {
        decoded[di + 2] = (buf[2] << 6) | buf[3];
      }
      di += 3;
      bi = 0;
    }
  }

  // Check PNG signature: 89 50 4E 47 0D 0A 1A 0A
  if decoded[0..8] != [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A] {
    return 0;
  }
  // IHDR height is at bytes 20-23 (big-endian u32).
  u32::from_be_bytes([decoded[20], decoded[21], decoded[22], decoded[23]])
}

/// Shared APC filter core logic used by both Unix and Windows implementations.
///
/// This struct holds the state machine and buffers for APC filtering.
/// Platform-specific readers wrap this and delegate byte processing to it.
struct ApcFilterCore {
  state: FilterState,
  apc_buf: Vec<u8>,
  /// Filtered bytes not yet returned to the caller.
  pending: Vec<u8>,
  pending_pos: usize,
  graphics_tx: mpsc::Sender<RawGraphicsCommand>,
  /// Callback to try-lock the terminal and get cursor position.
  cursor_fn: CursorFn,
  /// Cached cursor position from last successful try-lock.
  last_cursor: (i32, i32),
  /// Whether we've already injected CNL for the current image sequence.
  cnl_injected: bool,
  /// Shared atomic: terminal sets this to height_cells after place_image().
  /// Filter injects CNL on next read and resets to 0.
  pending_cnl: Arc<AtomicU32>,
}

impl ApcFilterCore {
  fn new(
    graphics_tx: mpsc::Sender<RawGraphicsCommand>,
    cursor_fn: CursorFn,
    pending_cnl: Arc<AtomicU32>,
  ) -> Self {
    ApcFilterCore {
      state: FilterState::Normal,
      apc_buf: Vec::with_capacity(4096),
      pending: Vec::with_capacity(8192),
      pending_pos: 0,
      graphics_tx,
      cursor_fn,
      last_cursor: (0, 0),
      cnl_injected: false,
      pending_cnl,
    }
  }

  /// Try to capture cursor position. Updates cache on success.
  fn capture_cursor(&mut self) -> (i32, i32) {
    if let Some(pos) = (self.cursor_fn)() {
      self.last_cursor = pos;
    }
    self.last_cursor
  }

  /// Inject CNL escape into the pending buffer to advance cursor past image.
  fn inject_cnl(&mut self, rows: u32) {
    if rows > 0 {
      let cnl = format!("\x1b[{}E", rows + IMAGE_BOTTOM_PADDING);
      self.pending.extend_from_slice(cnl.as_bytes());
      self.cnl_injected = true;
    }
  }

  /// Check for terminal-computed CNL feedback and prepend to pending buffer.
  fn check_pending_cnl(&mut self) {
    let feedback_rows = self.pending_cnl.swap(0, Ordering::AcqRel);
    if feedback_rows > 0 {
      if !self.cnl_injected {
        let cnl = format!("\x1b[{}E", feedback_rows + IMAGE_BOTTOM_PADDING);
        let mut new_pending = Vec::with_capacity(cnl.len() + self.pending.len());
        new_pending.extend_from_slice(cnl.as_bytes());
        if self.pending_pos < self.pending.len() {
          new_pending.extend_from_slice(&self.pending[self.pending_pos..]);
        }
        self.pending = new_pending;
        self.pending_pos = 0;
        self.cnl_injected = true;
      } else {
        // CNL was already injected for this image (by the filter's own
        // estimate or the pipe server). Just consume the feedback value
        // and reset so the next image can get CNL.
        self.cnl_injected = false;
      }
    }
  }

  /// Drain any leftover filtered bytes from a previous read.
  /// Returns `Some(n)` if bytes were drained, `None` if nothing to drain.
  fn drain_pending(&mut self, buf: &mut [u8]) -> Option<usize> {
    if self.pending_pos < self.pending.len() {
      let avail = &self.pending[self.pending_pos..];
      let n = avail.len().min(buf.len());
      buf[..n].copy_from_slice(&avail[..n]);
      self.pending_pos += n;
      if self.pending_pos >= self.pending.len() {
        self.pending.clear();
        self.pending_pos = 0;
      }
      Some(n)
    } else {
      None
    }
  }

  /// Process raw bytes through the APC state machine.
  fn process_bytes(&mut self, raw: &[u8]) {
    self.pending.clear();
    self.pending_pos = 0;

    for &byte in raw {
      match self.state {
        FilterState::Normal => {
          if byte == 0x1B {
            self.state = FilterState::Escape;
          } else {
            self.pending.push(byte);
          }
        }
        FilterState::Escape => {
          if byte == b'_' {
            self.state = FilterState::ApcCollect;
            self.apc_buf.clear();
          } else {
            self.pending.push(0x1B);
            self.pending.push(byte);
            self.state = FilterState::Normal;
          }
        }
        FilterState::ApcCollect => {
          if byte == 0x1B {
            self.state = FilterState::ApcEscape;
          } else {
            self.apc_buf.push(byte);
          }
        }
        FilterState::ApcEscape => {
          if byte == b'\\' {
            if self.apc_buf.first() == Some(&b'G') {
              let (cursor_line, cursor_column) = self.capture_cursor();
              let cmd_data = self.apc_buf[1..].to_vec();
              let params = parse_apc_params(&cmd_data);

              if params.is_display && !self.cnl_injected {
                let effective_rows = if params.display_rows > 0 {
                  params.display_rows
                } else if params.source_height > 0 {
                  (params.source_height + 19) / 20
                } else if params.format == 100 && !params.more_chunks {
                  let h = try_png_height_from_payload(&cmd_data);
                  if h > 0 { (h + 19) / 20 } else { 0 }
                } else {
                  0
                };
                self.inject_cnl(effective_rows);
              }

              // Note: cnl_injected is NOT reset here. It is reset by
              // check_pending_cnl when it consumes Terminal's feedback
              // value, ensuring exactly one CNL injection per image.

              let _ = self.graphics_tx.send(RawGraphicsCommand {
                data: cmd_data,
                cursor_line,
                cursor_column,
                clear_all: false,
                from_filter: true,
              });
            }
            self.apc_buf.clear();
            self.state = FilterState::Normal;
          } else {
            self.apc_buf.push(0x1B);
            self.apc_buf.push(byte);
            self.state = FilterState::ApcCollect;
          }
        }
      }
    }

    self.detect_and_send_clear();
  }

  /// Scan pending bytes for terminal clear/reset sequences and send a
  /// `clear_all` command if found.
  fn detect_and_send_clear(&self) {
    let has_clear = self
      .pending
      .windows(4)
      .any(|w| w == b"\x1b[2J" || w == b"\x1b[3J")
      || self.pending.windows(2).any(|w| w == b"\x1bc");
    if has_clear {
      let _ = self.graphics_tx.send(RawGraphicsCommand {
        data: Vec::new(),
        cursor_line: 0,
        cursor_column: 0,
        clear_all: true,
        from_filter: true,
      });
    }
  }

  /// Finalize a read by returning filtered bytes from the pending buffer.
  fn finalize_read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
    if self.pending.is_empty() {
      return Err(io::Error::from(io::ErrorKind::WouldBlock));
    }

    let n = self.pending.len().min(buf.len());
    buf[..n].copy_from_slice(&self.pending[..n]);
    self.pending_pos = n;
    if self.pending_pos >= self.pending.len() {
      self.pending.clear();
      self.pending_pos = 0;
    }
    Ok(n)
  }
}

#[cfg(unix)]
mod unix {
  use std::fs::File;
  use std::io::{self, Read};
  use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
  use std::sync::atomic::AtomicU32;
  use std::sync::{mpsc, Arc};

  use alacritty_terminal::event::{OnResize, WindowSize};
  use alacritty_terminal::tty::{ChildEvent, EventedPty, EventedReadWrite, Pty};
  use polling::{Event, PollMode, Poller};

  use super::{ApcFilterCore, CursorFn, RawGraphicsCommand};

  /// A `Read` adapter that filters APC graphics sequences from PTY output.
  pub struct FilteringReader {
    inner: File,
    filter: ApcFilterCore,
  }

  impl Read for FilteringReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
      self.filter.check_pending_cnl();
      if let Some(n) = self.filter.drain_pending(buf) {
        return Ok(n);
      }

      let mut raw = [0u8; 8192];
      let n = self.inner.read(&mut raw)?;
      if n == 0 {
        return Ok(0);
      }

      self.filter.process_bytes(&raw[..n]);
      self.filter.finalize_read(buf)
    }
  }

  /// Transparent PTY wrapper that filters Kitty graphics APC sequences.
  pub struct GraphicsPtyFilter {
    reader: FilteringReader,
    pty: Pty,
  }

  impl GraphicsPtyFilter {
    /// Create a graphics-filtering PTY wrapper that takes ownership of a `Pty`.
    pub fn new(
      pty: Pty,
      cursor_fn: CursorFn,
    ) -> io::Result<(Self, Arc<AtomicU32>, mpsc::Receiver<RawGraphicsCommand>)> {
      let master_fd = pty.file().as_raw_fd();
      let read_fd = unsafe { libc::dup(master_fd) };
      if read_fd < 0 {
        return Err(io::Error::last_os_error());
      }
      let read_file = unsafe { File::from_raw_fd(read_fd) };

      let (graphics_tx, graphics_rx) = mpsc::channel();
      let pending_cnl = Arc::new(AtomicU32::new(0));

      let reader = FilteringReader {
        inner: read_file,
        filter: ApcFilterCore::new(graphics_tx, cursor_fn, Arc::clone(&pending_cnl)),
      };

      Ok((GraphicsPtyFilter { reader, pty }, pending_cnl, graphics_rx))
    }

    /// Get the raw PTY master fd (for tcgetpgrp / PtyProcessInfo).
    pub fn pty_fd(&self) -> RawFd {
      self.pty.file().as_raw_fd()
    }

    /// Get the child process PID.
    pub fn child_pid(&self) -> u32 {
      self.pty.child().id()
    }
  }

  impl EventedReadWrite for GraphicsPtyFilter {
    type Reader = FilteringReader;
    type Writer = File;

    #[inline]
    unsafe fn register(
      &mut self,
      poll: &Arc<Poller>,
      interest: Event,
      poll_mode: PollMode,
    ) -> io::Result<()> {
      unsafe { self.pty.register(poll, interest, poll_mode) }
    }

    #[inline]
    fn reregister(
      &mut self,
      poll: &Arc<Poller>,
      interest: Event,
      poll_mode: PollMode,
    ) -> io::Result<()> {
      self.pty.reregister(poll, interest, poll_mode)
    }

    #[inline]
    fn deregister(&mut self, poll: &Arc<Poller>) -> io::Result<()> {
      self.pty.deregister(poll)
    }

    #[inline]
    fn reader(&mut self) -> &mut FilteringReader {
      &mut self.reader
    }

    #[inline]
    fn writer(&mut self) -> &mut File {
      self.pty.writer()
    }
  }

  impl EventedPty for GraphicsPtyFilter {
    #[inline]
    fn next_child_event(&mut self) -> Option<ChildEvent> {
      self.pty.next_child_event()
    }
  }

  impl OnResize for GraphicsPtyFilter {
    #[inline]
    fn on_resize(&mut self, window_size: WindowSize) {
      self.pty.on_resize(window_size);
    }
  }
}

#[cfg(windows)]
mod windows {
  use std::io::{self, Read};
  use std::sync::atomic::AtomicU32;
  use std::sync::{mpsc, Arc};

  use alacritty_terminal::event::{OnResize, WindowSize};
  use alacritty_terminal::tty::{ChildEvent, EventedPty, EventedReadWrite, Pty};
  use polling::{Event, PollMode, Poller};

  use super::{ApcFilterCore, CursorFn, RawGraphicsCommand};

  /// A `Read` adapter that filters APC graphics sequences from PTY output on Windows.
  ///
  /// Uses a raw pointer back to the `Pty` (which lives in a `Box` with a stable
  /// heap address) to read from the PTY's reader inline. This is safe because
  /// `reader()` and `writer()` are never called simultaneously — alacritty's
  /// EventLoop is single-threaded.
  pub struct WindowsFilteringReader {
    pty_ptr: *mut Pty,
    filter: ApcFilterCore,
  }

  // SAFETY: The Pty pointer is only accessed from the EventLoop thread,
  // which is the same thread that calls Read::read(). The Pty itself is
  // Send (alacritty requires it for EventLoop).
  unsafe impl Send for WindowsFilteringReader {}

  impl Read for WindowsFilteringReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
      self.filter.check_pending_cnl();
      if let Some(n) = self.filter.drain_pending(buf) {
        return Ok(n);
      }

      let mut raw = [0u8; 8192];
      // SAFETY: pty_ptr points to a Box<Pty> with a stable heap address.
      // The EventLoop is single-threaded, so no concurrent access to reader().
      let pty = unsafe { &mut *self.pty_ptr };
      let n = pty.reader().read(&mut raw)?;
      if n == 0 {
        return Ok(0);
      }

      self.filter.process_bytes(&raw[..n]);
      self.filter.finalize_read(buf)
    }
  }

  /// Transparent PTY wrapper that filters Kitty graphics APC sequences on Windows.
  ///
  /// Wraps an alacritty `Pty` in a `Box` (for stable heap address) and overrides
  /// `reader()` with a `WindowsFilteringReader` that filters inline. All other
  /// operations (poll registration, writing, signals, resize) delegate to the
  /// original `Pty`.
  pub struct GraphicsPtyFilter {
    // IMPORTANT: `pty` must be declared before `reader` for correct drop order.
    // `reader` holds a raw pointer to `pty`.
    pty: Box<Pty>,
    reader: WindowsFilteringReader,
  }

  impl GraphicsPtyFilter {
    /// Create a graphics-filtering PTY wrapper that takes ownership of a `Pty`.
    pub fn new(
      pty: Pty,
      cursor_fn: CursorFn,
    ) -> io::Result<(Self, Arc<AtomicU32>, mpsc::Receiver<RawGraphicsCommand>)> {
      let mut pty = Box::new(pty);
      let pty_ptr: *mut Pty = &mut *pty;

      let (graphics_tx, graphics_rx) = mpsc::channel();
      let pending_cnl = Arc::new(AtomicU32::new(0));

      let reader = WindowsFilteringReader {
        pty_ptr,
        filter: ApcFilterCore::new(graphics_tx, cursor_fn, Arc::clone(&pending_cnl)),
      };

      Ok((GraphicsPtyFilter { pty, reader }, pending_cnl, graphics_rx))
    }

    /// Create a graphics-filtering PTY wrapper using an existing channel and
    /// pending-CNL atomic. Used on Windows where the graphics pipe also sends
    /// commands on the same channel.
    pub fn new_shared(
      pty: Pty,
      cursor_fn: CursorFn,
      graphics_tx: mpsc::Sender<RawGraphicsCommand>,
      pending_cnl: Arc<AtomicU32>,
    ) -> io::Result<Self> {
      let mut pty = Box::new(pty);
      let pty_ptr: *mut Pty = &mut *pty;

      let reader = WindowsFilteringReader {
        pty_ptr,
        filter: ApcFilterCore::new(graphics_tx, cursor_fn, pending_cnl),
      };

      Ok(GraphicsPtyFilter { pty, reader })
    }

    /// Get the child process handle (for `GetProcessId` / PtyProcessInfo).
    pub fn child_handle(&self) -> isize {
      self.pty.child_watcher().raw_handle() as isize
    }

    /// Get the child process PID.
    pub fn child_pid(&self) -> u32 {
      self
        .pty
        .child_watcher()
        .pid()
        .map(|p| p.get())
        .unwrap_or(0)
    }
  }

  impl EventedReadWrite for GraphicsPtyFilter {
    type Reader = WindowsFilteringReader;
    type Writer = <Pty as EventedReadWrite>::Writer;

    #[inline]
    unsafe fn register(
      &mut self,
      poll: &Arc<Poller>,
      interest: Event,
      poll_mode: PollMode,
    ) -> io::Result<()> {
      unsafe { self.pty.register(poll, interest, poll_mode) }
    }

    #[inline]
    fn reregister(
      &mut self,
      poll: &Arc<Poller>,
      interest: Event,
      poll_mode: PollMode,
    ) -> io::Result<()> {
      self.pty.reregister(poll, interest, poll_mode)
    }

    #[inline]
    fn deregister(&mut self, poll: &Arc<Poller>) -> io::Result<()> {
      self.pty.deregister(poll)
    }

    #[inline]
    fn reader(&mut self) -> &mut WindowsFilteringReader {
      &mut self.reader
    }

    #[inline]
    fn writer(&mut self) -> &mut Self::Writer {
      self.pty.writer()
    }
  }

  impl EventedPty for GraphicsPtyFilter {
    #[inline]
    fn next_child_event(&mut self) -> Option<ChildEvent> {
      self.pty.next_child_event()
    }
  }

  impl OnResize for GraphicsPtyFilter {
    #[inline]
    fn on_resize(&mut self, window_size: WindowSize) {
      self.pty.on_resize(window_size);
    }
  }
}

#[cfg(unix)]
pub use unix::GraphicsPtyFilter;
#[cfg(windows)]
pub use windows::GraphicsPtyFilter;
