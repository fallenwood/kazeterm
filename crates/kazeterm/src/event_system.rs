//! Application event system for Kazeterm
//!
//! This module provides an optional event system that allows triggering actions
//! from external sources. The event system can be configured via command-line
//! arguments to read events from:
//!
//! - **stdio**: Read JSON events from stdin (useful for piping commands)
//! - **socket**: Read JSON events from a Unix domain socket (all platforms)
//!
//! # Command-line Usage
//!
//! ```bash
//! # Enable event system reading from stdin
//! kazeterm --event-source stdio
//!
//! # Enable event system reading from a Unix domain socket
//! kazeterm --event-source socket --event-socket /tmp/kazeterm.sock
//!
//! # On Windows, use a file path for Unix domain socket
//! kazeterm --event-source socket --event-socket C:\Users\user\kazeterm.sock
//! ```
//!
//! # Event Format (JSON)
//!
//! Events are sent as JSON objects, one per line:
//!
//! ```json
//! {"event": "NewTerminalWithDefaultProfile"}
//! {"event": "NewTerminalWithProfile", "profile_name": "bash", "working_directory": "/home"}
//! {"event": "SendTextToTerminal", "text": "echo hello\n"}
//! {"event": "SwitchToTab", "position": 0}
//! ```
//!
//! # Programmatic Usage
//!
//! ```rust,ignore
//! use kazeterm::event_system::{AppEvent, send_event};
//!
//! // From any thread (including background threads):
//! send_event(AppEvent::NewTerminalWithDefaultProfile);
//! ```

use std::path::PathBuf;
use std::sync::OnceLock;

use gpui::{AnyWindowHandle, App, AppContext, AsyncApp, WeakEntity, Window};
use serde::Deserialize;
use smol::channel::{Receiver, Sender, unbounded};

use crate::components::MainWindow;

/// Global event sender - can be accessed from any thread
static EVENT_SENDER: OnceLock<Sender<AppEvent>> = OnceLock::new();

/// Stored main window reference for event dispatch
#[allow(dead_code)]
static MAIN_WINDOW_HANDLE: OnceLock<AnyWindowHandle> = OnceLock::new();

/// Configuration for the event source
#[derive(Debug, Clone)]
pub enum EventSourceConfig {
  /// No external event source (events can still be sent programmatically)
  None,
  /// Read events from stdin (JSON, one per line)
  Stdio,
  /// Read events from a Unix domain socket (all platforms)
  Socket { path: PathBuf },
}

impl Default for EventSourceConfig {
  fn default() -> Self {
    Self::None
  }
}

/// Application events that can be triggered from any thread
#[derive(Debug, Clone)]
pub enum AppEvent {
  /// Create a new terminal tab with the default profile
  NewTerminalWithDefaultProfile,

  /// Create a new terminal tab with a specific profile
  NewTerminalWithProfile {
    profile_name: String,
    working_directory: Option<String>,
  },

  /// Close the active tab
  CloseActiveTab,

  /// Close a specific tab by its index
  CloseTab { tab_index: usize },

  /// Switch to the next tab
  NextTab,

  /// Switch to the previous tab
  PreviousTab,

  /// Switch to a specific tab by position (0-indexed)
  SwitchToTab { position: usize },

  /// Split the active pane horizontally
  SplitHorizontal,

  /// Split the active pane vertically
  SplitVertical,

  /// Close the active pane (within a split)
  CloseActivePane,

  /// Toggle search bar visibility
  ToggleSearch,

  /// Show the about dialog
  ShowAboutDialog,

  /// Reload configuration
  ReloadConfig,

  /// Focus the active terminal
  FocusActiveTerminal,

  /// Send text to the active terminal
  SendTextToTerminal { text: String },

  /// Custom event with arbitrary data (for extensions)
  Custom { name: String, data: String },
}

