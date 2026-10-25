//! Linux-only VTE backend; other targets build an empty crate.
#![cfg(target_os = "linux")]

pub mod vte_event_loop;
pub mod vte_term;

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicU32;

use futures::channel::mpsc::{UnboundedReceiver, unbounded};
use parking_lot::Mutex;
use terminal_kernel::program_status::ProgramStatusTracker;
use terminal_kernel::{
  PtyProcessInfo, PtySender, SessionEvents, SessionOptions, TerminalSession, event::WindowSize, tty,
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
  let mut env = HashMap::new();
  if std::env::var("LANG").is_err() {
    env
      .entry("LANG".to_string())
      .or_insert_with(|| "en_US.UTF-8".to_string());
  }

  env.insert("TERM_PROGRAM".to_string(), "kazeterm".to_string());
  env.insert(
    "TERM_PROGRAM_VERSION".to_string(),
    term_program_version.to_string(),
  );
  env.insert("TERM".to_string(), "xterm-256color".to_string());
  env.insert("COLORTERM".to_string(), "truecolor".to_string());
  env.insert("VTE_VERSION".to_string(), "7600".to_string());

  for (key, value) in &options.env {
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
    r#"printf '\e]133;A\e\\\e]7;file://%s%s\e\\' "$(hostname)" "$PWD"; printf '%s' "$PWD" >| "{}""#,
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
      r#"[Console]::Write("${{esc}}]133;A${{esc}}\"); "#,
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
