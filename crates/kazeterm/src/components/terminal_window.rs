use std::{path::PathBuf, sync::Arc};

use alacritty_terminal::term::Osc52;
use alacritty_terminal::vte::ansi::{CursorShape, CursorStyle as AlacCursorStyle};
use alacritty_terminal::{
  Term, event_loop::EventLoop, grid::Dimensions, sync::FairMutex, term::Config,
};
use futures::{FutureExt, StreamExt as _, channel::mpsc::unbounded};
use gpui::{AppContext, Context, Entity};

use terminal::{PtyProcessInfo, TerminalBounds, TerminalEventListener, TerminalView};

use crate::components::MainWindow;

fn parse_cursor_style(config: &config::Config) -> AlacCursorStyle {
  let shape = match config.cursor.shape.as_str() {
    "underline" => CursorShape::Underline,
    "beam" => CursorShape::Beam,
    _ => CursorShape::Block,
  };
  AlacCursorStyle {
    shape,
    blinking: config.cursor.blink,
  }
}

fn parse_osc52(mode: &str) -> Osc52 {
  match mode {
    "disabled" => Osc52::Disabled,
    "paste_only" => Osc52::OnlyPaste,
    "copy_paste" => Osc52::CopyPaste,
    _ => Osc52::OnlyCopy,
  }
}

fn new_terminal(
  program: String,
  args: Vec<String>,
  working_directory: Option<PathBuf>,
  app_config: &config::Config,
) -> Result<
  (
    terminal::Terminal,
    futures::channel::mpsc::UnboundedReceiver<alacritty_terminal::event::Event>,
  ),
  String,
> {
  let mut env = std::collections::HashMap::new();
  if std::env::var("LANG").is_err() {
    env
      .entry("LANG".to_string())
      .or_insert_with(|| "en_US.UTF-8".to_string());
  }

  env.insert("TERM_PROGRAM".to_string(), "kazeterm".to_string());
  env.insert("TERM".to_string(), "xterm-256color".to_string());
  env.insert("COLORTERM".to_string(), "truecolor".to_string());

  // Merge user-specified env vars (these can override defaults above)
  for (key, value) in &app_config.terminal.env {
    env.insert(key.clone(), value.clone());
  }

  // Create a temp file for CWD communication (cross-platform, works on Windows).
  // The shell writes its CWD to this file on each prompt.
  let cwd_file = std::env::temp_dir().join(format!(
    "kazeterm-cwd-{}-{}",
    std::process::id(),
    std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .map(|d| d.as_nanos())
      .unwrap_or(0)
  ));
  env.insert(
    "KAZETERM_CWD_FILE".to_string(),
    cwd_file.to_string_lossy().to_string(),
  );

  // Shell integration: inject hooks so shells report their CWD.
  let shell_name = std::path::Path::new(&program)
    .file_stem()
    .and_then(|s| s.to_str())
    .unwrap_or("")
    .to_lowercase();

  // Bash/Zsh CWD hook: emit OSC 7 AND write to cwd_file.
  let cwd_file_str = cwd_file.to_string_lossy();
  let bash_hook = format!(
    r#"printf '\e]7;file://%s%s\e\\' "$(hostname)" "$PWD"; printf '%s' "$PWD" >| "{}""#,
    cwd_file_str,
  );
  env.insert("__KAZETERM_OSC7".to_string(), bash_hook.clone());

  if shell_name == "bash" || shell_name == "sh" {
    let existing = std::env::var("PROMPT_COMMAND").unwrap_or_default();
    let prompt_cmd = if existing.is_empty() {
      bash_hook
    } else {
      format!("{bash_hook};{existing}")
    };
    env.insert("PROMPT_COMMAND".to_string(), prompt_cmd);
  } else if shell_name == "zsh" {
    env.insert("PROMPT_COMMAND".to_string(), bash_hook);
  }

  // PowerShell: auto-inject the prompt hook via -Command.
  // Writes CWD to temp file (reliable on Windows) and also emits OSC 7 (for Unix).
  let pwsh_init = format!(
    concat!(
      r#"$__kazeterm_orig_prompt = $function:prompt; "#,
      r#"function prompt {{ "#,
      r#"$cwd = (Get-Location).Path; "#,
      r#"[System.IO.File]::WriteAllText('{}', $cwd); "#,
      r#"$esc = [char]27; "#,
      r#"$host_name = [System.Net.Dns]::GetHostName(); "#,
      r#"[Console]::Write("${{esc}}]7;file://${{host_name}}${{cwd}}${{esc}}\"); "#,
      r#"if ($__kazeterm_orig_prompt) {{ & $__kazeterm_orig_prompt }} "#,
      r#"else {{ "PS $($executionContext.SessionState.Path.CurrentLocation)$('>' * ($nestedPromptLevel + 1)) " }} "#,
      r#"}}"#,
    ),
    cwd_file_str,
  );
  env.insert("KAZETERM_PWSH_OSC7_INIT".to_string(), pwsh_init.clone());

  let mut args = args;
  if shell_name == "pwsh" || shell_name == "powershell" {
    if args.is_empty() {
      args = vec!["-NoExit".to_string(), "-Command".to_string(), pwsh_init];
    }
  }

  let (events_tx, events_rx) = unbounded();

  let term = Term::new(
    Config {
      scrolling_history: app_config.terminal.get_scrollback_lines(),
      default_cursor_style: parse_cursor_style(app_config),
      osc52: parse_osc52(&app_config.terminal.osc52),
      ..Config::default()
    },
    &TerminalBounds::default(),
    TerminalEventListener(events_tx.clone()),
  );

  let term = Arc::new(FairMutex::new(term));

  let shell_program = program.clone();
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
    alacritty_terminal::tty::new(&pty_options, TerminalBounds::default().into(), 1)
      .map_err(|e| format!("Could not start shell '{}': {}", shell_program, e))?;

  #[cfg(unix)]
  let (pty_tx, pty_info, graphics_rx, pending_cnl, osc7_rx) = {
    use terminal::kitty_graphics::GraphicsPtyFilter;

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

    let (filter, pending_cnl, graphics_rx, osc7_rx) =
      GraphicsPtyFilter::new(pty, cursor_fn, dsr_cursor_fn).unwrap();
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
      Some(osc7_rx),
    )
  };

  #[cfg(not(unix))]
  let (pty_tx, pty_info, graphics_rx, pending_cnl, osc7_rx) = {
    use terminal::kitty_graphics::{WindowsDsrCursorFn, WindowsDsrFilter};

    let term_for_dsr = term.clone();
    let dsr_cursor_fn: WindowsDsrCursorFn = Box::new(move || {
      let t = term_for_dsr.try_lock_unfair()?;
      let cursor = t.grid().cursor.point;
      Some((cursor.line.0 + 1, cursor.column.0 as i32 + 1))
    });

    let pty_info = PtyProcessInfo::new(&pty);
    let filter = WindowsDsrFilter::new(pty, dsr_cursor_fn);

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

    (pty_tx, pty_info, None, None, None)
  };

  let terminal = terminal::Terminal::new(
    pty_tx,
    term,
    pty_info,
    graphics_rx,
    pending_cnl,
    osc7_rx,
    Some(cwd_file),
  );

  Ok((terminal, events_rx))
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
    new_terminal(program.to_string(), args, working_directory, &app_config)?;
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

  Ok(cx.new(|cx| TerminalView::new(terminal, window, index, cx)))
}