/// JSON representation of an event for external input
#[derive(Debug, Deserialize)]
#[serde(tag = "event")]
pub enum JsonEvent {
  NewTerminalWithDefaultProfile,
  NewTerminalWithProfile {
    profile_name: String,
    working_directory: Option<String>,
  },
  CloseActiveTab,
  CloseTab {
    tab_index: usize,
  },
  NextTab,
  PreviousTab,
  SwitchToTab {
    position: usize,
  },
  SplitHorizontal,
  SplitVertical,
  CloseActivePane,
  ToggleSearch,
  ShowAboutDialog,
  ReloadConfig,
  FocusActiveTerminal,
  SendTextToTerminal {
    text: String,
  },
  Custom {
    name: String,
    data: String,
  },
}

impl From<JsonEvent> for AppEvent {
  fn from(json: JsonEvent) -> Self {
    match json {
      JsonEvent::NewTerminalWithDefaultProfile => AppEvent::NewTerminalWithDefaultProfile,
      JsonEvent::NewTerminalWithProfile {
        profile_name,
        working_directory,
      } => AppEvent::NewTerminalWithProfile {
        profile_name,
        working_directory,
      },
      JsonEvent::CloseActiveTab => AppEvent::CloseActiveTab,
      JsonEvent::CloseTab { tab_index } => AppEvent::CloseTab { tab_index },
      JsonEvent::NextTab => AppEvent::NextTab,
      JsonEvent::PreviousTab => AppEvent::PreviousTab,
      JsonEvent::SwitchToTab { position } => AppEvent::SwitchToTab { position },
      JsonEvent::SplitHorizontal => AppEvent::SplitHorizontal,
      JsonEvent::SplitVertical => AppEvent::SplitVertical,
      JsonEvent::CloseActivePane => AppEvent::CloseActivePane,
      JsonEvent::ToggleSearch => AppEvent::ToggleSearch,
      JsonEvent::ShowAboutDialog => AppEvent::ShowAboutDialog,
      JsonEvent::ReloadConfig => AppEvent::ReloadConfig,
      JsonEvent::FocusActiveTerminal => AppEvent::FocusActiveTerminal,
      JsonEvent::SendTextToTerminal { text } => AppEvent::SendTextToTerminal { text },
      JsonEvent::Custom { name, data } => AppEvent::Custom { name, data },
    }
  }
}

/// Send an event to the application from any thread.
///
/// This function is thread-safe and can be called from background threads.
/// Events are processed asynchronously on the main thread.
///
/// # Returns
///
/// Returns `true` if the event was sent successfully, `false` if the
/// event system is not initialized or the channel is closed.
///
/// # Example
///
/// ```rust,ignore
/// use kazeterm::event_system::{AppEvent, send_event};
///
/// // Create a new terminal with default profile
/// send_event(AppEvent::NewTerminalWithDefaultProfile);
///
/// // Create a terminal with a specific profile
/// send_event(AppEvent::NewTerminalWithProfile {
///     profile_name: "zsh".to_string(),
///     working_directory: Some("/home/user".to_string()),
/// });
/// ```
pub fn send_event(event: AppEvent) -> bool {
  if let Some(sender) = EVENT_SENDER.get() {
    sender.send_blocking(event).is_ok()
  } else {
    tracing::warn!("Event system not initialized, event dropped: {:?}", event);
    false
  }
}

/// Try to send an event without blocking.
///
/// This is useful when you need to send an event from a context where
/// blocking is not allowed.
pub fn try_send_event(event: AppEvent) -> bool {
  if let Some(sender) = EVENT_SENDER.get() {
    sender.try_send(event).is_ok()
  } else {
    tracing::warn!("Event system not initialized, event dropped: {:?}", event);
    false
  }
}

/// Initialize the event system and start the event loop.
///
/// This should be called once during application startup, after the
/// main window is created. The event loop runs in the background and
/// dispatches events to the main window.
///
/// # Arguments
///
/// * `main_window` - A weak reference to the main window entity
/// * `window_handle` - The window handle for the main window
/// * `source_config` - Configuration for the event source
/// * `cx` - The GPUI application context
pub fn start_event_system(
  main_window: WeakEntity<MainWindow>,
  window_handle: AnyWindowHandle,
  source_config: EventSourceConfig,
  cx: &mut App,
) {
  let (sender, receiver) = unbounded::<AppEvent>();

  // Store the sender globally so it can be accessed from any thread
  if EVENT_SENDER.set(sender.clone()).is_err() {
    tracing::warn!("Event system already initialized");
    return;
  }

  // Store the window handle
  let _ = MAIN_WINDOW_HANDLE.set(window_handle);

  tracing::info!("Event system initialized with source: {:?}", source_config);

  // Start the external event reader if configured
  match source_config {
    EventSourceConfig::None => {
      tracing::debug!("No external event source configured");
    }
    EventSourceConfig::Stdio => {
      start_stdio_reader(sender);
    }
    EventSourceConfig::Socket { path } => {
      start_socket_reader(sender, path);
    }
  }

  // Spawn the event dispatch loop
  cx.spawn(async move |cx: &mut AsyncApp| {
    run_event_loop(main_window, window_handle, receiver, cx).await;
  })
  .detach();
}

