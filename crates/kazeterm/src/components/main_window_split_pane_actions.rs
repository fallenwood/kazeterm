use gpui::{Context, Window};

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
    if self.active_tab_item_mut().is_none() {
      return;
    }

    let working_directory = self.active_terminal_working_directory(cx);

    // Use the same shell as the source tab, not the default shell.
    let shell = self
      .active_tab_item_mut()
      .map(|item| item.shell_path.clone())
      .unwrap_or_else(|| cx.global::<::config::Config>().get_shell().clone());

    // Create a new terminal with the same shell
    let index = self
      .tab_index
      .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

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

    if let Some(item) = self.active_tab_item_mut() {
      // Split the active pane
      item
        .split_container
        .split_active_pane(direction, new_terminal.clone());
    }

    // Focus the new terminal
    Self::focus_terminal(window, &new_terminal, cx);
    cx.notify();
  }

  pub fn close_active_pane(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    let pane_closed = self
      .active_tab_item_mut()
      .map(|item| item.split_container.close_active_pane())
      .unwrap_or(false);

    if pane_closed {
      self.focus_active_terminal(window, cx);
      cx.notify();
    }
  }

  pub fn focus_next_pane(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    if let Some(item) = self.active_tab_item_mut() {
      if let Some(terminal) = item.split_container.focus_next_pane() {
        Self::focus_terminal(window, &terminal, cx);
        cx.notify();
      }
    }
  }

  pub fn focus_prev_pane(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    if let Some(item) = self.active_tab_item_mut() {
      if let Some(terminal) = item.split_container.focus_prev_pane() {
        Self::focus_terminal(window, &terminal, cx);
        cx.notify();
      }
    }
  }

  pub fn swap_split_panes(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
    if let Some(item) = self.active_tab_item_mut() {
      if item.split_container.swap_panes() {
        cx.notify();
      }
    }
  }
}
