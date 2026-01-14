use std::{path::PathBuf, sync::Arc};

use alacritty_terminal::{Term, event_loop::EventLoop, sync::FairMutex, term::Config};
use futures::{FutureExt, StreamExt as _, channel::mpsc::unbounded};
use gpui::{AppContext, Context, Entity};

use terminal::{PtyProcessInfo, TerminalBounds, TerminalEventListener, TerminalView};

use crate::components::MainWindow;

fn new_terminal(
  shell: String,
  working_directory: Option<PathBuf>) -> (
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

  let (events_tx, events_rx) = unbounded();

  let term = Term::new(
    Config::default(),
    &TerminalBounds::default(),
    TerminalEventListener(events_tx.clone()),
  );

  let term = Arc::new(FairMutex::new(term));

  let pty_options = {
    let alac_shell = alacritty_terminal::tty::Shell::new(shell, vec![]);

    alacritty_terminal::tty::Options {
      shell: Some(alac_shell),
      working_directory,
      drain_on_exit: true,
      env: env,
      #[cfg(windows)]
      escape_args: true,
    }
  };

  let pty =
    alacritty_terminal::tty::new(&pty_options, TerminalBounds::default().into(), 1).unwrap();
  let pty_info = PtyProcessInfo::new(&pty);

  let event_loop = EventLoop::new(
    term.clone(),
    TerminalEventListener(events_tx),
    pty,
    pty_options.drain_on_exit,
    false,
  )
  .unwrap();

  let pty_tx = event_loop.channel();
  let _io_thread = event_loop.spawn();

  let terminal = terminal::Terminal::new(pty_tx, term, pty_info);

  (terminal, events_rx)
}

pub fn new_terminal_window_with_shell(
  window: &mut gpui::Window,
  index: usize,
  shell: &str,
  working_directory: Option<PathBuf>,
  cx: &mut Context<MainWindow>,
) -> Entity<TerminalView> {
  let (terminal, events_rx) = new_terminal(shell.to_string(), working_directory);
  let mut events_rx = events_rx;
  let terminal = cx.new(|_| terminal);
  let weak_terminal = terminal.downgrade();

  cx.spawn(async move |_, cx| {
    while let Some(event) = events_rx.next().await {
      match event {
        alacritty_terminal::event::Event::Wakeup => {},
        _ => println!("Event: {:?}", event),
      };

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
