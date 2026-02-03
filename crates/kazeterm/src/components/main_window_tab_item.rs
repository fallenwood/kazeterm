use crate::components::split_pane::SplitContainer;

/// A single tab containing a terminal or split container
pub struct TabItem {
  pub(crate) index: usize,
  pub(crate) title: String,
  /// Custom title set by the user. When Some, auto-title updates are ignored.
  pub(crate) custom_title: Option<String>,
  pub(crate) shell_path: String,
  pub(crate) _shell_name: String,
  pub(crate) split_container: SplitContainer,
  pub(crate) _subscription: gpui::Subscription,
}

impl TabItem {
  /// Returns the display title (custom title if set, otherwise the auto-assigned title)
  pub fn display_title(&self) -> &str {
    self.custom_title.as_deref().unwrap_or(&self.title)
  }
}