/// Start reading events from stdin in a background thread
fn start_stdio_reader(sender: Sender<AppEvent>) {
  std::thread::spawn(move || {
    use std::io::BufRead;

    tracing::info!("Starting stdin event reader");

    let stdin = std::io::stdin();
    let reader = stdin.lock();

    for line in reader.lines() {
      match line {
        Ok(line) => {
          let line = line.trim();
          if line.is_empty() {
            continue;
          }

          match serde_json::from_str::<JsonEvent>(line) {
            Ok(json_event) => {
              let event: AppEvent = json_event.into();
              tracing::debug!("Received event from stdin: {:?}", event);
              if sender.send_blocking(event).is_err() {
                tracing::error!("Event channel closed, stopping stdin reader");
                break;
              }
            }
            Err(e) => {
              tracing::warn!("Failed to parse event from stdin: {} - line: {}", e, line);
            }
          }
        }
        Err(e) => {
          tracing::error!("Error reading from stdin: {}", e);
          break;
        }
      }
    }

    tracing::info!("Stdin event reader stopped");
  });
}

/// Start reading events from a Unix domain socket in a background thread
fn start_socket_reader(sender: Sender<AppEvent>, path: PathBuf) {
  std::thread::spawn(move || {
    #[cfg(unix)]
    {
      start_unix_socket_reader_unix(sender, path);
    }

    #[cfg(windows)]
    {
      start_unix_socket_reader_windows(sender, path);
    }
  });
}

/// Unix domain socket reader (Unix platforms)
#[cfg(unix)]
fn start_unix_socket_reader_unix(sender: Sender<AppEvent>, path: PathBuf) {
  use std::io::{BufRead, BufReader};
  use std::os::unix::net::UnixListener;

  tracing::info!("Starting Unix socket event reader at: {:?}", path);

  // Remove existing socket file if it exists
  let _ = std::fs::remove_file(&path);

  let listener = match UnixListener::bind(&path) {
    Ok(l) => l,
    Err(e) => {
      tracing::error!("Failed to bind Unix socket at {:?}: {}", path, e);
      return;
    }
  };

  tracing::info!("Listening for events on Unix socket: {:?}", path);

  for stream in listener.incoming() {
    match stream {
      Ok(stream) => {
        let sender = sender.clone();
        std::thread::spawn(move || {
          let reader = BufReader::new(stream);
          for line in reader.lines() {
            match line {
              Ok(line) => {
                let line = line.trim();
                if line.is_empty() {
                  continue;
                }

                match serde_json::from_str::<JsonEvent>(line) {
                  Ok(json_event) => {
                    let event: AppEvent = json_event.into();
                    tracing::debug!("Received event from socket: {:?}", event);
                    if sender.send_blocking(event).is_err() {
                      tracing::error!("Event channel closed");
                      break;
                    }
                  }
                  Err(e) => {
                    tracing::warn!("Failed to parse event from socket: {} - line: {}", e, line);
                  }
                }
              }
              Err(e) => {
                tracing::debug!("Client disconnected: {}", e);
                break;
              }
            }
          }
        });
      }
      Err(e) => {
        tracing::error!("Failed to accept connection: {}", e);
      }
    }
  }
}

