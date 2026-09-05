use std::sync::{
  Arc,
  atomic::{AtomicBool, Ordering},
};

use gpui::*;
use gpui_kit::component::{h_flex, label::Label};
use themeing::SettingsStore;

use super::main_window::MainWindow;
use super::shell_icon::ShellIcon;

/// Represents a tab being dragged
#[derive(Clone)]
pub struct DraggedTab {
  /// Stable identifier of the tab in its source window.
  pub tab_index: usize,
  /// Title of the tab being dragged
  pub title: String,
  /// Shell path for the icon
  pub shell_path: String,
  pub(crate) source: WeakEntity<MainWindow>,
  pub(crate) source_entity_id: EntityId,
  pub(crate) source_window: AnyWindowHandle,
  handled: Arc<AtomicBool>,
}

impl DraggedTab {
  pub(crate) fn new(
    tab_index: usize,
    title: String,
    shell_path: String,
    source: WeakEntity<MainWindow>,
    source_entity_id: EntityId,
    source_window: AnyWindowHandle,
  ) -> Self {
    Self {
      tab_index,
      title,
      shell_path,
      source,
      source_entity_id,
      source_window,
      handled: Arc::new(AtomicBool::new(false)),
    }
  }

  pub(crate) fn claim(&self) -> bool {
    self
      .handled
      .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
      .is_ok()
  }
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
