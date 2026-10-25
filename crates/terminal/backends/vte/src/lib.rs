//! Linux-only VTE backend; other targets build an empty crate.
#![cfg(target_os = "linux")]

pub mod vte_event_loop;
pub mod vte_term;

use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicU32;

use futures::channel::mpsc::{UnboundedReceiver, unbounded};
use parking_lot::Mutex;
use terminal_kernel::program_status::ProgramStatusTracker;
use terminal_kernel::{
  PtyProcessInfo, PtySender, SessionEvents, SessionOptions, ShellLaunch, TerminalSession,
  event::WindowSize, tty,
};

use vte_event_loop::{VteEventLoop, VteMsg, VteSender};
use vte_term::{VteBackend, VteTermInner};

/// PtySender wrapping the VTE event loop channel.
struct VtePtySender(VteSender);

impl PtySender for VtePtySender {
  fn send_input(&self, bytes: Cow<'static, [u8]>) {
    if !bytes.is_empty() {
      let _ = self.0.send(VteMsg::Input(bytes));
    }
  }

  fn send_resize(&self, size: WindowSize) {
    let _ = self.0.send(VteMsg::Resize(size));
  }
}

/// Create a terminal session using the custom VTE backend.
///
/// Uses `xterm-256color` TERM (matching real GNOME Terminal) and advertises
/// VTE-compatible identification via `VTE_VERSION`.
pub fn create_terminal_session(
  program: String,
  args: Vec<String>,
  working_directory: Option<PathBuf>,
  options: &SessionOptions,
  term_program_version: &str,
) -> Result<(TerminalSession, SessionEvents), String> {
  let shell_program = program.clone();
  let ShellLaunch {
    pty_options,
    cwd_file,
  } = ShellLaunch::new(
    program,
    args,
    working_directory,
    &options.env,
    term_program_version,
    &[("VTE_VERSION", "7600")],
  );

  // Event channels.
  let (events_tx, events_rx): (
    futures::channel::mpsc::UnboundedSender<terminal_kernel::event::Event>,
    UnboundedReceiver<terminal_kernel::event::Event>,
  ) = unbounded();

  let (osc7_tx, osc7_rx) = std::sync::mpsc::channel();
  let program_status_tracker = ProgramStatusTracker::default();

  // Default terminal dimensions.
  let size = options.size;
  let num_lines = size.lines as usize;
  let num_cols = size.columns as usize;

  // Build VTE terminal state.
  let state = Arc::new(Mutex::new(VteTermInner::new(
    num_lines,
    num_cols,
    options.scrollback_lines,
    events_tx,
    Some(osc7_tx),
    options.cursor_style.blinking,
  )));

  // Spawn the child shell.
  let pty = tty::new(&pty_options, size.into(), 1)
    .map_err(|e| format!("Could not start shell '{}': {}", shell_program, e))?;

  // Extract PTY fd/child info and build the event loop.
  #[cfg(unix)]
  let (tx, pty_info) = {
    use std::os::unix::io::AsRawFd;

    let raw_fd = pty.file().as_raw_fd();
    let child_pid = pty.child().id();
    let pty_info = PtyProcessInfo::from_raw(raw_fd, child_pid);

    let event_loop = VteEventLoop::new(pty, state.clone(), program_status_tracker.clone(), raw_fd);
    let tx = event_loop.channel();
    let _handle = event_loop.spawn();

    (tx, pty_info)
  };

  #[cfg(not(unix))]
  let (tx, pty_info) = {
    let pty_info = PtyProcessInfo::new(&pty);

    let event_loop = VteEventLoop::new(pty, state.clone(), program_status_tracker.clone());
    let tx = event_loop.channel();
    let _handle = event_loop.spawn();

    (tx, pty_info)
  };

  let backend = VteBackend::new(state);
  let terminal = TerminalSession {
    pty_tx: Box::new(VtePtySender(tx)),
    backend: Box::new(backend),
    pty_info,
    graphics_rx: None,
    pending_cnl: None,
    keyboard_protocol_flags: Arc::new(AtomicU32::new(0)),
    osc7_rx: Some(osc7_rx),
    cwd_file: Some(cwd_file),
    program_status_tracker,
  };

  Ok((terminal, events_rx))
}
