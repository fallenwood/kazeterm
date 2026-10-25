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
//!                 graphics worker      VTE parser
//! ```
//!
//! No pipe or filter thread is needed. The filtering happens inline in
//! `Read::read()`, using the same PTY master fd that alacritty expects.
//! The wrapper delegates all poll registration, writing, signal handling,
//! and resize to the original `Pty`.

/// Compute numeric version for Secondary DA response.
/// Encodes as major*10000 + minor*100 + patch.
fn da2_version_number() -> u32 {
  let major: u32 = env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap_or(0);
  let minor: u32 = env!("CARGO_PKG_VERSION_MINOR").parse().unwrap_or(0);
  let patch: u32 = env!("CARGO_PKG_VERSION_PATCH").parse().unwrap_or(0);
  major * 10000 + minor * 100 + patch
}

pub const KITTY_KEYBOARD_DISAMBIGUATE_ESCAPE_CODES: u32 = 0b1;
pub const KITTY_KEYBOARD_REPORT_EVENT_TYPES: u32 = 0b10;
pub const KITTY_KEYBOARD_REPORT_ALTERNATE_KEYS: u32 = 0b100;
pub const KITTY_KEYBOARD_REPORT_ALL_KEYS_AS_ESCAPE_CODES: u32 = 0b1000;
pub const KITTY_KEYBOARD_REPORT_ASSOCIATED_TEXT: u32 = 0b10000;
pub const WINDOWS_CONPTY_WIN32_INPUT_MODE: u32 = 1 << 30;

/// Maximum retained APC bytes, including the G prefix. Oversized commands are
/// consumed through ST but remain large enough for the presentation parser to reject.
pub const MAX_GRAPHICS_APC_BYTES: usize = 8192;

const MAX_KITTY_KEYBOARD_STACK_DEPTH: usize = 16;

#[derive(Debug, Clone, Default)]
struct KeyboardModeState {
  flags: u32,
  stack: Vec<u32>,
}

impl KeyboardModeState {
  fn apply_flags(&mut self, flags: u32, mode: u32) {
    match mode {
      2 => self.flags |= flags,
      3 => self.flags &= !flags,
      _ => self.flags = flags,
    }
  }

  fn push(&mut self, flags: u32) {
    if self.stack.len() >= MAX_KITTY_KEYBOARD_STACK_DEPTH {
      self.stack.remove(0);
    }
    self.stack.push(self.flags);
    self.flags = flags;
  }

  fn pop(&mut self, count: u32) {
    for _ in 0..count.max(1) {
      if let Some(flags) = self.stack.pop() {
        self.flags = flags;
      } else {
        self.flags = 0;
        break;
      }
    }
  }
}

#[derive(Debug, Clone, Default)]
struct KeyboardModeTracker {
  main: KeyboardModeState,
  alternate: KeyboardModeState,
  alternate_screen: bool,
  win32_input_mode: bool,
}

impl KeyboardModeTracker {
  fn current_flags(&self) -> u32 {
    self.active_state().flags
  }

  fn input_flags(&self) -> u32 {
    let mut flags = self.current_flags();
    if self.win32_input_mode {
      flags |= WINDOWS_CONPTY_WIN32_INPUT_MODE;
    }
    flags
  }

  fn active_state(&self) -> &KeyboardModeState {
    if self.alternate_screen {
      &self.alternate
    } else {
      &self.main
    }
  }

  fn active_state_mut(&mut self) -> &mut KeyboardModeState {
    if self.alternate_screen {
      &mut self.alternate
    } else {
      &mut self.main
    }
  }

  fn handle_csi_u(&mut self, csi_buf: &[u8]) -> KeyboardProtocolAction {
    match csi_buf.first().copied() {
      Some(b'?') if csi_buf.len() == 1 => {
        KeyboardProtocolAction::Reply(format!("\x1b[?{}u", self.current_flags()))
      }
      Some(b'=') => {
        let (flags, mode) = parse_flags_and_mode(&csi_buf[1..]).unwrap_or((0, 1));
        self.active_state_mut().apply_flags(flags, mode);
        KeyboardProtocolAction::Consumed
      }
      Some(b'>') => {
        let flags = parse_optional_number(&csi_buf[1..], 0).unwrap_or(0);
        self.active_state_mut().push(flags);
        KeyboardProtocolAction::Consumed
      }
      Some(b'<') => {
        let count = parse_optional_number(&csi_buf[1..], 1).unwrap_or(1);
        self.active_state_mut().pop(count);
        KeyboardProtocolAction::Consumed
      }
      _ => KeyboardProtocolAction::NotHandled,
    }
  }

