use gpui::{Context, Entity, Focusable, Window};

use super::main_window::MainWindow;
use crate::components::search_bar::{SearchBar, SearchBarCloseEvent};

impl MainWindow {
  pub(crate) fn on_search_bar_event(
    &mut self,
    _search_bar: &Entity<SearchBar>,
    _event: &SearchBarCloseEvent,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    self.toggle_search(window, cx);
  }

  pub(crate) fn toggle_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    self.search_visible = !self.search_visible;
    if self.search_visible {
      if let Some(active_ix) = self.active_tab_ix {
        if let Some(item) = self.items.get(active_ix) {
          if let Some(terminal) = item.split_container.get_active_terminal() {
            self.search_bar.update(cx, |search_bar, _cx| {
              search_bar.set_terminal_view(terminal);
            });
          }
        }
      }

      // Focus on search bar input
      self.search_bar.update(cx, |search_bar, cx| {
        search_bar.focus(window, cx);
      });
    } else {
      self.search_bar.update(cx, |search_bar, cx| {
        search_bar.clear_search(cx);
      });

      // Focus back on terminal
      if let Some(active_ix) = self.active_tab_ix {
        if let Some(item) = self.items.get(active_ix) {
          if let Some(terminal) = item.split_container.get_active_terminal() {
            window.focus(&terminal.focus_handle(cx));
          }
        }
      }
    }

    cx.notify();
  }
}
