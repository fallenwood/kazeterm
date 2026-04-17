//! Event loop that reads from the PTY, feeds bytes through `vte::Parser`,
//! and handles input/resize/shutdown messages.

use std::borrow::Cow;
use std::io::{self, Read, Write};
use std::sync::Arc;
use std::thread;

use parking_lot::Mutex;
use terminal_kernel::event::WindowSize;

use crate::vte_term::VteTermInner;

/// Messages sent to the VTE event loop.
#[allow(dead_code)]
pub enum VteMsg {
    Input(Cow<'static, [u8]>),
    Resize(WindowSize),
    Shutdown,
}

pub type VteSender = std::sync::mpsc::Sender<VteMsg>;

/// Single-threaded event loop that reads from a PTY and drives the VTE parser.
///
/// On Unix the PTY file descriptor is set to non-blocking so we can interleave
/// reading from the PTY and draining the message channel in one thread.
///
/// The event loop takes ownership of the `Pty` to keep the child process alive.
/// When the loop exits the `Pty` is dropped, which sends SIGHUP to the child.
pub struct VteEventLoop {
    tx: VteSender,
    rx: std::sync::mpsc::Receiver<VteMsg>,
    pty_reader: std::fs::File,
    pty_writer: std::fs::File,
    state: Arc<Mutex<VteTermInner>>,
    #[cfg(unix)]
    pty_raw_fd: i32,
    /// Keeps the child process alive for the lifetime of the event loop.
    _pty: terminal_kernel::tty::Pty,
}

impl VteEventLoop {
    /// Create a new event loop.
    ///
    /// `pty_reader` / `pty_writer` are cloned file handles to the PTY master.
    /// On Unix they share the same underlying fd.
    pub fn new(
        pty: terminal_kernel::tty::Pty,
        pty_reader: std::fs::File,
        pty_writer: std::fs::File,
        state: Arc<Mutex<VteTermInner>>,
        #[cfg(unix)] pty_raw_fd: i32,
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
        }
    }

    /// Get a clone of the sender for sending messages to this loop.
    pub fn channel(&self) -> VteSender {
        self.tx.clone()
    }

    /// Spawn the event loop on a dedicated thread.
    pub fn spawn(self) -> thread::JoinHandle<()> {
        thread::Builder::new()
            .name("vte-event-loop".into())
            .spawn(move || {
                self.run();
            })
            .expect("spawn vte event loop")
    }

    fn run(mut self) {
        // Set PTY to non-blocking on Unix so we can interleave channel draining.
        #[cfg(unix)]
        {
            unsafe {
                let flags = libc::fcntl(self.pty_raw_fd, libc::F_GETFL);
                libc::fcntl(self.pty_raw_fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
            }
        }

        let mut parser = vte::Parser::new();
        let mut buf = [0u8; 4096];

        loop {
            // Drain the message channel (non-blocking).
            loop {
                match self.rx.try_recv() {
                    Ok(VteMsg::Input(bytes)) => {
                        let _ = self.pty_writer.write_all(&bytes);
                        let _ = self.pty_writer.flush();
                    }
                    Ok(VteMsg::Resize(size)) => {
                        // Resize the PTY via ioctl.
                        #[cfg(unix)]
                        {
                            let win = libc::winsize {
                                ws_row: size.num_lines,
                                ws_col: size.num_cols,
                                ws_xpixel: size.cell_width.saturating_mul(size.num_cols),
                                ws_ypixel: size.cell_height.saturating_mul(size.num_lines),
                            };
                            unsafe {
                                libc::ioctl(
                                    self.pty_raw_fd,
                                    libc::TIOCSWINSZ,
                                    &win as *const _,
                                );
                            }
                        }
                        // Resize the grid.
                        self.state
                            .lock()
                            .do_resize(size.num_lines as usize, size.num_cols as usize);
                    }
                    Ok(VteMsg::Shutdown) => return,
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => return,
                }
            }

            // Read from PTY (non-blocking on Unix).
            match self.pty_reader.read(&mut buf) {
                Ok(0) => {
                    // EOF — child process exited.
                    self.state
                        .lock()
                        .send_event(terminal_kernel::event::Event::Exit);
                    return;
                }
                Ok(n) => {
                    let mut state = self.state.lock();
                    parser.advance(&mut *state, &buf[..n]);
                    drop(state);
                    // Wake up the UI.
                    self.state
                        .lock()
                        .send_event(terminal_kernel::event::Event::Wakeup);
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    // No data ready — sleep briefly to avoid busy-spinning.
                    thread::sleep(std::time::Duration::from_millis(2));
                }
                Err(_) => {
                    // PTY error — treat as exit.
                    self.state
                        .lock()
                        .send_event(terminal_kernel::event::Event::Exit);
                    return;
                }
            }
        }
    }
}
