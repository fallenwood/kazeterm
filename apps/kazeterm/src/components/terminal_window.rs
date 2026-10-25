use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use config::TerminalKernel;
use futures::{FutureExt, StreamExt as _};
use gpui::{AppContext, Context, Entity};

use terminal::TerminalView;
use terminal_kernel::term::Osc52;
use terminal_kernel::vte::ansi::{CursorShape, CursorStyle};
use terminal_kernel::{SessionOptions, TerminalSize};

use crate::components::MainWindow;

/// Signature for a test-injected terminal session factory.
///
/// Installed at process start-up by tests; production never sets this.
pub type TerminalSessionFactory = Box<
  dyn Fn(
      String,
      Vec<String>,
      Option<PathBuf>,
      &config::Config,
    ) -> Result<(terminal::Terminal, terminal_kernel::SessionEvents), String>
    + Send
    + Sync,
>;

static SESSION_FACTORY_OVERRIDE: OnceLock<Mutex<Option<TerminalSessionFactory>>> = OnceLock::new();

/// Install a test-only factory that will be consulted by
/// [`create_terminal_session`] in lieu of spawning a real PTY.
///
/// Called only from tests. Overwrites any previously installed factory.
#[doc(hidden)]
#[cfg_attr(not(test), allow(dead_code))]
pub fn set_terminal_session_factory_for_testing(factory: TerminalSessionFactory) {
  let slot = SESSION_FACTORY_OVERRIDE.get_or_init(|| Mutex::new(None));
  *slot.lock().unwrap() = Some(factory);
}

/// Remove any test factory override. Called from `#[gpui::test]` teardown.
#[doc(hidden)]
#[cfg_attr(not(test), allow(dead_code))]
pub fn clear_terminal_session_factory_for_testing() {
  if let Some(slot) = SESSION_FACTORY_OVERRIDE.get() {
    *slot.lock().unwrap() = None;
  }
}

/// Translate persisted settings once, at the application/backend boundary.
fn session_options(config: &config::Config) -> SessionOptions {
  let shape = match config.cursor.shape.as_str() {
    "underline" => CursorShape::Underline,
    "beam" => CursorShape::Beam,
    _ => CursorShape::Block,
  };
  let osc52 = match config.terminal.osc52.as_str() {
    "disabled" => Osc52::Disabled,
    "paste_only" => Osc52::OnlyPaste,
    "copy_paste" => Osc52::CopyPaste,
    _ => Osc52::OnlyCopy,
  };

  SessionOptions {
    size: TerminalSize::default(),
    scrollback_lines: config.terminal.get_scrollback_lines(),
    cursor_style: CursorStyle {
      shape,
      blinking: config.cursor.blink,
    },
    osc52,
    env: config.terminal.env.clone(),
  }
}

