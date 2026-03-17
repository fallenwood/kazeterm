use gpui::{AppContext, Context, Entity, Window};

use super::main_window::MainWindow;
use crate::components::about_dialog::{AboutDialog, AboutDialogCloseEvent};
use crate::components::close_confirm_dialog::{CloseConfirmDialog, CloseConfirmEvent};
use crate::components::session_restore_error_dialog::{
  SessionRestoreErrorDialog, SessionRestoreErrorEvent,
};
use crate::components::tab_rename_dialog::{TabRenameDialog, TabRenameEvent};
use crate::session_state::{SessionState, TabState};

impl MainWindow {
  /// Show rename dialog for a tab
  pub(crate) fn show_rename_dialog(
    &mut self,
    tab_index: usize,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    // Find the tab's current display title
    let current_title = self
      .items
      .iter()
      .find(|item| item.index == tab_index)
      .map(|item| item.display_title().to_string())
      .unwrap_or_default();

    let dialog = cx.new(|cx| TabRenameDialog::new(tab_index, &current_title, window, cx));

    let subscription = cx.subscribe_in(&dialog, window, Self::on_rename_dialog_event);

    // Focus the dialog
    dialog.update(cx, |dialog: &mut TabRenameDialog, cx| {
      dialog.focus(window, cx);
    });

    self.rename_dialog = Some(dialog);
    self._rename_dialog_subscription = Some(subscription);
    cx.notify();
  }

  pub(crate) fn on_rename_dialog_event(
    &mut self,
    _dialog: &Entity<TabRenameDialog>,
    event: &TabRenameEvent,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    let tab_index = event.tab_index;
    let new_title = event.new_title.clone();

    // Find the tab and update its custom_title
    if let Some(item) = self.items.iter_mut().find(|item| item.index == tab_index) {
      item.custom_title = new_title;
    }

    // Close the dialog
    self.rename_dialog = None;
    self._rename_dialog_subscription = None;

    // Refocus the terminal
    self.refocus_active_terminal(window, cx);
    cx.notify();
  }

  /// Show close confirmation dialog
  pub fn show_close_confirm_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    // Don't show if already visible
    if self.close_confirm_dialog.is_some() {
      return;
    }

    let dialog = cx.new(|cx| CloseConfirmDialog::new(window, cx));
    let subscription = cx.subscribe_in(&dialog, window, Self::on_close_confirm_event);

    self.close_confirm_dialog = Some(dialog);
    self._close_confirm_subscription = Some(subscription);
    cx.notify();
  }

  pub(crate) fn on_close_confirm_event(
    &mut self,
    _dialog: &Entity<CloseConfirmDialog>,
    event: &CloseConfirmEvent,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    match event {
      CloseConfirmEvent::Confirm => {
        // Save session before closing (if restore is enabled)
        self.save_session(cx);
        self.close_confirm_dialog = None;
        self._close_confirm_subscription = None;
        window.remove_window();
      }
      CloseConfirmEvent::Cancel => {
        // User cancelled, just close the dialog
        self.close_confirm_dialog = None;
        self._close_confirm_subscription = None;

        // Refocus the terminal
        self.refocus_active_terminal(window, cx);
        cx.notify();
      }
    }
  }

  /// Check if close confirmation dialog is currently showing
  pub fn is_close_confirm_visible(&self) -> bool {
    self.close_confirm_dialog.is_some()
  }

  /// Show about dialog
  pub fn show_about_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    // Don't show if already visible
    if self.about_dialog.is_some() {
      return;
    }

    let dialog = cx.new(|cx| AboutDialog::new(window, cx));
    let subscription = cx.subscribe_in(&dialog, window, Self::on_about_dialog_event);

    self.about_dialog = Some(dialog);
    self._about_dialog_subscription = Some(subscription);
    cx.notify();
  }

  pub(crate) fn on_about_dialog_event(
    &mut self,
    _dialog: &Entity<AboutDialog>,
    _event: &AboutDialogCloseEvent,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    // Close the dialog
    self.about_dialog = None;
    self._about_dialog_subscription = None;

    // Refocus the terminal
    self.refocus_active_terminal(window, cx);
    cx.notify();
  }

  /// Helper to refocus the active terminal after closing dialogs
  pub fn refocus_active_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    self.focus_active_terminal(window, cx);
  }

  /// Save current session state to disk (if restore_session is enabled).
  pub fn save_session(&self, cx: &mut Context<Self>) {
    let config = cx.global::<::config::Config>();
    if !config.restore_session {
      return;
    }

    let tabs: Vec<TabState> = self
      .items
      .iter()
      .map(|item| {
        // Get working directory from the first terminal in the split container
        let working_directory = item
          .split_container
          .get_active_terminal()
          .and_then(|terminal| {
            let terminal_entity = terminal.read(cx).terminal().clone();
            terminal_entity.read(cx).current_working_directory_cached()
          });

        TabState {
          profile_name: Some(item.shell_path.clone()),
          shell_path: item.shell_path.clone(),
          working_directory,
          custom_title: item.custom_title.clone(),
        }
      })
      .collect();

    let active_tab_index = self.active_tab_ix.unwrap_or(0);

    let state = SessionState {
      tabs,
      active_tab_index,
    };

    if let Err(e) = state.save() {
      tracing::error!("Failed to save session state: {}", e);
    }
  }

  /// Show the session restore error dialog
  pub fn show_session_restore_error_dialog(
    &mut self,
    error_message: String,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    let dialog =
      cx.new(|cx| SessionRestoreErrorDialog::new(error_message, window, cx));
    let subscription =
      cx.subscribe_in(&dialog, window, Self::on_session_restore_error_event);

    self.session_restore_error_dialog = Some(dialog);
    self._session_restore_error_subscription = Some(subscription);
    cx.notify();
  }

  pub(crate) fn on_session_restore_error_event(
    &mut self,
    _dialog: &Entity<SessionRestoreErrorDialog>,
    event: &SessionRestoreErrorEvent,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    match event {
      SessionRestoreErrorEvent::StartNew => {
        self.session_restore_error_dialog = None;
        self._session_restore_error_subscription = None;

        // Create a fresh tab if there are none
        if self.items.is_empty() {
          self.insert_new_tab(window, cx);
        }

        self.refocus_active_terminal(window, cx);
        cx.notify();
      }
      SessionRestoreErrorEvent::Quit => {
        self.session_restore_error_dialog = None;
        self._session_restore_error_subscription = None;
        window.remove_window();
      }
    }
  }

  /// Check if session restore error dialog is currently showing
  pub fn is_session_restore_error_visible(&self) -> bool {
    self.session_restore_error_dialog.is_some()
  }
}
