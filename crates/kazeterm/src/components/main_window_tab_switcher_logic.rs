use gpui::{AppContext, Context, Window};

use super::main_window::MainWindow;
use crate::components::tab_switcher::{TabSwitcher, TabSwitcherItem};

impl MainWindow {
  pub(crate) fn show_tab_switcher(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
    if self.items.len() <= 1 {
      return;
    }

    let was_visible = self.tab_switcher_visible;

    if !self.tab_switcher_visible {
      // First time showing the switcher - initialize selection
      let current_ix = self.active_tab_ix.unwrap_or(0);
      self.tab_switcher_selection = if forward {
        (current_ix + 1) % self.items.len()
      } else {
        if current_ix == 0 {
          self.items.len() - 1
        } else {
          current_ix - 1
        }
      };
      self.tab_switcher_visible = true;
    } else {
      // Switcher already visible - cycle selection
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

    // Switch to the selected tab immediately
    self.set_active_tab(self.tab_switcher_selection, window, cx);
    self.update_tab_switcher(cx);
    cx.notify();

    // Start polling if we just showed the switcher
    if !was_visible {
      self.start_ctrl_polling(cx);
    }
  }

  pub(crate) fn start_ctrl_polling(&self, cx: &mut Context<Self>) {
    // Poll to detect when Ctrl is released
    cx.spawn(async move |this_weak, cx| {
      loop {
        smol::Timer::after(std::time::Duration::from_millis(50)).await;

        // Check if Ctrl is still pressed (always true for now)
        let _ctrl_pressed = true; // Can't reliably check on Wayland without X11 libs

        let should_hide = cx
          .update(|_cx| {
            if let Some(this) = this_weak.upgrade() {
              let switcher_visible = this.read(_cx).tab_switcher_visible;
              switcher_visible
            } else {
              false
            }
          })
          .unwrap_or(false);

        if !should_hide {
          break;
        }
      }
    })
    .detach();
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
