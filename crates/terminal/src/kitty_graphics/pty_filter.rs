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

#[cfg(unix)]
mod unix {
  use std::fs::File;
  use std::io::{self, Read};
  use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
  use std::sync::atomic::{AtomicU32, Ordering};
  use std::sync::mpsc;
  use std::sync::Arc;

  use alacritty_terminal::event::{OnResize, WindowSize};
  use alacritty_terminal::tty::{ChildEvent, EventedPty, EventedReadWrite, Pty};
  use polling::{Event, PollMode, Poller};

  use super::super::command::RawGraphicsCommand;

  /// Callback that tries to get the cursor position from the terminal.
  /// Returns `Some((absolute_line, column))` on success, `None` if lock unavailable.
  pub type CursorFn = Box<dyn Fn() -> Option<(i32, i32)> + Send + Sync>;

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

  /// A `Read` adapter that filters APC graphics sequences from PTY output.
  ///
  /// Reads from a dup'd PTY master fd, strips out `\x1b_G...\x1b\\` sequences,
  /// sends their content to a channel, and returns only non-APC bytes to the caller.
  /// Returns `WouldBlock` when all bytes in a read were APC data.
  pub struct FilteringReader {
    inner: File,
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

  /// Parsed APC parameters relevant to cursor advancement.
  struct ApcParams {
    display_rows: u32,
    source_height: u32,
    format: u32,
    more_chunks: bool,
    is_display: bool,
  }

  /// Parse APC params relevant to cursor advancement and image dimensions.
  fn parse_apc_params(apc_content: &[u8]) -> ApcParams {
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
  fn try_png_height_from_payload(apc_content: &[u8]) -> u32 {
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

  impl FilteringReader {
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
        // Add 2 lines of padding below the image before the next prompt.
        let cnl = format!("\x1b[{}E", rows + 2);
        self.pending.extend_from_slice(cnl.as_bytes());
        self.cnl_injected = true;
      }
    }
  }

  impl Read for FilteringReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
      // Check for terminal-computed CNL feedback (from place_image).
      let feedback_rows = self.pending_cnl.swap(0, Ordering::AcqRel);
      if feedback_rows > 0 && !self.cnl_injected {
        let cnl = format!("\x1b[{}E", feedback_rows + 2);
        // Prepend to any existing pending data.
        let mut new_pending = Vec::with_capacity(cnl.len() + self.pending.len());
        new_pending.extend_from_slice(cnl.as_bytes());
        if self.pending_pos < self.pending.len() {
          new_pending.extend_from_slice(&self.pending[self.pending_pos..]);
        }
        self.pending = new_pending;
        self.pending_pos = 0;
        self.cnl_injected = true;
      }

      // Drain any leftover filtered bytes from a previous read.
      if self.pending_pos < self.pending.len() {
        let avail = &self.pending[self.pending_pos..];
        let n = avail.len().min(buf.len());
        buf[..n].copy_from_slice(&avail[..n]);
        self.pending_pos += n;
        if self.pending_pos >= self.pending.len() {
          self.pending.clear();
          self.pending_pos = 0;
        }
        return Ok(n);
      }

      // Read raw bytes from the PTY master.
      let mut raw = [0u8; 8192];
      let n = self.inner.read(&mut raw)?;
      if n == 0 {
        return Ok(0);
      }

      // Run the APC state machine over the raw bytes.
      self.pending.clear();
      self.pending_pos = 0;

      for &byte in &raw[..n] {
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
              // Not APC — pass through the ESC and this byte.
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
              // Complete APC sequence. Check for Kitty graphics prefix 'G'.
              if self.apc_buf.first() == Some(&b'G') {
                let (cursor_line, cursor_column) = self.capture_cursor();
                let cmd_data = self.apc_buf[1..].to_vec();
                let params = parse_apc_params(&cmd_data);

                // Inject cursor advancement on display actions.
                // For multi-chunk: inject on first chunk (which has r=/v= params),
                // skip subsequent chunks via cnl_injected flag.
                if params.is_display && !self.cnl_injected {
                  let effective_rows = if params.display_rows > 0 {
                    params.display_rows
                  } else if params.source_height > 0 {
                    // Estimate ~20 pixels per cell row.
                    (params.source_height + 19) / 20
                  } else if params.format == 100 && !params.more_chunks {
                    // PNG: try reading height from image header.
                    let h = try_png_height_from_payload(&cmd_data);
                    if h > 0 { (h + 19) / 20 } else { 0 }
                  } else {
                    0
                  };
                  self.inject_cnl(effective_rows);
                }

                // Reset cnl_injected flag after last chunk so next image can inject.
                if !params.more_chunks {
                  self.cnl_injected = false;
                }

                let _ = self.graphics_tx.send(RawGraphicsCommand {
                  data: cmd_data,
                  cursor_line,
                  cursor_column,
                  clear_all: false,
                });
              }
              self.apc_buf.clear();
              self.state = FilterState::Normal;
            } else {
              // ESC inside APC not followed by '\' — keep collecting.
              self.apc_buf.push(0x1B);
              self.apc_buf.push(byte);
              self.state = FilterState::ApcCollect;
            }
          }
        }
      }

      // Detect terminal clear/reset sequences in pass-through bytes.
      // ESC[2J (erase display), ESC[3J (erase display+scrollback), ESC c (RIS).
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
        });
      }

      if self.pending.is_empty() {
        // All bytes were APC data — signal "no data yet" to the EventLoop.
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

  /// Transparent PTY wrapper that filters Kitty graphics APC sequences.
  ///
  /// Wraps an alacritty `Pty`, overriding `reader()` with a `FilteringReader`
  /// and delegating everything else (poll registration, writing, signals,
  /// resize) to the original `Pty`.
  pub struct GraphicsPtyFilter {
    reader: FilteringReader,
    pty: Pty,
  }

  impl GraphicsPtyFilter {
    /// Create a graphics-filtering PTY wrapper that takes ownership of a `Pty`.
    ///
    /// `cursor_fn` is called (via try-lock) to capture the cursor position
    /// when an APC graphics sequence is intercepted.
    ///
    /// Returns `(filter, pending_cnl, graphics_rx)`:
    /// - `pending_cnl`: shared atomic for terminal to request cursor advancement
    /// - `graphics_rx`: receives Kitty graphics commands with cursor positions
    pub fn new(
      pty: Pty,
      cursor_fn: CursorFn,
    ) -> io::Result<(Self, Arc<AtomicU32>, mpsc::Receiver<RawGraphicsCommand>)> {
      // Dup the master fd so the FilteringReader has its own fd for reading.
      // The original fd stays in the Pty for poll registration and writing.
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
        state: FilterState::Normal,
        apc_buf: Vec::with_capacity(4096),
        pending: Vec::with_capacity(8192),
        pending_pos: 0,
        graphics_tx,
        cursor_fn,
        last_cursor: (0, 0),
        cnl_injected: false,
        pending_cnl: Arc::clone(&pending_cnl),
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

  // Delegate poll registration, writer, and deregistration to the inner Pty.
  // Only reader() is overridden to return the FilteringReader.

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

#[cfg(unix)]
pub use unix::GraphicsPtyFilter;