  fn observe_private_mode(&mut self, csi_buf: &[u8], enabled: bool) {
    let Some(params) = csi_buf.strip_prefix(b"?") else {
      return;
    };

    for param in params.split(|byte| *byte == b';') {
      if param == b"47" || param == b"1047" || param == b"1049" {
        self.alternate_screen = enabled;
      } else if param == b"9001" {
        self.win32_input_mode = enabled;
      }
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum KeyboardProtocolAction {
  NotHandled,
  Consumed,
  Reply(String),
}

fn parse_optional_number(bytes: &[u8], default: u32) -> Option<u32> {
  if bytes.is_empty() {
    return Some(default);
  }

  std::str::from_utf8(bytes).ok()?.parse().ok()
}

fn parse_flags_and_mode(bytes: &[u8]) -> Option<(u32, u32)> {
  let text = std::str::from_utf8(bytes).ok()?;
  let mut parts = text.splitn(3, ';');
  let flags = match parts.next() {
    Some("") | None => 0,
    Some(value) => value.parse().ok()?,
  };
  let mode = match parts.next() {
    Some("") | None => 1,
    Some(value) => value.parse().ok()?,
  };

  if parts.next().is_some() {
    return None;
  }

  Some((flags, mode))
}

#[cfg(unix)]
mod unix {
  use std::fs::File;
  use std::io::{self, Read, Write};
  use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
  use std::sync::atomic::{AtomicU32, Ordering};

  use std::sync::Arc;
  use std::sync::mpsc;

  use crate::event::{OnResize, WindowSize};
  use crate::tty::{ChildEvent, EventedPty, EventedReadWrite, Pty};
  use polling::{Event, PollMode, Poller};

  use super::super::{RawGraphicsCommand, RawGraphicsEvent};
  use super::KeyboardModeTracker;
  use crate::osc7;
  use crate::program_status::{ProgramStatusParser, ProgramStatusTracker};
  use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};

  /// Callback that tries to get the cursor position from the terminal.
  /// Returns `Some((absolute_line, column))` on success, `None` if lock unavailable.
  pub type CursorFn = Box<dyn Fn() -> Option<(i32, i32)> + Send + Sync>;

  /// Callback for DSR cursor position queries (DECXCPR).
  /// Returns `Some((row_1based, col_1based))` screen-relative, or `None` if lock unavailable.
  pub type DsrCursorFn = Box<dyn Fn() -> Option<(i32, i32)> + Send + Sync>;

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
    /// Inside CSI sequence (\x1b[...), collecting parameter/intermediate bytes.
    CsiCollect,
  }

  /// A `Read` adapter that filters APC graphics sequences from PTY output.
  ///
  /// Reads from a dup'd PTY master fd, strips out `\x1b_G...\x1b\\` sequences,
  /// sends their content to a channel, and returns only non-APC bytes to the caller.
  /// Returns `WouldBlock` when all bytes in a read were APC data.
  pub struct FilteringReader {
    inner: File,
    program_status_parser: ProgramStatusParser,
    state: FilterState,
    apc_buf: Vec<u8>,
    /// Filtered bytes not yet returned to the caller.
    pending: Vec<u8>,
    pending_pos: usize,
    graphics_tx: UnboundedSender<RawGraphicsEvent>,
    /// Once graphics work is queued, route subsequent filter replies through
    /// the same consumer. No thread waits for the UI while holding emulator locks.
    defer_replies: bool,
    osc7_tx: mpsc::Sender<std::path::PathBuf>,
    /// Callback to try-lock the terminal and get cursor position.
    cursor_fn: CursorFn,
    /// Cached cursor position from last successful try-lock.
    last_cursor: (i32, i32),
    /// Shared atomic: terminal sets this to height_cells after place_image().
    /// Filter injects CNL on next read and resets to 0.
    pending_cnl: Arc<AtomicU32>,
    /// Buffer for collecting CSI sequence bytes (after ESC [).
    csi_buf: Vec<u8>,
    /// Callback for DSR cursor queries (returns 1-based screen-relative row, col).
    dsr_cursor_fn: DsrCursorFn,
    /// Cached DSR cursor position from last successful try-lock.
    last_dsr_cursor: (i32, i32),
    keyboard_mode: KeyboardModeTracker,
    keyboard_flags: Arc<AtomicU32>,
  }

  fn route_reply(
    inner: &mut impl Write,
    defer_replies: &mut bool,
    graphics_tx: &UnboundedSender<RawGraphicsEvent>,
    reply: &[u8],
  ) {
    if *defer_replies
      && graphics_tx
        .unbounded_send(RawGraphicsEvent::Reply(reply.to_vec()))
        .is_ok()
    {
      return;
    }
    // No graphics consumer (or none needed yet): retain standalone PTY replies.
    *defer_replies = false;
    let _ = inner.write_all(reply);
  }

  impl FilteringReader {
    fn reply(&mut self, reply: &[u8]) {
      route_reply(
        &mut self.inner,
        &mut self.defer_replies,
        &self.graphics_tx,
        reply,
      );
    }

    /// Try to capture cursor position. Updates cache on success.
    fn capture_cursor(&mut self) -> (i32, i32) {
      if let Some(pos) = (self.cursor_fn)() {
        self.last_cursor = pos;
      }
      self.last_cursor
    }

    /// Try to capture screen-relative cursor position for DSR. Updates cache on success.
    fn capture_dsr_cursor(&mut self) -> (i32, i32) {
      if let Some(pos) = (self.dsr_cursor_fn)() {
        self.last_dsr_cursor = pos;
      }
      self.last_dsr_cursor
    }

    fn sync_keyboard_flags(&self) {
      self
        .keyboard_flags
        .store(self.keyboard_mode.input_flags(), Ordering::Relaxed);
    }

    /// Handle a completed CSI sequence. Intercepts Device Attributes (DA),
    /// XTVERSION, Kitty keyboard protocol negotiation, and private DSR queries.
    /// After graphics work starts, replies join its FIFO instead of overtaking
    /// asynchronous query results. Returns true if the sequence was consumed.
    fn handle_csi_final(&mut self, final_byte: u8) -> bool {
      if matches!(final_byte, b'h' | b'l') {
        let consume_conpty_win32_mode = self.csi_buf == b"?9001";
        self
          .keyboard_mode
          .observe_private_mode(&self.csi_buf, final_byte == b'h');
        self.sync_keyboard_flags();
        return consume_conpty_win32_mode;
      }

      match final_byte {
        b'c' => self.handle_device_attributes(),
        b'q' => self.handle_xtversion(),
        b'u' => self.handle_keyboard_protocol(),
        b'n' => self.handle_private_dsr(),
        b'J' if self.csi_buf == b"2" || self.csi_buf == b"3" => {
          let _ = self.graphics_tx.unbounded_send(RawGraphicsEvent::ClearAll);
          false // The emulator must still erase the text.
        }
        _ => false,
      }
    }

    fn handle_keyboard_protocol(&mut self) -> bool {
      match self.keyboard_mode.handle_csi_u(&self.csi_buf) {
        super::KeyboardProtocolAction::NotHandled => false,
        super::KeyboardProtocolAction::Consumed => {
          self.sync_keyboard_flags();
          true
        }
        super::KeyboardProtocolAction::Reply(resp) => {
          self.sync_keyboard_flags();
          self.reply(resp.as_bytes());
          true
        }
      }
    }

    /// Handle Device Attributes requests (Primary, Secondary, Tertiary DA).
    fn handle_device_attributes(&mut self) -> bool {
      let response: Option<String> = if self.csi_buf.is_empty() || self.csi_buf == b"0" {
        // Primary DA (CSI c / CSI 0 c).
        // Report as VT220 (level 2) with ANSI color/VT525 support.
        Some("\x1b[?62;22c".to_string())
      } else if self.csi_buf.first() == Some(&b'>') {
        let param = &self.csi_buf[1..];
        if param.is_empty() || param == b"0" {
          // Secondary DA (CSI > c / CSI > 0 c).
          // Pp=1 (VT220 family), Pv=version, Pc=0 (ROM cartridge absent).
          Some(format!("\x1b[>1;{};0c", super::da2_version_number()))
        } else {
          None
        }
      } else if self.csi_buf.first() == Some(&b'=') {
        let param = &self.csi_buf[1..];
        if param.is_empty() || param == b"0" {
          // Tertiary DA (CSI = c / CSI = 0 c).
          // Report unit ID as zeros (not applicable).
          Some("\x1bP!|00000000\x1b\\".to_string())
        } else {
          None
        }
      } else {
        None
      };

      if let Some(resp) = response {
        self.reply(resp.as_bytes());
        true
      } else {
        false
      }
    }

    /// Handle XTVERSION query (CSI > q / CSI > 0 q).
    fn handle_xtversion(&mut self) -> bool {
      if self.csi_buf.first() != Some(&b'>') {
        return false;
      }
      let param = &self.csi_buf[1..];
      if !param.is_empty() && param != b"0" {
        return false;
      }
      let resp = format!("\x1bP>|kazeterm({})\x1b\\", env!("CARGO_PKG_VERSION"));
      self.reply(resp.as_bytes());
      true
    }

    /// Handle private DSR queries (CSI ? N n).
    fn handle_private_dsr(&mut self) -> bool {
      if self.csi_buf.first() != Some(&b'?') {
        return false;
      }

      let param_bytes = &self.csi_buf[1..];
      let param_str = match std::str::from_utf8(param_bytes) {
        Ok(s) => s,
        Err(_) => return false,
      };
      let param: u16 = match param_str.parse() {
        Ok(v) => v,
        Err(_) => return false,
      };

      let response: Option<String> = match param {
        // DECXCPR: Extended Cursor Position Report.
        6 => {
          let (row, col) = self.capture_dsr_cursor();
          Some(format!("\x1b[?{};{}R", row, col))
        }
        // Printer status: no printer connected.
        15 => Some("\x1b[?13n".to_string()),
        // UDK (User Defined Keys) status: locked.
        25 => Some("\x1b[?21n".to_string()),
        // Keyboard dialect: North American (US).
        26 => Some("\x1b[?27;1n".to_string()),
        // Locator status: no locator.
        53 | 55 => Some("\x1b[?53n".to_string()),
        // Locator type: mouse.
        56 => Some("\x1b[?57;1n".to_string()),
        // Macro space report: 0 bytes available.
        62 => Some("\x1b[0*{".to_string()),
        // Data integrity report: no malfunction.
        75 => Some("\x1b[?70n".to_string()),
        // Multi-session status: not in multi-session mode.
        85 => Some("\x1b[?83n".to_string()),
        _ => None,
      };

      if let Some(resp) = response {
        self.reply(resp.as_bytes());
        true
      } else {
        false
      }
    }

    /// Flush the buffered CSI sequence to pending (pass-through to alacritty).
    fn flush_csi_to_pending(&mut self, final_byte: Option<u8>) {
      self.pending.push(0x1B);
      self.pending.push(b'[');
      self.pending.extend_from_slice(&self.csi_buf);
      if let Some(fb) = final_byte {
        self.pending.push(fb);
      }
      self.csi_buf.clear();
    }

    fn push_apc_byte(&mut self, byte: u8) {
      if self.apc_buf.len() < super::MAX_GRAPHICS_APC_BYTES {
        self.apc_buf.push(byte);
      }
    }
  }

  impl Read for FilteringReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
      // Check for terminal-computed CNL feedback (from place_image).
      let feedback_rows = self.pending_cnl.swap(0, Ordering::AcqRel);
      if feedback_rows > 0 {
        let cnl = format!("\x1b[{}E", feedback_rows);
        // Prepend to any existing pending data.
        let mut new_pending = Vec::with_capacity(cnl.len() + self.pending.len());
        new_pending.extend_from_slice(cnl.as_bytes());
        if self.pending_pos < self.pending.len() {
          new_pending.extend_from_slice(&self.pending[self.pending_pos..]);
        }
        self.pending = new_pending;
        self.pending_pos = 0;
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
        // Observe in stream order, not in a pre-pass that could answer an OSC
        // query later in this read before an earlier graphics command.
        self
          .program_status_parser
          .advance(std::slice::from_ref(&byte), |reply| {
            route_reply(
              &mut self.inner,
              &mut self.defer_replies,
              &self.graphics_tx,
              reply,
            );
          });
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
            } else if byte == b'[' {
              self.state = FilterState::CsiCollect;
              self.csi_buf.clear();
            } else {
              if byte == b'c' {
                let _ = self.graphics_tx.unbounded_send(RawGraphicsEvent::ClearAll);
              }
              // Not APC or CSI — pass through the ESC and this byte.
              self.pending.push(0x1B);
              self.pending.push(byte);
              self.state = FilterState::Normal;
            }
          }
          FilterState::CsiCollect => {
            match byte {
              // Parameter bytes (digits, semicolons, and private-mode markers like '?').
              0x30..=0x3F => {
                self.csi_buf.push(byte);
                // Cap at 256 bytes to prevent unbounded buffering.
                if self.csi_buf.len() > 256 {
                  self.flush_csi_to_pending(None);
                  self.state = FilterState::Normal;
                }
              }
              // Intermediate bytes (space through '/').
              0x20..=0x2F => {
                self.csi_buf.push(byte);
              }
              // Final byte — CSI sequence is complete.
              0x40..=0x7E => {
                if !self.handle_csi_final(byte) {
                  // Not a DSR we handle — flush to pending for alacritty.
                  self.flush_csi_to_pending(Some(byte));
                } else {
                  self.csi_buf.clear();
                }
                self.state = FilterState::Normal;
              }
              // Control character or other invalid byte inside CSI.
              _ => {
                self.flush_csi_to_pending(None);
                self.pending.push(byte);
                self.state = FilterState::Normal;
              }
            }
          }
          FilterState::ApcCollect => {
            if byte == 0x1B {
              self.state = FilterState::ApcEscape;
            } else {
              self.push_apc_byte(byte);
            }
          }
          FilterState::ApcEscape => {
            if byte == b'\\' {
              // Complete APC sequence. Check for Kitty graphics prefix 'G'.
              if self.apc_buf.first() == Some(&b'G') {
                let (cursor_line, cursor_column) = self.capture_cursor();
                let cmd_data = self.apc_buf[1..].to_vec();
                // Never move the cursor based on unvalidated metadata or a
                // partial upload. Only successful placements request feedback.

                if self
                  .graphics_tx
                  .unbounded_send(RawGraphicsEvent::Command(RawGraphicsCommand {
                    data: cmd_data,
                    cursor_line,
                    cursor_column,
                  }))
                  .is_ok()
                {
                  self.defer_replies = true;
                }
              }
              self.apc_buf.clear();
              self.state = FilterState::Normal;
            } else {
              // ESC inside APC not followed by '\' — keep collecting.
              self.push_apc_byte(0x1B);
              self.push_apc_byte(byte);
              self.state = FilterState::ApcCollect;
            }
          }
        }
      }

      if self.pending.is_empty() {
        // All bytes were APC data — signal "no data yet" to the EventLoop.
        return Err(io::Error::from(io::ErrorKind::WouldBlock));
      }

      // Scan passthrough bytes for OSC 7 CWD sequences.
      if let Some(cwd) = osc7::extract_osc7_path(&self.pending) {
        let _ = self.osc7_tx.send(cwd);
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
    /// `dsr_cursor_fn` is called to capture the screen-relative cursor position
    /// (1-based row, col) for DECXCPR responses.
    ///
    /// Returns `(filter, pending_cnl, keyboard_flags, graphics_rx, osc7_rx)`:
    /// - `pending_cnl`: shared atomic for terminal to request cursor advancement
    /// - `keyboard_flags`: shared atomic exposing active kitty keyboard flags
    /// - `graphics_rx`: receives ordered graphics commands, clears, and deferred replies
    /// - `osc7_rx`: receives CWD paths extracted from OSC 7 sequences
    pub fn new(
      pty: Pty,
      cursor_fn: CursorFn,
      dsr_cursor_fn: DsrCursorFn,
      program_status_tracker: ProgramStatusTracker,
    ) -> io::Result<(
      Self,
      Arc<AtomicU32>,
      Arc<AtomicU32>,
      UnboundedReceiver<RawGraphicsEvent>,
      mpsc::Receiver<std::path::PathBuf>,
    )> {
      // Dup the master fd so the FilteringReader has its own fd for reading.
      // The original fd stays in the Pty for poll registration and writing.
      let master_fd = pty.file().as_raw_fd();
      let read_fd = unsafe { libc::dup(master_fd) };
      if read_fd < 0 {
        return Err(io::Error::last_os_error());
      }
      let read_file = unsafe { File::from_raw_fd(read_fd) };

      let (graphics_tx, graphics_rx) = unbounded();
      let (osc7_tx, osc7_rx) = mpsc::channel();
      let pending_cnl = Arc::new(AtomicU32::new(0));
      let keyboard_flags = Arc::new(AtomicU32::new(0));

      let reader = FilteringReader {
        inner: read_file,
        program_status_parser: ProgramStatusParser::new(program_status_tracker),
        state: FilterState::Normal,
        apc_buf: Vec::with_capacity(4096),
        pending: Vec::with_capacity(8192),
        pending_pos: 0,
        graphics_tx,
        defer_replies: false,
        osc7_tx,
        cursor_fn,
        last_cursor: (0, 0),
        pending_cnl: Arc::clone(&pending_cnl),
        csi_buf: Vec::with_capacity(64),
        dsr_cursor_fn,
        last_dsr_cursor: (1, 1),
        keyboard_mode: KeyboardModeTracker::default(),
        keyboard_flags: Arc::clone(&keyboard_flags),
      };

      Ok((
        GraphicsPtyFilter { reader, pty },
        pending_cnl,
        keyboard_flags,
        graphics_rx,
        osc7_rx,
      ))
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

  #[cfg(test)]
  mod tests {
    use super::*;
    use std::os::{fd::OwnedFd, unix::net::UnixStream};

    fn reader() -> (
      FilteringReader,
      UnixStream,
      UnboundedReceiver<RawGraphicsEvent>,
    ) {
      let (input, output) = UnixStream::pair().unwrap();
      input.set_nonblocking(true).unwrap();
      let (graphics_tx, graphics_rx) = unbounded();
      let (osc7_tx, _) = mpsc::channel();
      let reader = FilteringReader {
        inner: File::from(OwnedFd::from(input)),
        program_status_parser: ProgramStatusParser::new(ProgramStatusTracker::default()),
        state: FilterState::Normal,
        apc_buf: Vec::new(),
        pending: Vec::new(),
        pending_pos: 0,
        graphics_tx,
        defer_replies: false,
        osc7_tx,
        cursor_fn: Box::new(|| Some((5, 6))),
        last_cursor: (0, 0),
        pending_cnl: Arc::new(AtomicU32::new(0)),
        csi_buf: Vec::new(),
        dsr_cursor_fn: Box::new(|| Some((1, 1))),
        last_dsr_cursor: (1, 1),
        keyboard_mode: KeyboardModeTracker::default(),
        keyboard_flags: Arc::new(AtomicU32::new(0)),
      };
      (reader, output, graphics_rx)
    }

    #[test]
    fn graphics_capture_does_not_advance_unvalidated_or_partial_images() {
      let (mut reader, mut input, mut graphics) = reader();
      let mut buffer = [0; 256];
      for command in [
        b"\x1b_Ga=T,s=1,v=1,r=2,m=1;AAAA\x1b\\".as_slice(),
        b"\x1b_Gm=0;AAAA\x1b\\",
        b"\x1b_Ga=T,s=1,v=1,r=2,C=1;AAAA\x1b\\",
      ] {
        input.write_all(command).unwrap();
        assert_eq!(
          reader.read(&mut buffer).unwrap_err().kind(),
          io::ErrorKind::WouldBlock
        );
        let RawGraphicsEvent::Command(command) = graphics.try_recv().unwrap() else {
          panic!("expected graphics command")
        };
        assert_eq!((command.cursor_line, command.cursor_column), (5, 6));
        assert!(reader.pending.is_empty());
      }
      reader.pending_cnl.store(3, Ordering::Release);
      let count = reader.read(&mut buffer).unwrap();
      assert_eq!(&buffer[..count], b"\x1b[3E"); // no invented padding
      assert_eq!(
        reader.read(&mut buffer).unwrap_err().kind(),
        io::ErrorKind::WouldBlock
      );
    }

    #[test]
    fn graphics_probe_does_not_send_da_before_graphics_processing() {
      let (mut reader, mut input, mut graphics) = reader();
      input.set_nonblocking(true).unwrap();
      input
        .write_all(b"\x1b_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\\x1b[c")
        .unwrap();
      let mut buffer = [0; 256];
      assert_eq!(
        reader.read(&mut buffer).unwrap_err().kind(),
        io::ErrorKind::WouldBlock
      );
      assert_eq!(
        input.read(&mut buffer).unwrap_err().kind(),
        io::ErrorKind::WouldBlock
      );
      assert!(
        matches!(graphics.try_recv().unwrap(), RawGraphicsEvent::Command(command) if command.data.starts_with(b"i=31"))
      );
      assert!(
        matches!(graphics.try_recv().unwrap(), RawGraphicsEvent::Reply(reply) if reply == b"\x1b[?62;22c")
      );
    }

    #[test]
    fn replies_and_clears_keep_stream_order_across_reads() {
      let (mut reader, mut input, mut graphics) = reader();
      input.set_nonblocking(true).unwrap();
      let mut buffer = [0; 256];
      // Before any graphics command, standalone DA replies remain synchronous.
      input.write_all(b"\x1b[c").unwrap();
      assert!(reader.read(&mut buffer).is_err());
      let count = input.read(&mut buffer).unwrap();
      assert_eq!(&buffer[..count], b"\x1b[?62;22c");
      input
        .write_all(b"\x1b_Ga=q,i=31,f=24,s=1,v=1;AAAA\x1b\\\x1b[2")
        .unwrap();
      assert!(reader.read(&mut buffer).is_err());
      input
        .write_all(b"J\x1b_Ga=T,i=2;AAAA\x1b\\\x1b[?u\x1b]7501;?\x07\x1bc\x1b[c")
        .unwrap();
      let count = reader.read(&mut buffer).unwrap();
      assert!(buffer[..count].starts_with(b"\x1b[2J"));
      assert!(matches!(
        graphics.try_recv().unwrap(),
        RawGraphicsEvent::Command(_)
      ));
      assert!(matches!(
        graphics.try_recv().unwrap(),
        RawGraphicsEvent::ClearAll
      ));
      assert!(matches!(
        graphics.try_recv().unwrap(),
        RawGraphicsEvent::Command(_)
      ));
      assert!(
        matches!(graphics.try_recv().unwrap(), RawGraphicsEvent::Reply(reply) if reply == b"\x1b[?0u")
      );
      assert!(
        matches!(graphics.try_recv().unwrap(), RawGraphicsEvent::Reply(reply) if reply.starts_with(b"\x1b]7501;?:states="))
      );
      assert!(matches!(
        graphics.try_recv().unwrap(),
        RawGraphicsEvent::ClearAll
      ));
      assert!(
        matches!(graphics.try_recv().unwrap(), RawGraphicsEvent::Reply(reply) if reply == b"\x1b[?62;22c")
      );
      assert_eq!(
        input.read(&mut buffer).unwrap_err().kind(),
        io::ErrorKind::WouldBlock
      );
    }

    #[test]
    fn closed_graphics_consumer_falls_back_without_waiting() {
      let (mut reader, mut input, graphics) = reader();
      input.set_nonblocking(true).unwrap();
      let mut buffer = [0; 256];
      input
        .write_all(b"\x1b_Ga=q,i=31,f=24,s=1,v=1;AAAA\x1b\\")
        .unwrap();
      assert!(reader.read(&mut buffer).is_err());
      assert!(reader.defer_replies);
      drop(graphics);
      input.write_all(b"\x1b[c").unwrap();
      assert!(reader.read(&mut buffer).is_err());
      let count = input.read(&mut buffer).unwrap();
      assert_eq!(&buffer[..count], b"\x1b[?62;22c");
      assert!(!reader.defer_replies);
    }

    #[test]
    fn oversized_apc_is_bounded_and_consumed_until_split_terminator() {
      let (mut reader, mut input, mut graphics) = reader();
      let mut buffer = [0; 256];
      input.write_all(b"\x1b_Ga=T,i=1;").unwrap();
      assert!(reader.read(&mut buffer).is_err());
      for _ in 0..8 {
        input.write_all(&[b'A'; 2048]).unwrap();
        assert!(reader.read(&mut buffer).is_err());
        assert!(reader.apc_buf.len() <= super::super::MAX_GRAPHICS_APC_BYTES);
      }
      input.write_all(b"\x1b").unwrap();
      assert!(reader.read(&mut buffer).is_err());
      input.write_all(b"\\text").unwrap();
      let count = reader.read(&mut buffer).unwrap();
      assert_eq!(&buffer[..count], b"text");
      let RawGraphicsEvent::Command(command) = graphics.try_recv().unwrap() else {
        panic!("expected graphics command")
      };
      assert_eq!(command.data.len(), super::super::MAX_GRAPHICS_APC_BYTES - 1);
      assert_eq!(reader.state, FilterState::Normal);
    }
  }
}

#[cfg(unix)]
pub use unix::GraphicsPtyFilter;

#[cfg(not(unix))]
mod windows {
  use std::io::{self, Read, Write};
  use std::sync::Arc;
  use std::sync::atomic::{AtomicU32, Ordering};

  use super::KeyboardModeTracker;
  use crate::event::{OnResize, WindowSize};
  use crate::program_status::{ProgramStatusParser, ProgramStatusTracker};
  use crate::tty::{ChildEvent, EventedPty, EventedReadWrite, Pty};
  use polling::{Event, PollMode, Poller};

  /// Callback for DSR cursor position queries (DECXCPR).
  /// Returns `Some((row_1based, col_1based))` screen-relative, or `None` if lock unavailable.
  pub type DsrCursorFn = Box<dyn Fn() -> Option<(i32, i32)> + Send + Sync>;

  /// Minimal CSI filter state for DSR detection on Windows.
  #[derive(Debug, Clone, Copy, PartialEq)]
  enum FilterState {
    Normal,
    Escape,
    CsiCollect,
  }

  /// PTY wrapper that intercepts private DSR queries and Kitty keyboard
  /// protocol negotiation on Windows.
  ///
  /// ConPTY may pass through CSI sequences it doesn't handle. This wrapper
  /// scans the conout byte stream for `CSI ? N n` (private DSR) and writes
  /// responses directly to the conin pipe (PTY input).
  ///
  /// Uses `type Reader = Self` / `type Writer = Self` so that `Read::read()`
  /// has access to both the inner reader and writer of the wrapped Pty.
  pub struct WindowsDsrFilter {
    pty: Pty,
    program_status_parser: ProgramStatusParser,
    state: FilterState,
    csi_buf: Vec<u8>,
    pending: Vec<u8>,
    pending_pos: usize,
    dsr_cursor_fn: DsrCursorFn,
    last_dsr_cursor: (i32, i32),
    keyboard_mode: KeyboardModeTracker,
    keyboard_flags: Arc<AtomicU32>,
  }

  impl WindowsDsrFilter {
    pub fn new(
      pty: Pty,
      dsr_cursor_fn: DsrCursorFn,
      program_status_tracker: ProgramStatusTracker,
    ) -> (Self, Arc<AtomicU32>) {
      let keyboard_flags = Arc::new(AtomicU32::new(0));

      (
        Self {
          pty,
          program_status_parser: ProgramStatusParser::new(program_status_tracker),
          state: FilterState::Normal,
          csi_buf: Vec::with_capacity(64),
          pending: Vec::with_capacity(8192),
          pending_pos: 0,
          dsr_cursor_fn,
          last_dsr_cursor: (1, 1),
          keyboard_mode: KeyboardModeTracker::default(),
          keyboard_flags: Arc::clone(&keyboard_flags),
        },
        keyboard_flags,
      )
    }

    /// Try to capture screen-relative cursor position for DSR.
    fn capture_dsr_cursor(&mut self) -> (i32, i32) {
      if let Some(pos) = (self.dsr_cursor_fn)() {
        self.last_dsr_cursor = pos;
      }
      self.last_dsr_cursor
    }

    fn sync_keyboard_flags(&self) {
      self
        .keyboard_flags
        .store(self.keyboard_mode.input_flags(), Ordering::Relaxed);
    }

    /// Handle a completed CSI sequence. Intercepts Device Attributes (DA),
    /// XTVERSION, Kitty keyboard protocol negotiation, and private DSR
    /// queries, writing responses directly to the PTY input. Returns true if
    /// the sequence was consumed.
    fn handle_csi_final(&mut self, final_byte: u8) -> bool {
      if matches!(final_byte, b'h' | b'l') {
        let consume_conpty_win32_mode = self.csi_buf == b"?9001";
        self
          .keyboard_mode
          .observe_private_mode(&self.csi_buf, final_byte == b'h');
        self.sync_keyboard_flags();
        return consume_conpty_win32_mode;
      }

      match final_byte {
        b'c' => self.handle_device_attributes(),
        b'q' => self.handle_xtversion(),
        b'u' => self.handle_keyboard_protocol(),
        b'n' => self.handle_private_dsr(),
        _ => false,
      }
    }

    fn handle_keyboard_protocol(&mut self) -> bool {
      match self.keyboard_mode.handle_csi_u(&self.csi_buf) {
        super::KeyboardProtocolAction::NotHandled => false,
        super::KeyboardProtocolAction::Consumed => {
          self.sync_keyboard_flags();
          true
        }
        super::KeyboardProtocolAction::Reply(resp) => {
          self.sync_keyboard_flags();
          let _ = self.pty.writer().write_all(resp.as_bytes());
          true
        }
      }
    }

    /// Handle Device Attributes requests (Primary, Secondary, Tertiary DA).
    fn handle_device_attributes(&mut self) -> bool {
      let response: Option<String> = if self.csi_buf.is_empty() || self.csi_buf == b"0" {
        // Primary DA (CSI c / CSI 0 c).
        // Report as VT220 (level 2) with ANSI color/VT525 support.
        Some("\x1b[?62;22c".to_string())
      } else if self.csi_buf.first() == Some(&b'>') {
        let param = &self.csi_buf[1..];
        if param.is_empty() || param == b"0" {
          // Secondary DA (CSI > c / CSI > 0 c).
          // Pp=1 (VT220 family), Pv=version, Pc=0 (ROM cartridge absent).
          Some(format!("\x1b[>1;{};0c", super::da2_version_number()))
        } else {
          None
        }
      } else if self.csi_buf.first() == Some(&b'=') {
        let param = &self.csi_buf[1..];
        if param.is_empty() || param == b"0" {
          // Tertiary DA (CSI = c / CSI = 0 c).
          // Report unit ID as zeros (not applicable).
          Some("\x1bP!|00000000\x1b\\".to_string())
        } else {
          None
        }
      } else {
        None
      };

      if let Some(resp) = response {
        let _ = self.pty.writer().write_all(resp.as_bytes());
        true
      } else {
        false
      }
    }

    /// Handle XTVERSION query (CSI > q / CSI > 0 q).
    fn handle_xtversion(&mut self) -> bool {
      if self.csi_buf.first() != Some(&b'>') {
        return false;
      }
      let param = &self.csi_buf[1..];
      if !param.is_empty() && param != b"0" {
        return false;
      }
      let resp = format!("\x1bP>|kazeterm({})\x1b\\", env!("CARGO_PKG_VERSION"));
      let _ = self.pty.writer().write_all(resp.as_bytes());
      true
    }

    /// Handle private DSR queries (CSI ? N n).
    fn handle_private_dsr(&mut self) -> bool {
      if self.csi_buf.first() != Some(&b'?') {
        return false;
      }

      let param_bytes = &self.csi_buf[1..];
      let param_str = match std::str::from_utf8(param_bytes) {
        Ok(s) => s,
        Err(_) => return false,
      };
      let param: u16 = match param_str.parse() {
        Ok(v) => v,
        Err(_) => return false,
      };

      let response: Option<String> = match param {
        // DECXCPR: Extended Cursor Position Report.
        6 => {
          let (row, col) = self.capture_dsr_cursor();
          Some(format!("\x1b[?{};{}R", row, col))
        }
        // Printer status: no printer connected.
        15 => Some("\x1b[?13n".to_string()),
        // UDK (User Defined Keys) status: locked.
        25 => Some("\x1b[?21n".to_string()),
        // Keyboard dialect: North American (US).
        26 => Some("\x1b[?27;1n".to_string()),
        // Locator status: no locator.
        53 | 55 => Some("\x1b[?53n".to_string()),
        // Locator type: mouse.
        56 => Some("\x1b[?57;1n".to_string()),
        // Macro space report: 0 bytes available.
        62 => Some("\x1b[0*{".to_string()),
        // Data integrity report: no malfunction.
        75 => Some("\x1b[?70n".to_string()),
        // Multi-session status: not in multi-session mode.
        85 => Some("\x1b[?83n".to_string()),
        _ => None,
      };

      if let Some(resp) = response {
        let _ = self.pty.writer().write_all(resp.as_bytes());
        true
      } else {
        false
      }
    }

    /// Flush the buffered CSI sequence to pending (pass-through to alacritty).
    fn flush_csi_to_pending(&mut self, final_byte: Option<u8>) {
      self.pending.push(0x1B);
      self.pending.push(b'[');
      self.pending.extend_from_slice(&self.csi_buf);
      if let Some(fb) = final_byte {
        self.pending.push(fb);
      }
      self.csi_buf.clear();
    }
  }

  impl Read for WindowsDsrFilter {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
      // Drain pending filtered bytes from a previous read.
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

      // Read raw bytes from the inner PTY reader.
      let mut raw = [0u8; 8192];
      let n = self.pty.reader().read(&mut raw)?;
      if n == 0 {
        return Ok(0);
      }

      self.program_status_parser.advance(&raw[..n], |reply| {
        let _ = self.pty.writer().write_all(reply);
      });

      self.pending.clear();
      self.pending_pos = 0;

      // Run the CSI/DSR state machine over the raw bytes.
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
            if byte == b'[' {
              self.state = FilterState::CsiCollect;
              self.csi_buf.clear();
            } else {
              // Not CSI — pass through the ESC and this byte.
              self.pending.push(0x1B);
              self.pending.push(byte);
              self.state = FilterState::Normal;
            }
          }
          FilterState::CsiCollect => match byte {
            // Parameter bytes (digits, semicolons, private-mode markers like '?').
            0x30..=0x3F => {
              self.csi_buf.push(byte);
              if self.csi_buf.len() > 256 {
                self.flush_csi_to_pending(None);
                self.state = FilterState::Normal;
              }
            }
            // Intermediate bytes (space through '/').
            0x20..=0x2F => {
              self.csi_buf.push(byte);
            }
            // Final byte — CSI sequence is complete.
            0x40..=0x7E => {
              if !self.handle_csi_final(byte) {
                self.flush_csi_to_pending(Some(byte));
              } else {
                self.csi_buf.clear();
              }
              self.state = FilterState::Normal;
            }
            // Invalid byte inside CSI.
            _ => {
              self.flush_csi_to_pending(None);
              self.pending.push(byte);
              self.state = FilterState::Normal;
            }
          },
        }
      }

      if self.pending.is_empty() {
        return Ok(0);
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

  impl Write for WindowsDsrFilter {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
      self.pty.writer().write(buf)
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
      self.pty.writer().flush()
    }
  }

  impl EventedReadWrite for WindowsDsrFilter {
    type Reader = Self;
    type Writer = Self;

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
    fn reader(&mut self) -> &mut Self::Reader {
      self
    }

    #[inline]
    fn writer(&mut self) -> &mut Self::Writer {
      self
    }
  }

  impl EventedPty for WindowsDsrFilter {
    #[inline]
    fn next_child_event(&mut self) -> Option<ChildEvent> {
      self.pty.next_child_event()
    }
  }

  impl OnResize for WindowsDsrFilter {
    #[inline]
    fn on_resize(&mut self, window_size: WindowSize) {
      self.pty.on_resize(window_size);
    }
  }
}

