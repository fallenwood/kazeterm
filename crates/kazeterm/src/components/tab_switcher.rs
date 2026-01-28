use gpui::*;
use gpui_component::{h_flex, label::Label, v_flex};
use themeing::SettingsStore;

use crate::components::shell_icon::ShellIcon;

pub struct TabSwitcherItem {
  pub index: usize,
  pub title: String,
  pub shell_path: String,
  pub is_selected: bool,
}

pub struct TabSwitcher {
  items: Vec<TabSwitcherItem>,
  selected_index: usize,
}

impl TabSwitcher {
  pub fn new(items: Vec<TabSwitcherItem>, selected_index: usize) -> Self {
    Self {
      items,
      selected_index,
    }
  }
}

impl Render for TabSwitcher {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let setting_store = cx.global::<SettingsStore>();
    let theme = setting_store.theme();
    let colors = theme.colors();

    div().absolute().top_1_2().left_1_2().child(
      v_flex()
        .gap_1()
        .p_4()
        .bg(colors.background)
        .border_1()
        .border_color(colors.border)
        .rounded_lg()
        .shadow_lg()
        .min_w(px(300.0))
        .max_h(px(400.0))
        .overflow_hidden()
        .children(
          self
            .items
            .iter()
            .enumerate()
            .map(|(ix, item)| {
              let is_selected = ix == self.selected_index;
              let shell_icon = ShellIcon::new(&item.shell_path);
              let bg_color = if is_selected {
                colors.tab_active_background
              } else {
                colors.tab_inactive_background
              };

              h_flex()
                .gap_2()
                .px_3()
                .py_2()
                .items_center()
                .bg(bg_color)
                .rounded_md()
                .child(shell_icon.into_element(px(16.0)))
                .child(Label::new(item.title.clone()).text_color(colors.text))
            })
            .collect::<Vec<_>>(),
        ),
    )
  }
}
