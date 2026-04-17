//! Centralized event bus and external event readers for Kazeterm.
//!
//! This crate provides the reusable event-system plumbing used by the app:
//! [`AppEvent`], [`JsonEvent`], [`EventSourceConfig`], a generic [`EventBus`],
//! external stdin/socket readers, and a GPUI-based dispatch loop that forwards
//! events onto a target entity on the main thread.
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
//! {"event": "NewWindow"}
//! {"event": "SendTextToTerminal", "text": "echo hello\n"}
//! {"event": "ToggleFullscreen"}
//! {"event": "SwitchToTab", "position": 0}
//! ```
//!
//! # Programmatic Usage
//!
//! ```rust,ignore
//! use kazeterm_event_system::{AppEvent, send_event};
//!
//! send_event(AppEvent::NewTerminalWithDefaultProfile);
//! ```

mod app_event;
mod event_bus;
mod event_sources;
mod json_event;

pub use app_event::AppEvent;
pub use event_bus::EventBus;
pub use json_event::{EventSourceConfig, JsonEvent};

use std::sync::OnceLock;

use gpui::{AnyWindowHandle, App, AppContext, AsyncApp, WeakEntity};
use smol::channel::{Receiver, Sender, unbounded};

use crate::event_sources::{start_socket_reader, start_stdio_reader};

/// Global event sender that can be accessed from any thread.
static EVENT_SENDER: OnceLock<Sender<AppEvent>> = OnceLock::new();

/// Send an event to the application from any thread.
///
/// This function is thread-safe and can be called from background threads.
/// Events are processed asynchronously on the main thread.
///
/// # Returns
///
/// Returns `true` if the event was sent successfully, `false` if the
/// event system is not initialized or the channel is closed.
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
/// This should be called once during application startup, after the target
/// entity is created. The event loop runs in the background and dispatches
/// events to the target via the provided [`EventBus`].
pub fn start_event_system<T: 'static>(
  target: WeakEntity<T>,
  window_handle: AnyWindowHandle,
  source_config: EventSourceConfig,
  event_bus: EventBus<T>,
  cx: &mut App,
) {
  let (sender, receiver) = unbounded::<AppEvent>();

  if EVENT_SENDER.set(sender.clone()).is_err() {
    tracing::warn!("Event system already initialized");
    return;
  }

  tracing::info!("Event system initialized with source: {:?}", source_config);

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

  cx.spawn(async move |cx: &mut AsyncApp| {
    run_event_loop(target, window_handle, receiver, event_bus, cx).await;
  })
  .detach();
}

async fn run_event_loop<T: 'static>(
  target: WeakEntity<T>,
  window_handle: AnyWindowHandle,
  receiver: Receiver<AppEvent>,
  event_bus: EventBus<T>,
  cx: &mut AsyncApp,
) {
  loop {
    match receiver.recv().await {
      Ok(event) => {
        if let Err(error) = dispatch_event(&target, window_handle, event, &event_bus, cx).await {
          tracing::error!("Failed to dispatch event: {}", error);
          break;
        }
      }
      Err(error) => {
        tracing::error!("Event channel closed: {}", error);
        break;
      }
    }
  }
}

async fn dispatch_event<T: 'static>(
  target: &WeakEntity<T>,
  window_handle: AnyWindowHandle,
  event: AppEvent,
  event_bus: &EventBus<T>,
  cx: &mut AsyncApp,
) -> anyhow::Result<()> {
  let target = target
    .upgrade()
    .ok_or_else(|| anyhow::anyhow!("Event target has been dropped"))?;

  cx.update_window(window_handle, |_root_view, window, cx| {
    target.update(cx, |this, cx| {
      event_bus.dispatch(this, event, window, cx);
    });
  })?;

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::{AppEvent, JsonEvent};

  #[test]
  fn event_debug_format() {
    let event = AppEvent::NewTerminalWithDefaultProfile;
    assert!(format!("{:?}", event).contains("NewTerminalWithDefaultProfile"));

    let event = AppEvent::NewTerminalWithProfile {
      profile_name: "bash".to_string(),
      working_directory: Some("/home".to_string()),
    };
    assert!(format!("{:?}", event).contains("bash"));
  }

  #[test]
  fn json_event_parsing() {
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

    let json = r#"{"event": "NewWindow"}"#;
    let event: JsonEvent = serde_json::from_str(json).unwrap();
    assert!(matches!(event, JsonEvent::NewWindow));

    let json = r#"{"event": "FocusPaneRight"}"#;
    let event: JsonEvent = serde_json::from_str(json).unwrap();
    assert!(matches!(event, JsonEvent::FocusPaneRight));

    let json = r#"{"event": "SendTextToTerminal", "text": "echo hello\n"}"#;
    let event: JsonEvent = serde_json::from_str(json).unwrap();
    assert!(matches!(
      event,
      JsonEvent::SendTextToTerminal { text } if text == "echo hello\n"
    ));
  }

  #[test]
  fn json_event_to_app_event() {
    let json_event = JsonEvent::NewTerminalWithDefaultProfile;
    let app_event: AppEvent = json_event.into();
    assert!(matches!(app_event, AppEvent::NewTerminalWithDefaultProfile));

    let json_event = JsonEvent::SwitchToTab { position: 3 };
    let app_event: AppEvent = json_event.into();
    assert!(matches!(app_event, AppEvent::SwitchToTab { position: 3 }));

    let json_event = JsonEvent::ToggleFullscreen;
    let app_event: AppEvent = json_event.into();
    assert!(matches!(app_event, AppEvent::ToggleFullscreen));
  }

  #[test]
  fn event_discriminant() {
    assert_eq!(
      AppEvent::NewTerminalWithDefaultProfile.discriminant(),
      "NewTerminalWithDefaultProfile"
    );
    assert_eq!(
      AppEvent::NewTerminalWithProfile {
        profile_name: "bash".into(),
        working_directory: None,
      }
      .discriminant(),
      "NewTerminalWithProfile"
    );
    assert_eq!(
      AppEvent::SwitchToTab { position: 0 }.discriminant(),
      "SwitchToTab"
    );
    assert_eq!(AppEvent::NewWindow.discriminant(), "NewWindow");
    assert_eq!(AppEvent::Quit.discriminant(), "Quit");
    assert_eq!(AppEvent::FocusPaneLeft.discriminant(), "FocusPaneLeft");
    assert_eq!(
      AppEvent::Custom {
        name: "x".into(),
        data: "y".into(),
      }
      .discriminant(),
      "Custom"
    );
  }
}
