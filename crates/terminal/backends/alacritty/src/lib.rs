use std::{borrow::Cow, path::PathBuf, sync::Arc};

use futures::{channel::mpsc::UnboundedReceiver, channel::mpsc::unbounded};
use terminal_kernel::program_status::ProgramStatusTracker;
use terminal_kernel::{
  AlacrittyBackend, PtyProcessInfo, PtySender, SessionEvents, SessionOptions, ShellLaunch, Term,
  TerminalEventListener, TerminalSession,
  event::WindowSize,
  event_loop::{EventLoop, EventLoopSender, Msg},
  sync::FairMutex,
  term::Config,
  tty,
};

#[cfg(unix)]
use terminal_kernel::grid::Dimensions as _;
#[cfg(unix)]
use terminal_kernel::protocols::pty_filter::GraphicsPtyFilter;
#[cfg(not(unix))]
use terminal_kernel::protocols::pty_filter::{WindowsDsrCursorFn, WindowsDsrFilter};

/// PtySender implementation wrapping alacritty's EventLoopSender.
pub struct AlacrittyPtySender(EventLoopSender);

impl PtySender for AlacrittyPtySender {
  fn send_input(&self, bytes: Cow<'static, [u8]>) {
    if !bytes.is_empty() {
      let _ = self.0.send(Msg::Input(bytes));
    }
  }

  fn send_resize(&self, size: WindowSize) {
    let _ = self.0.send(Msg::Resize(size));
  }
}

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
    &[],
  );

  let (events_tx, events_rx): (
    futures::channel::mpsc::UnboundedSender<terminal_kernel::event::Event>,
    UnboundedReceiver<terminal_kernel::event::Event>,
  ) = unbounded();

  let term = Term::new(
    Config {
      scrolling_history: options.scrollback_lines,
      default_cursor_style: options.cursor_style,
      osc52: options.osc52,
      ..Config::default()
    },
    &options.size,
    TerminalEventListener(events_tx.clone()),
  );

  let term = Arc::new(FairMutex::new(term));
  let program_status_tracker = ProgramStatusTracker::default();

  let pty = tty::new(&pty_options, options.size.into(), 1)
    .map_err(|e| format!("Could not start shell '{}': {}", shell_program, e))?;

  #[cfg(unix)]
  let (pty_tx, pty_info, graphics_rx, pending_cnl, keyboard_flags, osc7_rx) = {
    let term_for_cursor = term.clone();
    let cursor_fn: Box<dyn Fn() -> Option<(i32, i32)> + Send + Sync> = Box::new(move || {
      let t = term_for_cursor.try_lock_unfair()?;
      let cursor = t.grid().cursor.point;
      let hs = t.history_size() as i32;
      Some((hs + cursor.line.0, cursor.column.0 as i32))
    });

    let term_for_dsr = term.clone();
    let dsr_cursor_fn: Box<dyn Fn() -> Option<(i32, i32)> + Send + Sync> = Box::new(move || {
      let t = term_for_dsr.try_lock_unfair()?;
      let cursor = t.grid().cursor.point;
      Some((cursor.line.0 + 1, cursor.column.0 as i32 + 1))
    });

    let (filter, pending_cnl, keyboard_flags, graphics_rx, osc7_rx) = GraphicsPtyFilter::new(
      pty,
      cursor_fn,
      dsr_cursor_fn,
      program_status_tracker.clone(),
    )
    .unwrap();
    let pty_info = PtyProcessInfo::from_raw(filter.pty_fd(), filter.child_pid());

    let event_loop = EventLoop::new(
      term.clone(),
      TerminalEventListener(events_tx),
      filter,
      pty_options.drain_on_exit,
      false,
    )
    .unwrap();

    let pty_tx = event_loop.channel();
    let _io_thread = event_loop.spawn();

    (
      pty_tx,
      pty_info,
      Some(graphics_rx),
      Some(pending_cnl),
      keyboard_flags,
      Some(osc7_rx),
    )
  };

  #[cfg(not(unix))]
  let (pty_tx, pty_info, graphics_rx, pending_cnl, keyboard_flags, osc7_rx) = {
    let term_for_dsr = term.clone();
    let dsr_cursor_fn: WindowsDsrCursorFn = Box::new(move || {
      let t = term_for_dsr.try_lock_unfair()?;
      let cursor = t.grid().cursor.point;
      Some((cursor.line.0 + 1, cursor.column.0 as i32 + 1))
    });

    let pty_info = PtyProcessInfo::new(&pty);
    let (filter, keyboard_flags) =
      WindowsDsrFilter::new(pty, dsr_cursor_fn, program_status_tracker.clone());

    let event_loop = EventLoop::new(
      term.clone(),
      TerminalEventListener(events_tx),
      filter,
      pty_options.drain_on_exit,
      false,
    )
    .unwrap();

    let pty_tx = event_loop.channel();
    let _io_thread = event_loop.spawn();

    (pty_tx, pty_info, None, None, keyboard_flags, None)
  };

  let backend = AlacrittyBackend::new(term);
  let terminal = TerminalSession {
    pty_tx: Box::new(AlacrittyPtySender(pty_tx)),
    backend: Box::new(backend),
    pty_info,
    graphics_rx,
    pending_cnl,
    keyboard_protocol_flags: keyboard_flags,
    osc7_rx,
    cwd_file: Some(cwd_file),
    program_status_tracker,
  };

  Ok((terminal, events_rx))
}
