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

  /// Focus the next pane in the active tab's split container
  FocusNextPane,

  /// Focus the previous pane in the active tab's split container
  FocusPreviousPane,

  /// Swap the two halves of the split containing the active pane
  SwapSplitPanes,

  /// Toggle search bar visibility
  ToggleSearch,

  /// Toggle tab bar visibility
  ToggleTabBar,

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

impl AppEvent {
  /// Returns a string discriminant used as the key for subscriber lookup.
  pub fn discriminant(&self) -> &'static str {
    match self {
      AppEvent::NewTerminalWithDefaultProfile => "NewTerminalWithDefaultProfile",
      AppEvent::NewTerminalWithProfile { .. } => "NewTerminalWithProfile",
      AppEvent::CloseActiveTab => "CloseActiveTab",
      AppEvent::CloseTab { .. } => "CloseTab",
      AppEvent::NextTab => "NextTab",
      AppEvent::PreviousTab => "PreviousTab",
      AppEvent::SwitchToTab { .. } => "SwitchToTab",
      AppEvent::SplitHorizontal => "SplitHorizontal",
      AppEvent::SplitVertical => "SplitVertical",
      AppEvent::CloseActivePane => "CloseActivePane",
      AppEvent::FocusNextPane => "FocusNextPane",
      AppEvent::FocusPreviousPane => "FocusPreviousPane",
      AppEvent::SwapSplitPanes => "SwapSplitPanes",
      AppEvent::ToggleSearch => "ToggleSearch",
      AppEvent::ToggleTabBar => "ToggleTabBar",
      AppEvent::ShowAboutDialog => "ShowAboutDialog",
      AppEvent::ReloadConfig => "ReloadConfig",
      AppEvent::FocusActiveTerminal => "FocusActiveTerminal",
      AppEvent::SendTextToTerminal { .. } => "SendTextToTerminal",
      AppEvent::Custom { .. } => "Custom",
    }
  }
}
