use std::path::PathBuf;

#[cfg(any(feature = "kernel-alacritty", feature = "kernel-vte", feature = "kernel-ghostty"))]
use config::TerminalKernel;
use futures::{FutureExt, StreamExt as _};
use gpui::{AppContext, Context, Entity};

use terminal::TerminalView;

use crate::components::MainWindow;

#[allow(unused_variables)]
fn create_terminal_session(
  program: String,
  args: Vec<String>,
  working_directory: Option<PathBuf>,
  app_config: &config::Config,
) -> Result<(terminal::Terminal, terminal_kernel::SessionEvents), String> {
  match app_config.terminal.kernel {
    #[cfg(feature = "kernel-alacritty")]
    TerminalKernel::Alacritty => terminal_kernel_alacritty::create_terminal_session(
      program,
      args,
      working_directory,
      app_config,
    ),
    #[cfg(feature = "kernel-vte")]
    TerminalKernel::Vte => terminal_kernel_vte::create_terminal_session(
      program,
      args,
      working_directory,
      app_config,
    ),
    #[cfg(feature = "kernel-ghostty")]
    TerminalKernel::Libghostty => terminal_kernel_ghostty::create_terminal_session(
      program,
      args,
      working_directory,
      app_config,
    ),
    #[allow(unreachable_patterns)]
    other => Err(format!(
      "Terminal kernel '{other}' is not available. Enable the corresponding feature to use it: \
       kernel-alacritty, kernel-vte, kernel-ghostty."
    )),
  }
}

pub fn new_terminal_window_with_shell(
  window: &mut gpui::Window,
  index: usize,
  program: &str,
  args: Vec<String>,
  working_directory: Option<PathBuf>,
  cx: &mut Context<MainWindow>,
) -> Result<Entity<TerminalView>, String> {
  let app_config = cx.global::<config::Config>().clone();
  // Use global working_directory as fallback if no per-profile working directory
  let working_directory = working_directory.or_else(|| {
    app_config
      .terminal
      .working_directory
      .as_ref()
      .map(|wd| PathBuf::from(wd))
  });
  let (terminal, events_rx) =
    create_terminal_session(program.to_string(), args, working_directory, &app_config)?;
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
                if matches!(event, terminal_kernel::event::Event::Wakeup) {
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
            this.process_event(terminal_kernel::event::Event::Wakeup, cx);
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

  Ok(cx.new(|cx| TerminalView::new(terminal, window, index, cx)))
}