#[cfg(not(unix))]
pub use windows::DsrCursorFn as WindowsDsrCursorFn;
#[cfg(not(unix))]
pub use windows::WindowsDsrFilter;

#[cfg(test)]
mod tests {
  use super::{
    KITTY_KEYBOARD_DISAMBIGUATE_ESCAPE_CODES, KITTY_KEYBOARD_REPORT_ALL_KEYS_AS_ESCAPE_CODES,
    KeyboardModeTracker, KeyboardProtocolAction, WINDOWS_CONPTY_WIN32_INPUT_MODE,
  };

  #[test]
  fn kitty_keyboard_push_query_and_pop_track_flags() {
    let mut tracker = KeyboardModeTracker::default();

    assert_eq!(
      tracker.handle_csi_u(b"?"),
      KeyboardProtocolAction::Reply("\x1b[?0u".to_string())
    );

    assert_eq!(
      tracker.handle_csi_u(b">1"),
      KeyboardProtocolAction::Consumed
    );
    assert_eq!(
      tracker.current_flags(),
      KITTY_KEYBOARD_DISAMBIGUATE_ESCAPE_CODES
    );

    assert_eq!(
      tracker.handle_csi_u(b"=8;2"),
      KeyboardProtocolAction::Consumed
    );
    assert_eq!(
      tracker.current_flags(),
      KITTY_KEYBOARD_DISAMBIGUATE_ESCAPE_CODES | KITTY_KEYBOARD_REPORT_ALL_KEYS_AS_ESCAPE_CODES
    );

    assert_eq!(tracker.handle_csi_u(b"<"), KeyboardProtocolAction::Consumed);
    assert_eq!(tracker.current_flags(), 0);
  }

