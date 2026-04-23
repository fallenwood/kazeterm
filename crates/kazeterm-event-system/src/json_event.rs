use std::path::PathBuf;

use serde::Deserialize;

use crate::AppEvent;

/// Configuration for the external event source.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum EventSourceConfig {
  /// No external event source (events can still be sent programmatically).
  #[default]
  None,
  /// Read events from stdin (JSON, one per line).
  Stdio,
  /// Read events from a Unix domain socket (all platforms).
  Socket { path: PathBuf },
}

/// JSON representation of an event for external input.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
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
  FocusNextPane,
  FocusPreviousPane,
  FocusPaneUp,
  FocusPaneDown,
  FocusPaneLeft,
  FocusPaneRight,
  SwapSplitPanes,
  ToggleSearch,
  ToggleFullscreen,
  ToggleTabBar,
  ShowAboutDialog,
  ShowImportAlacrittyDialog,
  ReloadConfig,
  FocusActiveTerminal,
  NewWindow,
  Quit,
  SendTextToTerminal {
    text: String,
  },
  Custom {
    name: String,
    data: String,
  },
  /// Dispatch a UIAction through the data-driven UI tree.
  /// The `action_json` field is the JSON-serialized `UIAction`.
  DispatchUIAction {
    action_json: String,
  },
  /// Request a snapshot of the current UI tree as JSON.
  SnapshotUITree,
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
      JsonEvent::FocusNextPane => AppEvent::FocusNextPane,
      JsonEvent::FocusPreviousPane => AppEvent::FocusPreviousPane,
      JsonEvent::FocusPaneUp => AppEvent::FocusPaneUp,
      JsonEvent::FocusPaneDown => AppEvent::FocusPaneDown,
      JsonEvent::FocusPaneLeft => AppEvent::FocusPaneLeft,
      JsonEvent::FocusPaneRight => AppEvent::FocusPaneRight,
      JsonEvent::SwapSplitPanes => AppEvent::SwapSplitPanes,
      JsonEvent::ToggleSearch => AppEvent::ToggleSearch,
      JsonEvent::ToggleFullscreen => AppEvent::ToggleFullscreen,
      JsonEvent::ToggleTabBar => AppEvent::ToggleTabBar,
      JsonEvent::ShowAboutDialog => AppEvent::ShowAboutDialog,
      JsonEvent::ShowImportAlacrittyDialog => AppEvent::ShowImportAlacrittyDialog,
      JsonEvent::ReloadConfig => AppEvent::ReloadConfig,
      JsonEvent::FocusActiveTerminal => AppEvent::FocusActiveTerminal,
      JsonEvent::NewWindow => AppEvent::NewWindow,
      JsonEvent::Quit => AppEvent::Quit,
      JsonEvent::SendTextToTerminal { text } => AppEvent::SendTextToTerminal { text },
      JsonEvent::Custom { name, data } => AppEvent::Custom { name, data },
      JsonEvent::DispatchUIAction { action_json } => {
        AppEvent::DispatchUIAction { action_json }
      }
      JsonEvent::SnapshotUITree => AppEvent::SnapshotUITree,
    }
  }
}
