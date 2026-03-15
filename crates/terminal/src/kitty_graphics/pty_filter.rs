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
  use std::sync::mpsc;
  use std::sync::Arc;

  use alacritty_terminal::event::{OnResize, WindowSize};
  use alacritty_terminal::tty::{ChildEvent, EventedPty, EventedReadWrite, Pty};
  use polling::{Event, PollMode, Poller};

  /// Raw bytes of a complete APC graphics sequence (content between \x1b_G and \x1b\\).
  pub type RawGraphicsCommand = Vec<u8>;

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
  /// sends their content to a channel, and returns only normal bytes to the caller.
  /// Returns `WouldBlock` when all bytes in a read were APC data.
  pub struct FilteringReader {
    inner: File,
    state: FilterState,
    apc_buf: Vec<u8>,
    /// Filtered bytes not yet returned to the caller.
    pending: Vec<u8>,
    pending_pos: usize,
    graphics_tx: mpsc::Sender<RawGraphicsCommand>,
  }

  impl Read for FilteringReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
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
                let cmd_data = self.apc_buf[1..].to_vec();
                let _ = self.graphics_tx.send(cmd_data);
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
    /// Returns `(filter, graphics_rx)` where `graphics_rx` receives raw
    /// Kitty graphics command bytes extracted from the PTY stream.
    pub fn new(pty: Pty) -> io::Result<(Self, mpsc::Receiver<RawGraphicsCommand>)> {
      // Dup the master fd so the FilteringReader has its own fd for reading.
      // The original fd stays in the Pty for poll registration and writing.
      let master_fd = pty.file().as_raw_fd();
      let read_fd = unsafe { libc::dup(master_fd) };
      if read_fd < 0 {
        return Err(io::Error::last_os_error());
      }
      let read_file = unsafe { File::from_raw_fd(read_fd) };

      let (graphics_tx, graphics_rx) = mpsc::channel();

      let reader = FilteringReader {
        inner: read_file,
        state: FilterState::Normal,
        apc_buf: Vec::with_capacity(4096),
        pending: Vec::with_capacity(8192),
        pending_pos: 0,
        graphics_tx,
      };

      Ok((GraphicsPtyFilter { reader, pty }, graphics_rx))
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
