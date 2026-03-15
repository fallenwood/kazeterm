//! PTY byte-stream filter for intercepting Kitty graphics protocol APC sequences.
//!
//! The VTE parser used by alacritty_terminal silently discards APC sequences.
//! This module provides a transparent PTY wrapper that intercepts `\x1b_G...\x1b\\`
//! sequences before they reach the VTE parser, extracts graphics commands, and
//! passes remaining bytes through to alacritty.

#[cfg(unix)]
mod unix {
  use std::fs::File;
  use std::io::{self, Read, Write};
  use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
  use std::sync::mpsc;
  use std::sync::Arc;
  use std::thread::JoinHandle;

  use alacritty_terminal::event::{OnResize, WindowSize};
  use alacritty_terminal::tty::{ChildEvent, EventedPty, EventedReadWrite};
  use polling::{Event, PollMode, Poller};

  /// Token values matching alacritty_terminal's event_loop.rs constants.
  const PTY_CHILD_EVENT_TOKEN: usize = 1;

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

  /// Raw bytes of a complete APC graphics sequence (content between \x1b_G and \x1b\\).
  pub type RawGraphicsCommand = Vec<u8>;

  /// Transparent PTY wrapper that filters Kitty graphics APC sequences.
  ///
  /// Implements `EventedReadWrite` and `EventedPty` so it can be used as a
  /// drop-in replacement for alacritty's Pty in the EventLoop.
  pub struct GraphicsPtyFilter {
    read_file: File,
    write_file: File,
    child_event_file: File,
    child_pid: u32,
    _filter_handle: JoinHandle<()>,
  }

  impl GraphicsPtyFilter {
    /// Create a graphics-filtering PTY wrapper from an alacritty Pty.
    ///
    /// This extracts the master fd, sets up filter plumbing, and spawns
    /// the filter thread. The original Pty is consumed (dropped after
    /// fd extraction — duped fds keep the PTY alive).
    ///
    /// Returns `(filter, graphics_rx)`.
    pub fn new(
      pty: &alacritty_terminal::tty::Pty,
    ) -> io::Result<(Self, mpsc::Receiver<RawGraphicsCommand>)> {
      let master_fd = pty.file().as_raw_fd();
      let child_pid = pty.child().id();

      // Dup the master fd: one for filter thread reading, one for EventLoop writing.
      let read_dup = unsafe { libc::dup(master_fd) };
      if read_dup < 0 {
        return Err(io::Error::last_os_error());
      }
      let write_dup = unsafe { libc::dup(master_fd) };
      if write_dup < 0 {
        unsafe { libc::close(read_dup) };
        return Err(io::Error::last_os_error());
      }

      // Create pipe for filtered PTY output.
      let mut pipe_fds = [0i32; 2];
      if unsafe { libc::pipe2(pipe_fds.as_mut_ptr(), libc::O_CLOEXEC) } < 0 {
        unsafe {
          libc::close(read_dup);
          libc::close(write_dup);
        };
        return Err(io::Error::last_os_error());
      }
      let pipe_read_fd = pipe_fds[0];
      let pipe_write_fd = pipe_fds[1];

      // Create pipe for child exit notification.
      let mut child_pipe = [0i32; 2];
      if unsafe { libc::pipe2(child_pipe.as_mut_ptr(), libc::O_CLOEXEC | libc::O_NONBLOCK) } < 0 {
        unsafe {
          libc::close(read_dup);
          libc::close(write_dup);
          libc::close(pipe_read_fd);
          libc::close(pipe_write_fd);
        };
        return Err(io::Error::last_os_error());
      }
      let child_event_read_fd = child_pipe[0];
      let child_event_write_fd = child_pipe[1];

      // Channel for extracted graphics commands.
      let (graphics_tx, graphics_rx) = mpsc::channel();
      let filter_graphics_tx = graphics_tx.clone();

      // Spawn filter thread.
      let filter_handle = std::thread::Builder::new()
        .name("kitty-graphics-filter".into())
        .spawn(move || {
          filter_thread_main(
            read_dup,
            pipe_write_fd,
            child_event_write_fd,
            filter_graphics_tx,
          );
        })
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

      // Set the pipe read end to non-blocking (required by alacritty's EventLoop polling).
      unsafe {
        let flags = libc::fcntl(pipe_read_fd, libc::F_GETFL);
        libc::fcntl(pipe_read_fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
      }

      let read_file = unsafe { File::from_raw_fd(pipe_read_fd) };
      let write_file = unsafe { File::from_raw_fd(write_dup) };
      let child_event_file = unsafe { File::from_raw_fd(child_event_read_fd) };

      Ok((
        GraphicsPtyFilter {
          read_file,
          write_file,
          child_event_file,
          child_pid,
          _filter_handle: filter_handle,
        },
        graphics_rx,
      ))
    }

    /// Get the raw fd usable for tcgetpgrp (for PtyProcessInfo).
    pub fn pty_fd(&self) -> RawFd {
      self.write_file.as_raw_fd()
    }

    /// Get the child process PID.
    pub fn child_pid(&self) -> u32 {
      self.child_pid
    }
  }

  impl EventedReadWrite for GraphicsPtyFilter {
    type Reader = File;
    type Writer = File;