/// Unix domain socket reader (Windows)
///
/// Windows has supported Unix domain sockets since Windows 10 version 1803.
/// We use the uds_windows crate to provide UnixListener/UnixStream on Windows.
#[cfg(windows)]
fn start_unix_socket_reader_windows(sender: Sender<AppEvent>, path: PathBuf) {
  use std::io::{BufRead, BufReader};
  use uds_windows::UnixListener;

  tracing::info!("Starting Unix socket event reader at: {:?}", path);

  // Remove existing socket file if it exists
  let _ = std::fs::remove_file(&path);

  let listener = match UnixListener::bind(&path) {
    Ok(l) => l,
    Err(e) => {
      tracing::error!("Failed to bind Unix socket at {:?}: {}", path, e);
      return;
    }
  };

  tracing::info!("Listening for events on Unix socket: {:?}", path);

  for stream in listener.incoming() {
    match stream {
      Ok(stream) => {
        let sender = sender.clone();
        std::thread::spawn(move || {
          let reader = BufReader::new(stream);
          for line in reader.lines() {
            match line {
              Ok(line) => {
                let line = line.trim();
                if line.is_empty() {
                  continue;
                }

                match serde_json::from_str::<JsonEvent>(line) {
                  Ok(json_event) => {
                    let event: AppEvent = json_event.into();
                    tracing::debug!("Received event from socket: {:?}", event);
                    if sender.send_blocking(event).is_err() {
                      tracing::error!("Event channel closed");
                      break;
                    }
                  }
                  Err(e) => {
                    tracing::warn!("Failed to parse event from socket: {} - line: {}", e, line);
                  }
                }
              }
              Err(e) => {
                tracing::debug!("Client disconnected: {}", e);
                break;
              }
            }
          }
        });
      }
      Err(e) => {
        tracing::error!("Failed to accept connection: {}", e);
      }
    }
  }
}

/// Run the event loop, reading events and dispatching them to the main window.
async fn run_event_loop(
  main_window: WeakEntity<MainWindow>,
  window_handle: AnyWindowHandle,
  receiver: Receiver<AppEvent>,
  cx: &mut AsyncApp,
) {
  tracing::debug!("Event dispatch loop started");

  loop {
    match receiver.recv().await {
      Ok(event) => {
        tracing::debug!("Dispatching event: {:?}", event);

        if let Err(e) = dispatch_event(&main_window, window_handle, event, cx).await {
          tracing::error!("Failed to dispatch event: {}", e);
          // If the main window is gone, exit the event loop
          break;
        }
      }
      Err(e) => {
        tracing::error!("Event channel closed: {}", e);
        break;
      }
    }
  }

  tracing::debug!("Event dispatch loop exited");
}

/// Dispatch an event to the main window.
async fn dispatch_event(
  main_window: &WeakEntity<MainWindow>,
  window_handle: AnyWindowHandle,
  event: AppEvent,
  cx: &mut AsyncApp,
) -> anyhow::Result<()> {
  // Try to upgrade the weak reference
  let main_window = main_window
    .upgrade()
    .ok_or_else(|| anyhow::anyhow!("Main window has been dropped"))?;

  cx.update_window(window_handle, |_root_view, window, cx| {
    main_window.update(cx, |this, cx| {
      handle_event(this, event, window, cx);
    });
  })?;

  Ok(())
}

