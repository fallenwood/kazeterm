pub mod ghostty_event_loop;
pub mod ghostty_term;

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicU32;

use futures::channel::mpsc::{UnboundedReceiver, unbounded};
use parking_lot::Mutex;
use terminal::{PtyProcessInfo, PtySender, Terminal, TerminalBounds};
use terminal_kernel::{SessionEvents, event::WindowSize, tty};

use ghostty_event_loop::{GhosttyEventLoop, GhosttyMsg, GhosttyMsgSender};
use ghostty_term::{GhosttyBackend, GhosttyTermInner};

/// PtySender wrapping the ghostty event loop channel.
struct GhosttyPtySender(GhosttyMsgSender);

impl PtySender for GhosttyPtySender {
  fn send_input(&self, bytes: Cow<'static, [u8]>) {
    if !bytes.is_empty() {
      let _ = self.0.send(GhosttyMsg::Input(bytes));
    }
  }

  fn send_resize(&self, size: WindowSize) {
    let _ = self.0.send(GhosttyMsg::Resize(size));
  }
}

/// Create a terminal session using the libghostty backend.
///
/// Uses `xterm-256color` TERM and advertises Ghostty identification via
/// `TERM_PROGRAM`.
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
  env.insert("TERM".to_string(), "xterm-256color".to_string());
  env.insert("COLORTERM".to_string(), "truecolor".to_string());

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

  // Event channels.
  let (events_tx, events_rx): (
    futures::channel::mpsc::UnboundedSender<terminal_kernel::event::Event>,
    UnboundedReceiver<terminal_kernel::event::Event>,
  ) = unbounded();

  let (_osc7_tx, osc7_rx) = std::sync::mpsc::channel();

  // Default terminal dimensions.
  let bounds = TerminalBounds::default();
  let num_lines = bounds.num_lines();
  let num_cols = bounds.num_columns();

  // Build ghostty terminal state.
  let state = Arc::new(Mutex::new(GhosttyTermInner::new(
    num_lines,
    num_cols,
    app_config.terminal.get_scrollback_lines(),
    events_tx.clone(),
  )));

  // Spawn the child shell.
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

  let pty = tty::new(&pty_options, bounds.into(), 1)
    .map_err(|e| format!("Could not start shell '{}': {}", shell_program, e))?;

  #[cfg(unix)]
  let (tx, pty_info) = {
    use std::os::unix::io::AsRawFd;

    let raw_fd = pty.file().as_raw_fd();
    let child_pid = pty.child().id();
    let pty_info = PtyProcessInfo::from_raw(raw_fd, child_pid);

    let event_loop = GhosttyEventLoop::new(
      pty,
      state.clone(),
      events_tx.clone(),
      raw_fd,
      num_cols as u16,
      num_lines as u16,
      app_config.terminal.get_scrollback_lines(),
    );
    let tx = event_loop.channel();
    let _handle = event_loop.spawn();

    (tx, pty_info)
  };

  #[cfg(not(unix))]
  let (tx, pty_info) = {
    let pty_info = PtyProcessInfo::new(&pty);

    let event_loop = GhosttyEventLoop::new(
      pty,
      state.clone(),
      events_tx.clone(),
      num_cols as u16,
      num_lines as u16,
      app_config.terminal.get_scrollback_lines(),
    );
    let tx = event_loop.channel();
    let _handle = event_loop.spawn();

    (tx, pty_info)
  };

  let backend = GhosttyBackend::new(state);
  let terminal = Terminal::new(
    Box::new(GhosttyPtySender(tx)),
    Box::new(backend),
    pty_info,
    None,
    None,
    Arc::new(AtomicU32::new(0)),
    Some(osc7_rx),
    Some(cwd_file),
  );

  Ok((terminal, events_rx))
}