  #[test]
  fn kitty_keyboard_state_is_separate_per_screen() {
    let mut tracker = KeyboardModeTracker::default();

    assert_eq!(
      tracker.handle_csi_u(b">1"),
      KeyboardProtocolAction::Consumed
    );
    tracker.observe_private_mode(b"?1049", true);
    assert_eq!(tracker.current_flags(), 0);

    assert_eq!(
      tracker.handle_csi_u(b">8"),
      KeyboardProtocolAction::Consumed
    );
    assert_eq!(
      tracker.current_flags(),
      KITTY_KEYBOARD_REPORT_ALL_KEYS_AS_ESCAPE_CODES
    );

    tracker.observe_private_mode(b"?1049", false);
    assert_eq!(
      tracker.current_flags(),
      KITTY_KEYBOARD_DISAMBIGUATE_ESCAPE_CODES
    );
  }

  #[test]
  fn conpty_win32_input_mode_sets_and_clears_flag() {
    let mut tracker = KeyboardModeTracker::default();

    tracker.observe_private_mode(b"?9001", true);
    assert_eq!(tracker.input_flags(), WINDOWS_CONPTY_WIN32_INPUT_MODE);

    tracker.observe_private_mode(b"?9001", false);
    assert_eq!(tracker.input_flags(), 0);
  }
}
