use gpui::{Context, Entity, Window};

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
      let font_size = cx.global::<::config::Config>().font_size;
      if let Some(terminal) = self.active_terminal() {
        self.search_bar.update(cx, |search_bar, _cx| {
          search_bar.set_terminal_view(terminal);
          search_bar.set_font_size(font_size);
          search_bar.reset_position();
        });
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
      self.focus_active_terminal(window, cx);
    }

    cx.notify();
  }

  pub(crate) fn toggle_tab_bar(&mut self, cx: &mut Context<Self>) {
    self.tab_bar_visible = !self.tab_bar_visible;
    cx.notify();
  }
}