    unsafe fn register(
      &mut self,
      poll: &Arc<Poller>,
      interest: Event,
      poll_mode: PollMode,
    ) -> io::Result<()> {
      // Register the filtered output pipe for read/write events.
      unsafe {
        poll.add_with_mode(&self.read_file, interest, poll_mode)?;
      }
      // Register the child event pipe for child exit notification.
      unsafe {
        poll.add_with_mode(
          &self.child_event_file,
          Event::readable(PTY_CHILD_EVENT_TOKEN),
          PollMode::Level,
        )
      }
    }

    fn reregister(
      &mut self,
      poll: &Arc<Poller>,
      interest: Event,
      poll_mode: PollMode,
    ) -> io::Result<()> {
      poll.modify_with_mode(&self.read_file, interest, poll_mode)?;
      poll.modify_with_mode(
        &self.child_event_file,
        Event::readable(PTY_CHILD_EVENT_TOKEN),
        PollMode::Level,
      )
    }

    fn deregister(&mut self, poll: &Arc<Poller>) -> io::Result<()> {
      poll.delete(&self.read_file)?;
      poll.delete(&self.child_event_file)
    }

    fn reader(&mut self) -> &mut File {
      &mut self.read_file
    }

    fn writer(&mut self) -> &mut File {
      &mut self.write_file
    }
  }

  impl EventedPty for GraphicsPtyFilter {
    fn next_child_event(&mut self) -> Option<ChildEvent> {
      // Check if the child event pipe has been signaled.
      let mut buf = [0u8; 1];
      match self.child_event_file.read(&mut buf) {
        Ok(0) | Err(_) => return None,
        Ok(_) => {}
      }

      // Check child status via waitpid.
      let mut status: libc::c_int = 0;
      let result = unsafe { libc::waitpid(self.child_pid as i32, &mut status, libc::WNOHANG) };

      if result <= 0 {
        return None;
      }

      if libc::WIFEXITED(status) {
        Some(ChildEvent::Exited(Some(libc::WEXITSTATUS(status))))
      } else if libc::WIFSIGNALED(status) {
        Some(ChildEvent::Exited(None))
      } else {
        None
      }
    }
  }

  impl OnResize for GraphicsPtyFilter {
    fn on_resize(&mut self, window_size: WindowSize) {
      let win = libc::winsize {
        ws_row: window_size.num_lines,
        ws_col: window_size.num_cols,
        ws_xpixel: window_size.cell_width * window_size.num_cols,
        ws_ypixel: window_size.cell_height * window_size.num_lines,
      };
      unsafe {
        libc::ioctl(
          self.write_file.as_raw_fd(),
          libc::TIOCSWINSZ,
          &win as *const _,
        );
      }
    }
  }

  /// The filter thread's main function.
  ///
  /// Reads raw bytes from the PTY master, scans for APC graphics sequences,
  /// sends them to the graphics channel, and writes remaining bytes to the pipe.
  fn filter_thread_main(
    pty_read_fd: RawFd,
    pipe_write_fd: RawFd,
    child_event_write_fd: RawFd,
    graphics_tx: mpsc::Sender<RawGraphicsCommand>,
  ) {
    let mut pty_read = unsafe { File::from_raw_fd(pty_read_fd) };
    let mut pipe_write = unsafe { File::from_raw_fd(pipe_write_fd) };
    let child_event_write = unsafe { File::from_raw_fd(child_event_write_fd) };

    let mut buf = [0u8; 8192];
    let mut state = FilterState::Normal;
    let mut apc_buf = Vec::with_capacity(4096);
    let mut passthrough_buf = Vec::with_capacity(8192);

    loop {
      let n = match pty_read.read(&mut buf) {
        Ok(0) => {
          // EOF: PTY closed (child exited).
          let _ = (&child_event_write).write_all(&[1]);
          break;
        }
        Ok(n) => n,
        Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
        Err(_) => {
          let _ = (&child_event_write).write_all(&[1]);
          break;
        }
      };

      passthrough_buf.clear();

      for &byte in &buf[..n] {
        match state {
          FilterState::Normal => {
            if byte == 0x1B {
              state = FilterState::Escape;
            } else {
              passthrough_buf.push(byte);
            }
          }
          FilterState::Escape => {
            if byte == b'_' {
              // Start of APC sequence.
              state = FilterState::ApcCollect;
              apc_buf.clear();
            } else {
              // Not APC — pass through the ESC and this byte.
              passthrough_buf.push(0x1B);
              passthrough_buf.push(byte);
              state = FilterState::Normal;
            }
          }
          FilterState::ApcCollect => {
            if byte == 0x1B {
              state = FilterState::ApcEscape;
            } else {
              apc_buf.push(byte);
            }
          }
          FilterState::ApcEscape => {
            if byte == b'\\' {
              // End of APC sequence (ST = ESC \).
              // Check if this is a graphics command (starts with 'G').
              if apc_buf.first() == Some(&b'G') {
                // Strip the 'G' prefix and send.
                let cmd_data = apc_buf[1..].to_vec();
                let _ = graphics_tx.send(cmd_data);
              }
              // Non-graphics APC: silently discard (matches VTE behavior).
              apc_buf.clear();
              state = FilterState::Normal;
            } else {
              // False alarm — ESC inside APC that's not followed by '\'.
              apc_buf.push(0x1B);
              apc_buf.push(byte);
              state = FilterState::ApcCollect;
            }
          }
        }
      }

      // Write filtered bytes to the pipe.
      if !passthrough_buf.is_empty() {
        let _ = pipe_write.write_all(&passthrough_buf);
      }
    }
  }
}

#[cfg(unix)]
pub use unix::GraphicsPtyFilter;
