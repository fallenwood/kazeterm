use gpui::*;
use gpui_component::{h_flex, label::Label};
use themeing::SettingsStore;

use super::shell_icon::ShellIcon;

/// Represents a tab being dragged
#[derive(Clone)]
pub struct DraggedTab {
  /// Index of the tab in the items list
  pub from_ix: usize,
  /// Title of the tab being dragged
  pub title: String,
  /// Shell path for the icon
  pub shell_path: String,
}

/// Simple view to render the dragged tab appearance
pub struct DraggedTabView {
  title: String,
  shell_path: String,
}

impl DraggedTabView {
  pub fn new(title: String, shell_path: String) -> Self {
    Self { title, shell_path }
  }
}

impl Render for DraggedTabView {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let setting_store = cx.global::<SettingsStore>();
    let theme = setting_store.theme();
    let colors = theme.colors();
    let shell_icon = ShellIcon::new(&self.shell_path);

    h_flex()
      .gap_1p5()
      .pl_2p5()
      .pr_2()
      .py_1()
      .items_center()
      .bg(colors.tab_active_background)
      .border_1()
      .border_color(colors.text_accent)
      .rounded_t_md()
      .shadow_lg()
      .opacity(0.9)
      .child(shell_icon.into_element(px(14.0)))
      .child(
        Label::new(self.title.clone())
          .text_color(colors.text)
          .whitespace_nowrap(),
      )
  }
}
