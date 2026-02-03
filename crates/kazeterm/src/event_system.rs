//! Application event system for Kazeterm
//!
//! This module provides a global event system that allows triggering actions
//! from any thread, including background threads. Events are dispatched to
//! the main window through GPUI's async runtime.
//!
//! # Example
//!
//! ```rust,ignore
//! use kazeterm::event_system::{AppEvent, send_event};
//!
//! // From any thread (including background threads):
//! send_event(AppEvent::NewTerminalWithDefaultProfile);
//! send_event(AppEvent::NewTerminalWithProfile { profile_name: "bash".to_string() });
//! ```

use std::sync::OnceLock;

use gpui::{AnyWindowHandle, App, AppContext, AsyncApp, WeakEntity, Window};
use smol::channel::{Receiver, Sender, unbounded};

use crate::components::MainWindow;

/// Global event sender - can be accessed from any thread
static EVENT_SENDER: OnceLock<Sender<AppEvent>> = OnceLock::new();

/// Stored main window reference for event dispatch
static MAIN_WINDOW_HANDLE: OnceLock<AnyWindowHandle> = OnceLock::new();

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
/// * `main_window` - A weak reference to the main window entity
/// * `window_handle` - The window handle for the main window
/// * `cx` - The GPUI application context
pub fn start_event_system(
  main_window: WeakEntity<MainWindow>,
  window_handle: AnyWindowHandle,
  cx: &mut App,
) {
  let (sender, receiver) = unbounded::<AppEvent>();

  // Store the sender globally so it can be accessed from any thread
  if EVENT_SENDER.set(sender).is_err() {
    tracing::warn!("Event system already initialized");
    return;
  }

  // Store the window handle
  let _ = MAIN_WINDOW_HANDLE.set(window_handle);

  tracing::info!("Event system initialized");

  // Spawn the event loop
  cx.spawn(async move |cx: &mut AsyncApp| {
    run_event_loop(main_window, window_handle, receiver, cx).await;
  })
  .detach();
}

/// Run the event loop, reading events and dispatching them to the main window.
async fn run_event_loop(
  main_window: WeakEntity<MainWindow>,
  window_handle: AnyWindowHandle,
  receiver: Receiver<AppEvent>,
  cx: &mut AsyncApp,
) {
  tracing::debug!("Event loop started");

  loop {
    match receiver.recv().await {
      Ok(event) => {
        tracing::debug!("Received event: {:?}", event);

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

  tracing::debug!("Event loop exited");
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
}
