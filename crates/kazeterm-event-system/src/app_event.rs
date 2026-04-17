/// Application events that can be triggered from any thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppEvent {
  /// Create a new terminal tab with the default profile.
  NewTerminalWithDefaultProfile,

  /// Create a new terminal tab with a specific profile.
  NewTerminalWithProfile {
    profile_name: String,
    working_directory: Option<String>,
  },

  /// Close the active tab.
  CloseActiveTab,

  /// Close a specific tab by its index.
  CloseTab { tab_index: usize },

  /// Switch to the next tab.
  NextTab,

  /// Switch to the previous tab.
  PreviousTab,

  /// Switch to a specific tab by position (0-indexed).
  SwitchToTab { position: usize },

  /// Split the active pane horizontally.
  SplitHorizontal,

  /// Split the active pane vertically.
  SplitVertical,

  /// Close the active pane (within a split).
  CloseActivePane,

  /// Focus the next pane in the active tab's split container.
  FocusNextPane,

  /// Focus the previous pane in the active tab's split container.
  FocusPreviousPane,

  /// Focus the pane above the active pane.
  FocusPaneUp,

  /// Focus the pane below the active pane.
  FocusPaneDown,

  /// Focus the pane to the left of the active pane.
  FocusPaneLeft,

  /// Focus the pane to the right of the active pane.
  FocusPaneRight,

  /// Swap the two halves of the split containing the active pane.
  SwapSplitPanes,

  /// Toggle search bar visibility.
  ToggleSearch,

  /// Toggle fullscreen mode for the active window.
  ToggleFullscreen,

  /// Toggle tab bar visibility.
  ToggleTabBar,

  /// Show the about dialog.
  ShowAboutDialog,

  /// Show the import Alacritty configuration dialog.
  ShowImportAlacrittyDialog,

  /// Reload configuration.
  ReloadConfig,

  /// Focus the active terminal.
  FocusActiveTerminal,

  /// Open a new Kazeterm window.
  NewWindow,

  /// Quit the application.
  Quit,

  /// Send text to the active terminal.
  SendTextToTerminal { text: String },

  /// Custom event with arbitrary data (for extensions).
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
      AppEvent::FocusPaneUp => "FocusPaneUp",
      AppEvent::FocusPaneDown => "FocusPaneDown",
      AppEvent::FocusPaneLeft => "FocusPaneLeft",
      AppEvent::FocusPaneRight => "FocusPaneRight",
      AppEvent::SwapSplitPanes => "SwapSplitPanes",
      AppEvent::ToggleSearch => "ToggleSearch",
      AppEvent::ToggleFullscreen => "ToggleFullscreen",
      AppEvent::ToggleTabBar => "ToggleTabBar",
      AppEvent::ShowAboutDialog => "ShowAboutDialog",
      AppEvent::ShowImportAlacrittyDialog => "ShowImportAlacrittyDialog",
      AppEvent::ReloadConfig => "ReloadConfig",
      AppEvent::FocusActiveTerminal => "FocusActiveTerminal",
      AppEvent::NewWindow => "NewWindow",
      AppEvent::Quit => "Quit",
      AppEvent::SendTextToTerminal { .. } => "SendTextToTerminal",
      AppEvent::Custom { .. } => "Custom",
    }
  }
}
