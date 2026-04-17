use std::{collections::HashMap, path::PathBuf, sync::Arc};

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::Osc52;
use alacritty_terminal::vte::ansi::{CursorShape, CursorStyle};
use futures::{channel::mpsc::UnboundedReceiver, channel::mpsc::unbounded};
use terminal::{PtyProcessInfo, Terminal, TerminalBounds, TerminalEventListener};
use terminal_kernel::{
  AlacrittyBackend, SessionEvents, Term, event_loop::EventLoop, sync::FairMutex, term::Config, tty,
};

#[cfg(unix)]
use terminal::kitty_graphics::GraphicsPtyFilter;
#[cfg(not(unix))]
use terminal::kitty_graphics::{WindowsDsrCursorFn, WindowsDsrFilter};

fn parse_cursor_style(config: &config::Config) -> CursorStyle {
  let shape = match config.cursor.shape.as_str() {
    "underline" => CursorShape::Underline,
    "beam" => CursorShape::Beam,
    _ => CursorShape::Block,
  };

  CursorStyle {
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

/// Create a terminal session emulating a GNOME VTE terminal.
///
/// Uses `xterm-256color` TERM (matching real GNOME Terminal) and advertises
/// VTE-compatible identification via `VTE_VERSION`.
pub fn create_terminal_session(
  program: String,
  args: Vec<String>,
  working_directory: Option<PathBuf>,
  app_config: &config::Config,
) -> Result<(Terminal, SessionEvents), String> {
  let mut env = HashMap::new();
  if std::env::var("LANG").is_err() {
    env
      .entry("LANG".to_string())
      .or_insert_with(|| "en_US.UTF-8".to_string());
  }

  env.insert("TERM_PROGRAM".to_string(), "kazeterm".to_string());
  env.insert(
    "TERM_PROGRAM_VERSION".to_string(),
    env!("CARGO_PKG_VERSION").to_string(),
  );
  // VTE-based terminals use xterm-256color.
  env.insert("TERM".to_string(), "xterm-256color".to_string());
  env.insert("COLORTERM".to_string(), "truecolor".to_string());
  // Advertise VTE compatibility so shell integration scripts can detect us.
  env.insert("VTE_VERSION".to_string(), "7600".to_string());

  for (key, value) in &app_config.terminal.env {
    env.insert(key.clone(), value.clone());
  }

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

  let shell_name = std::path::Path::new(&program)
    .file_stem()
    .and_then(|s| s.to_str())
    .unwrap_or("")
    .to_lowercase();

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

  let (events_tx, events_rx): (
    futures::channel::mpsc::UnboundedSender<terminal_kernel::event::Event>,
    UnboundedReceiver<terminal_kernel::event::Event>,
  ) = unbounded();

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
    let shell = tty::Shell::new(program, args);

    tty::Options {
      shell: Some(shell),
      working_directory,
      drain_on_exit: true,
      env,
      #[cfg(windows)]
      escape_args: true,
    }
  };

  let pty = tty::new(&pty_options, TerminalBounds::default().into(), 1)
    .map_err(|e| format!("Could not start shell '{}': {}", shell_program, e))?;

  #[cfg(unix)]
  let (pty_tx, pty_info, graphics_rx, pending_cnl, osc7_rx) = {
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

  let backend = AlacrittyBackend::new(term);
  let terminal = Terminal::new(
    pty_tx,
    Box::new(backend),
    pty_info,
    graphics_rx,
    pending_cnl,
    osc7_rx,
    Some(cwd_file),
  );

  Ok((terminal, events_rx))
}
