use std::{path::PathBuf, sync::Arc};

use alacritty_terminal::{
  Term, event_loop::EventLoop, grid::Dimensions, sync::FairMutex, term::Config,
};
use futures::{FutureExt, StreamExt as _, channel::mpsc::unbounded};
use gpui::{AppContext, Context, Entity};

use terminal::{PtyProcessInfo, TerminalBounds, TerminalEventListener, TerminalView};

use crate::components::MainWindow;

fn new_terminal(
  program: String,
  args: Vec<String>,
  working_directory: Option<PathBuf>,
) -> (
  terminal::Terminal,
  futures::channel::mpsc::UnboundedReceiver<alacritty_terminal::event::Event>,
) {
  let mut env = std::collections::HashMap::new();
  if std::env::var("LANG").is_err() {
    env
      .entry("LANG".to_string())
      .or_insert_with(|| "en_US.UTF-8".to_string());
  }

  env.insert("TERM_PROGRAM".to_string(), "kazeterm".to_string());
  env.insert("TERM".to_string(), "xterm-256color".to_string());
  env.insert("COLORTERM".to_string(), "truecolor".to_string());

  #[cfg(windows)]
  let graphics_pipe_name = {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let name = format!(
      "\\\\.\\pipe\\kazeterm-graphics-{}-{}",
      std::process::id(),
      id
    );
    env.insert("KAZETERM_GRAPHICS_PIPE".to_string(), name.clone());
    name
  };

  let (events_tx, events_rx) = unbounded();

  let term = Term::new(
    Config::default(),
    &TerminalBounds::default(),
    TerminalEventListener(events_tx.clone()),
  );

  let term = Arc::new(FairMutex::new(term));

  let pty_options = {
    let alac_shell = alacritty_terminal::tty::Shell::new(program, args);

    alacritty_terminal::tty::Options {
      shell: Some(alac_shell),
      working_directory,
      drain_on_exit: true,
      env,
      #[cfg(windows)]
      escape_args: true,
    }
  };

  let pty =
    alacritty_terminal::tty::new(&pty_options, TerminalBounds::default().into(), 1).unwrap();

  #[cfg(unix)]
  let (pty_tx, pty_info, graphics_rx, pending_cnl) = {
    use terminal::kitty_graphics::GraphicsPtyFilter;

    let term_for_cursor = term.clone();
    let cursor_fn: Box<dyn Fn() -> Option<(i32, i32)> + Send + Sync> =
      Box::new(move || {
        let t = term_for_cursor.try_lock_unfair()?;
        let cursor = t.grid().cursor.point;
        let hs = t.history_size() as i32;
        Some((hs + cursor.line.0, cursor.column.0 as i32))
      });

    let (filter, pending_cnl, graphics_rx) =
      GraphicsPtyFilter::new(pty, cursor_fn).unwrap();
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

    (pty_tx, pty_info, Some(graphics_rx), Some(pending_cnl))
  };

  #[cfg(windows)]
  let (pty_tx, pty_info, graphics_rx, pending_cnl) = {
    use terminal::kitty_graphics::GraphicsPtyFilter;

    let (graphics_tx, graphics_rx) = std::sync::mpsc::channel();
    let pending_cnl = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));

    // Start named pipe server for graphics (bypasses ConPTY's APC stripping).
    let term_for_pipe = term.clone();
    let pipe_cursor_fn: Box<dyn Fn() -> Option<(i32, i32)> + Send + Sync> =
      Box::new(move || {
        let t = term_for_pipe.try_lock_unfair()?;
        let cursor = t.grid().cursor.point;
        let hs = t.history_size() as i32;
        Some((hs + cursor.line.0, cursor.column.0 as i32))
      });
    terminal::kitty_graphics::graphics_pipe::start_server(
      graphics_pipe_name,
      graphics_tx.clone(),
      pipe_cursor_fn,
      pending_cnl.clone(),
    );

    // Wrap PTY with full APC filter. When ConPTY passes APC through, the
    // filter intercepts it (accurate cursor position + inline CNL injection).
    // When ConPTY strips APC, only the pipe provides commands.
    // Both share the same channel; Terminal deduplicates via from_filter flag.
    let term_for_filter = term.clone();
    let filter_cursor_fn: Box<dyn Fn() -> Option<(i32, i32)> + Send + Sync> =
      Box::new(move || {
        let t = term_for_filter.try_lock_unfair()?;
        let cursor = t.grid().cursor.point;
        let hs = t.history_size() as i32;
        Some((hs + cursor.line.0, cursor.column.0 as i32))
      });
    let filter =
      GraphicsPtyFilter::new_shared(pty, filter_cursor_fn, graphics_tx, pending_cnl.clone())
        .unwrap();

    let pty_info = PtyProcessInfo::from_raw(filter.child_handle(), filter.child_pid());

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

    (pty_tx, pty_info, Some(graphics_rx), Some(pending_cnl))
  };

  let terminal = terminal::Terminal::new(pty_tx, term, pty_info, graphics_rx, pending_cnl);

  (terminal, events_rx)
}

pub fn new_terminal_window_with_shell(
  window: &mut gpui::Window,
  index: usize,
  program: &str,
  args: Vec<String>,
  working_directory: Option<PathBuf>,
  cx: &mut Context<MainWindow>,
) -> Entity<TerminalView> {
  let (terminal, events_rx) = new_terminal(program.to_string(), args, working_directory);
  let mut events_rx = events_rx;
  let terminal = cx.new(|_| terminal);
  let weak_terminal = terminal.downgrade();

  cx.spawn(async move |_, cx| {
    while let Some(event) = events_rx.next().await {
      let terminal = match weak_terminal.upgrade() {
        Some(terminal) => terminal,
        None => break,
      };

      _ = terminal.update(cx, |t, cx| {
        //Process the first event immediately for lowered latency
        t.process_event(event, cx);
      });

      'outer: loop {
        let mut events = Vec::new();

        let mut timer = cx
          .background_executor()
          .timer(std::time::Duration::from_millis(4))
          .fuse();

        let mut wakeup = false;
        loop {
          futures::select_biased! {
            _ = timer => break,
            event = events_rx.next() => {
              if let Some(event) = event {
                if matches!(event, alacritty_terminal::event::Event::Wakeup) {
                  wakeup = true;
                } else {
                  events.push(event);
                }

                if events.len() > 100 {
                  break;
                }
              } else {
                break;
              }
            },
          }
        }

        if events.is_empty() && !wakeup {
          smol::future::yield_now().await;
          break 'outer;
        }

        _ = terminal.update(cx, |this, cx| {
          if wakeup {
            this.process_event(alacritty_terminal::event::Event::Wakeup, cx);
          }

          for event in events {
            this.process_event(event, cx);
          }
        });
        smol::future::yield_now().await;
      }
    }
  })
  .detach();

  cx.new(|cx| TerminalView::new(terminal, window, index, cx))
}