fn create_terminal_session(
  program: String,
  args: Vec<String>,
  working_directory: Option<PathBuf>,
  app_config: &config::Config,
) -> Result<(terminal::Terminal, terminal_kernel::SessionEvents), String> {
  // Tests can install a factory to avoid spawning a child process.
  if let Some(slot) = SESSION_FACTORY_OVERRIDE.get()
    && let Some(factory) = slot.lock().unwrap().as_ref()
  {
    return factory(program, args, working_directory, app_config);
  }

  let kernel = app_config.terminal.kernel;
  let term_program_version = crate::build_info::terminal_program_version(kernel);
  let options = session_options(app_config);

  let session = match app_config.terminal.kernel {
    TerminalKernel::Alacritty => terminal_kernel_alacritty::create_terminal_session(
      program,
      args,
      working_directory,
      &options,
      term_program_version,
    ),
    #[cfg(target_os = "linux")]
    TerminalKernel::Vte => terminal_kernel_vte::create_terminal_session(
      program,
      args,
      working_directory,
      &options,
      term_program_version,
    ),
    #[allow(unreachable_patterns)]
    other => Err(format!(
      "Terminal kernel '{other}' is not available on this platform. Supported kernels are: \
       alacritty (Windows/Linux/macOS), vte (Linux)."
    )),
  };

  session.map(|(session, events)| (terminal::Terminal::from(session), events))
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

#[cfg(test)]
mod tests {
  use super::*;

  #[gpui::test]
  fn unpainted_terminal_processes_graphics_probes_and_preserves_upload_on_view_recreation(
    cx: &mut gpui::TestAppContext,
  ) {
    use futures::channel::mpsc;
    use std::sync::{Arc, atomic::AtomicU32};
    use terminal::test_support::fake_terminal_session;
    use terminal_kernel::protocols::{RawGraphicsCommand, RawGraphicsEvent};

    // Own a TerminalView without ever rendering it as a child. This exercises
    // background/unpainted setup, not the old lazy start in Terminal::sync().
    struct UnpaintedTerminal {
      view: Entity<TerminalView>,
    }
    impl gpui::Render for UnpaintedTerminal {
      fn render(
        &mut self,
        _window: &mut gpui::Window,
        _cx: &mut Context<Self>,
      ) -> impl gpui::IntoElement {
        gpui::div()
      }
    }
    crate::test_support::init_test_app(cx);
    let (original, _, writes, _) = fake_terminal_session(80, 24);
    let (tx, rx) = mpsc::unbounded();
    let model = cx.new(|_| {
      terminal::Terminal::new(
        original.pty_tx,
        original.term,
        original.pty_info,
        Some(rx),
        None,
        Arc::new(AtomicU32::new(0)),
        None,
        None,
        original.program_status_tracker,
      )
    });
    let window = cx.add_window(|window, cx| UnpaintedTerminal {
      view: cx.new(|cx| TerminalView::new(model.clone(), window, 0, cx)),
    });
    let send = |data: &[u8]| {
      tx.unbounded_send(RawGraphicsEvent::Command(RawGraphicsCommand {
        data: data.to_vec(),
        cursor_line: 0,
        cursor_column: 0,
      }))
      .unwrap()
    };
    send(b"a=q,i=31,f=24,s=1,v=1;AAAA");
    tx.unbounded_send(RawGraphicsEvent::Reply(b"\x1b[?62;22c".to_vec()))
      .unwrap();
    cx.run_until_parked();
    assert_eq!(
      *writes.lock().unwrap(),
      [b"\x1b_Gi=31;OK\x1b\\".to_vec(), b"\x1b[?62;22c".to_vec()]
    );
    assert_eq!(
      model.read_with(cx, |terminal, _| terminal.image_storage.image_count()),
      0
    );

    send(b"a=t,i=7,p=3,s=1,v=1,m=1;AAAA");
    window
      .update(cx, |host, window, cx| {
        host.view = cx.new(|cx| TerminalView::new(model.clone(), window, 1, cx));
      })
      .unwrap();
    send(b"m=0;AA==");
    cx.run_until_parked();
    assert_eq!(
      writes.lock().unwrap().last().unwrap(),
      b"\x1b_Gi=7,p=3;OK\x1b\\"
    );
    assert_eq!(
      model.read_with(cx, |terminal, _| terminal.image_storage.image_count()),
      1
    );
  }

  #[test]
  fn session_options_preserve_backend_settings_and_fallbacks() {
    let mut config = config::Config::default();
    config.terminal.scrollback_lines = 200_000;
    config.terminal.env.insert("CUSTOM".into(), "value".into());
    config.cursor.blink = false;

    for (shape, expected) in [
      ("underline", CursorShape::Underline),
      ("beam", CursorShape::Beam),
      ("block", CursorShape::Block),
      ("unknown", CursorShape::Block),
    ] {
      config.cursor.shape = shape.into();
      let options = session_options(&config);
      assert_eq!(options.cursor_style.shape, expected);
      assert!(!options.cursor_style.blinking);
      assert_eq!(options.scrollback_lines, 100_000);
      assert_eq!(options.env, config.terminal.env);
      let initial: terminal_kernel::event::WindowSize = options.size.into();
      let previous: terminal_kernel::event::WindowSize = terminal::TerminalBounds::default().into();
      assert_eq!(
        (
          initial.num_lines,
          initial.num_cols,
          initial.cell_width,
          initial.cell_height
        ),
        (
          previous.num_lines,
          previous.num_cols,
          previous.cell_width,
          previous.cell_height
        ),
      );
    }

    for (mode, expected) in [
      ("disabled", Osc52::Disabled),
      ("paste_only", Osc52::OnlyPaste),
      ("copy_paste", Osc52::CopyPaste),
      ("copy_only", Osc52::OnlyCopy),
      ("unknown", Osc52::OnlyCopy),
    ] {
      config.terminal.osc52 = mode.into();
      assert_eq!(session_options(&config).osc52, expected);
    }
  }
}
