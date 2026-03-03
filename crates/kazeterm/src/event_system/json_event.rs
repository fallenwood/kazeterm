use std::path::PathBuf;

use serde::Deserialize;

use super::AppEvent;

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
