use gpui::{Context, Entity, Window};
use kazeterm_ui_tree::action::UIAction;

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
    if !self.reconciling_ui_tree {
      let Some(window_id) = self.sync_ui_tree_and_window_id(cx) else {
        return;
      };
      self.dispatch_default_ui_action(
        UIAction::ToggleSearch { window_id },
        "toggle search",
        window,
        cx,
      );
      return;
    }

    self.search_visible = !self.search_visible;
    if self.search_visible {
      let font_size = cx.global::<::config::Config>().font.size;
      if let Some(terminal) = self.active_terminal() {
        self.search_bar.update(cx, |search_bar, _cx| {
          search_bar.set_terminal_view(terminal);
          search_bar.set_font_size(font_size);
          search_bar.reset_position();
        });
      }

      // Save open state to active tab
      if let Some(ix) = self.active_tab_ix {
        if ix < self.items.len() {
          self.items[ix].search_bar_state.visible = true;
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

      // Save closed state to active tab
      if let Some(ix) = self.active_tab_ix {
        if ix < self.items.len() {
          self.items[ix].search_bar_state.visible = false;
        }
      }

      // Focus back on terminal
      self.focus_active_terminal(window, cx);
    }

    cx.notify();
  }

  pub(crate) fn toggle_tab_bar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    if !self.reconciling_ui_tree {
      let Some(window_id) = self.sync_ui_tree_and_window_id(cx) else {
        return;
      };
      self.dispatch_default_ui_action(
        UIAction::ToggleTabBar { window_id },
        "toggle tab bar",
        window,
        cx,
      );
      return;
    }

    self.tab_bar_visible = !self.tab_bar_visible;
    cx.notify();
  }
}
