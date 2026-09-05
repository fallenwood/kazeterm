use gpui::{AppContext, Context, Window};

use super::main_window::MainWindow;
use crate::components::tab_switcher::{TabSwitcher, TabSwitcherItem};

impl MainWindow {
  pub(crate) fn show_tab_switcher(
    &mut self,
    forward: bool,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    if self.items.len() <= 1 {
      return;
    }

    if !self.tab_switcher_visible {
      let current_ix = self.active_tab_ix.unwrap_or(0);
      self.tab_switcher_selection = if forward {
        (current_ix + 1) % self.items.len()
      } else if current_ix == 0 {
        self.items.len() - 1
      } else {
        current_ix - 1
      };
      self.tab_switcher_visible = true;
    } else {
      if forward {
        self.tab_switcher_selection = (self.tab_switcher_selection + 1) % self.items.len();
      } else {
        self.tab_switcher_selection = if self.tab_switcher_selection == 0 {
          self.items.len() - 1
        } else {
          self.tab_switcher_selection - 1
        };
      }
    }

    self.update_tab_switcher(cx);
    self.set_active_tab(self.tab_switcher_selection, window, cx);
    cx.notify();
  }

  pub(crate) fn hide_tab_switcher(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
    if self.tab_switcher_visible {
      self.tab_switcher_visible = false;
      self.tab_switcher = None;
      cx.notify();
    }
  }

  pub(crate) fn update_tab_switcher(&mut self, cx: &mut Context<Self>) {
    let items: Vec<TabSwitcherItem> = self
      .items
      .iter()
      .enumerate()
      .map(|(ix, item)| TabSwitcherItem {
        index: item.index,
        title: item.display_title().to_string(),
        shell_path: item.shell_path.clone(),
        is_selected: ix == self.tab_switcher_selection,
      })
      .collect();

    let tab_switcher = cx.new(|_cx| TabSwitcher::new(items, self.tab_switcher_selection));
    self.tab_switcher = Some(tab_switcher);
  }
}