/// Handle an event on the main window.
fn handle_event(
  main_window: &mut MainWindow,
  event: AppEvent,
  window: &mut Window,
  cx: &mut gpui::Context<MainWindow>,
) {
  match event {
    AppEvent::NewTerminalWithDefaultProfile => {
      main_window.insert_new_tab(window, cx);
    }

    AppEvent::NewTerminalWithProfile {
      profile_name,
      working_directory,
    } => {
      main_window.insert_new_tab_with_profile(Some(&profile_name), working_directory, window, cx);
    }

    AppEvent::CloseActiveTab => {
      if let Some(active_ix) = main_window.active_tab_ix {
        if let Some(item) = main_window.items.get(active_ix) {
          let tab_index = item.index;
          main_window.remove_tab_by(tab_index, window, cx);
        }
      }
    }

    AppEvent::CloseTab { tab_index } => {
      main_window.remove_tab_by(tab_index, window, cx);
    }

    AppEvent::NextTab => {
      if !main_window.items.is_empty() {
        let current_ix = main_window.active_tab_ix.unwrap_or(0);
        let next_ix = (current_ix + 1) % main_window.items.len();
        main_window.set_active_tab(next_ix, window, cx);
      }
    }

    AppEvent::PreviousTab => {
      if !main_window.items.is_empty() {
        let current_ix = main_window.active_tab_ix.unwrap_or(0);
        let prev_ix = if current_ix == 0 {
          main_window.items.len() - 1
        } else {
          current_ix - 1
        };
        main_window.set_active_tab(prev_ix, window, cx);
      }
    }

    AppEvent::SwitchToTab { position } => {
      if position < main_window.items.len() {
        main_window.set_active_tab(position, window, cx);
      }
    }

    AppEvent::SplitHorizontal => {
      main_window.split_pane_horizontal(window, cx);
    }

    AppEvent::SplitVertical => {
      main_window.split_pane_vertical(window, cx);
    }

    AppEvent::CloseActivePane => {
      main_window.close_active_pane(window, cx);
    }

    AppEvent::ToggleSearch => {
      main_window.toggle_search(window, cx);
    }

    AppEvent::ShowAboutDialog => {
      main_window.show_about_dialog(window, cx);
    }

    AppEvent::ReloadConfig => {
      // Trigger config reload
      crate::config_watcher::reload_config_and_theme_from_event(cx);
    }

    AppEvent::FocusActiveTerminal => {
      main_window.refocus_active_terminal(window, cx);
    }

    AppEvent::SendTextToTerminal { text } => {
      // Send text to the active terminal
      if let Some(active_ix) = main_window.active_tab_ix {
        if let Some(item) = main_window.items.get(active_ix) {
          if let Some(terminal) = item.split_container.get_active_terminal() {
            terminal.update(cx, |view, cx| {
              view.terminal().update(cx, |term, _cx| {
                // Convert String to bytes for terminal input
                term.input(text.into_bytes());
              });
            });
          }
        }
      }
    }

    AppEvent::Custom { name, data } => {
      tracing::info!("Custom event received: {} = {}", name, data);
      // Custom events can be handled by extensions or plugins
      // For now, just log them
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_event_debug_format() {
    let event = AppEvent::NewTerminalWithDefaultProfile;
    assert!(format!("{:?}", event).contains("NewTerminalWithDefaultProfile"));

    let event = AppEvent::NewTerminalWithProfile {
      profile_name: "bash".to_string(),
      working_directory: Some("/home".to_string()),
    };
    assert!(format!("{:?}", event).contains("bash"));
  }

  #[test]
  fn test_json_event_parsing() {
    let json = r#"{"event": "NewTerminalWithDefaultProfile"}"#;
    let event: JsonEvent = serde_json::from_str(json).unwrap();
    assert!(matches!(event, JsonEvent::NewTerminalWithDefaultProfile));

    let json = r#"{"event": "NewTerminalWithProfile", "profile_name": "bash", "working_directory": "/home"}"#;
    let event: JsonEvent = serde_json::from_str(json).unwrap();
    assert!(matches!(
      event,
      JsonEvent::NewTerminalWithProfile {
        profile_name,
        working_directory: Some(_)
      } if profile_name == "bash"
    ));

    let json = r#"{"event": "SwitchToTab", "position": 2}"#;
    let event: JsonEvent = serde_json::from_str(json).unwrap();
    assert!(matches!(event, JsonEvent::SwitchToTab { position: 2 }));

    let json = r#"{"event": "SendTextToTerminal", "text": "echo hello\n"}"#;
    let event: JsonEvent = serde_json::from_str(json).unwrap();
    assert!(matches!(
      event,
      JsonEvent::SendTextToTerminal { text } if text == "echo hello\n"
    ));
  }

  #[test]
  fn test_json_event_to_app_event() {
    let json_event = JsonEvent::NewTerminalWithDefaultProfile;
    let app_event: AppEvent = json_event.into();
    assert!(matches!(app_event, AppEvent::NewTerminalWithDefaultProfile));

    let json_event = JsonEvent::SwitchToTab { position: 3 };
    let app_event: AppEvent = json_event.into();
    assert!(matches!(app_event, AppEvent::SwitchToTab { position: 3 }));
  }
}
