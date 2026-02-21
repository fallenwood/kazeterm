use gpui::{Context, Focusable, Window};

use super::main_window::MainWindow;
use super::main_window_tab_management::get_working_directory_pathbuf;
use crate::components::split_pane::SplitDirection;

impl MainWindow {
  pub fn split_pane_horizontal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    self.split_pane(SplitDirection::Horizontal, window, cx);
  }

  pub fn split_pane_vertical(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    self.split_pane(SplitDirection::Vertical, window, cx);
  }

  fn split_pane(&mut self, direction: SplitDirection, window: &mut Window, cx: &mut Context<Self>) {
    let working_directory = self.active_terminal_working_directory(cx);

    if let Some(active_tab_ix) = self.active_tab_ix {
      if let Some(item) = self.items.get_mut(active_tab_ix) {
        // Create a new terminal with the same shell
        let index = self
          .tab_index
          .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let config = cx.global::<::config::Config>();
        let shell = config.get_shell().clone();
        let working_directory_path = get_working_directory_pathbuf(working_directory);

        let new_terminal = crate::components::terminal_window::new_terminal_window_with_shell(
          window,
          index,
          &shell,
          vec![],
          working_directory_path,
          cx,
        );

        // Subscribe to the new terminal
        let subscription = cx.subscribe_in(&new_terminal, window, Self::subscribe_terminal_view_event);

        // Store subscription (we'll need to manage this better in production)
        // For now, we'll leak it as we don't have a good place to store per-pane subscriptions
        std::mem::forget(subscription);

        // Split the active pane
        item.split_container.split_active_pane(direction, new_terminal.clone());

        // Focus the new terminal
        window.focus(&new_terminal.focus_handle(cx));

        cx.notify();
      }
    }
  }

  pub fn close_active_pane(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    let mut pane_closed = false;

    if let Some(active_tab_ix) = self.active_tab_ix {
      if let Some(item) = self.items.get_mut(active_tab_ix) {
        pane_closed = item.split_container.close_active_pane();
      }
    }

    if pane_closed {
      self.focus_active_terminal(window, cx);
      cx.notify();
    }
  }
}
