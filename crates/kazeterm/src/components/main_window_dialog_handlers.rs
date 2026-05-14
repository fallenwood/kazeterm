use std::path::{Path, PathBuf};

use gpui::{AppContext, Context, Entity, PathPromptOptions, Window};

use super::main_window::MainWindow;
use crate::components::about_dialog::{AboutDialog, AboutDialogCloseEvent, AboutDialogEvent};
use crate::components::close_confirm_dialog::{CloseConfirmDialog, CloseConfirmEvent};
use crate::components::import_alacritty_dialog::{ImportAlacrittyDialog, ImportAlacrittyEvent};
use crate::components::shell_error_dialog::{ShellErrorCloseEvent, ShellErrorDialog};
use crate::components::tab_rename_dialog::{TabRenameDialog, TabRenameEvent};
use crate::components::update_confirm_dialog::{UpdateConfirmDialog, UpdateConfirmEvent};

const UI_TREE_JSON_FILENAME: &str = "kazeterm-ui-tree.json";
const UI_TREE_JSON_LOAD_PROMPT: &str = "Load UI Tree JSON";

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

    let restore_workspace = cx.global::<::config::Config>().window.restore_workspace;
    let dialog = cx.new(|cx| CloseConfirmDialog::new(restore_workspace, window, cx));
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
      CloseConfirmEvent::SaveAndClose => {
        // Save workspace state before closing
        self.sync_ui_tree(cx);
        self.ui_tree.save_workspace();
        // User confirmed, close the window and quit the app
        self.close_confirm_dialog = None;
        self._close_confirm_subscription = None;
        window.remove_window();
        cx.quit();
      }
      CloseConfirmEvent::Close => {
        // Close without saving workspace state
        self.close_confirm_dialog = None;
        self._close_confirm_subscription = None;
        window.remove_window();
        cx.quit();
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
    let event_subscription = cx.subscribe_in(&dialog, window, Self::on_about_dialog_action_event);

    self.about_dialog = Some(dialog);
    self._about_dialog_subscription = Some(subscription);
    self._about_dialog_event_subscription = Some(event_subscription);
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
    self._about_dialog_event_subscription = None;

    // Refocus the terminal
    self.refocus_active_terminal(window, cx);
    cx.notify();
  }

  pub(crate) fn on_about_dialog_action_event(
    &mut self,
    dialog: &Entity<AboutDialog>,
    event: &AboutDialogEvent,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    match event {
      AboutDialogEvent::CheckForUpdates => {
        let dialog = dialog.clone();
        let window_handle = window.window_handle();
        let auto_update = cx.global::<::config::Config>().auto_update.clone();

        cx.spawn(async move |this, cx| {
          let result =
            smol::unblock(move || crate::auto_update::prepare_manual_update(auto_update)).await;

          match result {
            Ok(Some(prepared_update)) => {
              let release_tag = prepared_update.release_tag().to_string();
              let _ = dialog.update(cx, |dialog, cx| {
                dialog.finish_update_check(
                  format!("Kazeterm {release_tag} is available. Waiting for confirmation."),
                  false,
                  cx,
                );
              });

              if let Err(error) =
                crate::auto_update::prompt_prepared_update(this, window_handle, prepared_update, cx)
                  .await
              {
                let _ = dialog.update(cx, |dialog, cx| {
                  dialog.finish_update_check(
                    format!("Failed to show update prompt: {error}"),
                    true,
                    cx,
                  );
                });
              }
            }
            Ok(None) => {
              let _ = dialog.update(cx, |dialog, cx| {
                dialog.finish_update_check(
                  "Kazeterm is already up to date.".to_string(),
                  false,
                  cx,
                );
              });
            }
            Err(error) => {
              let _ = dialog.update(cx, |dialog, cx| {
                dialog.finish_update_check(
                  format!("Failed to check for updates: {error}"),
                  true,
                  cx,
                );
              });
            }
          }
        })
        .detach();
      }
    }
  }

  pub(crate) fn show_update_confirm_dialog(
    &mut self,
    prepared_update: crate::auto_update::PreparedUpdate,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    let release_tag = prepared_update.release_tag().to_string();
    if self.update_confirm_dialog.is_some() {
      tracing::warn!("Discarding update '{release_tag}' because an update prompt is already open");
      if let Err(error) = prepared_update.discard() {
        tracing::warn!("Failed to discard duplicate update '{release_tag}': {error}");
      }
      return;
    }

    let dialog = cx.new(|cx| UpdateConfirmDialog::new(release_tag, window, cx));
    let subscription = cx.subscribe_in(&dialog, window, Self::on_update_confirm_event);

    self.pending_update = Some(prepared_update);
    self.update_confirm_dialog = Some(dialog);
    self._update_confirm_subscription = Some(subscription);
    cx.notify();
  }

  pub(crate) fn on_update_confirm_event(
    &mut self,
    _dialog: &Entity<UpdateConfirmDialog>,
    event: &UpdateConfirmEvent,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    self.update_confirm_dialog = None;
    self._update_confirm_subscription = None;

    match event {
      UpdateConfirmEvent::Confirm => {
        let Some(prepared_update) = self.pending_update.take() else {
          tracing::warn!("Update was confirmed but no prepared update was pending");
          self.refocus_active_terminal(window, cx);
          cx.notify();
          return;
        };
        let release_tag = prepared_update.release_tag().to_string();

        if let Some(about_dialog) = &self.about_dialog {
          about_dialog.update(cx, |dialog, cx| {
            dialog.finish_update_check(format!("Installing update {release_tag}..."), false, cx);
          });
        }

        if let Err(error) =
          crate::auto_update::apply_prepared_update_in_context(self, prepared_update, cx)
        {
          tracing::error!("Failed to apply confirmed update '{release_tag}': {error:#}");
          if let Some(about_dialog) = &self.about_dialog {
            about_dialog.update(cx, |dialog, cx| {
              dialog.finish_update_check(format!("Failed to apply update: {error}"), true, cx);
            });
          }
          self.refocus_active_terminal(window, cx);
          cx.notify();
        }
      }
      UpdateConfirmEvent::Cancel => {
        if let Some(prepared_update) = self.pending_update.take() {
          let release_tag = prepared_update.release_tag().to_string();
          if let Err(error) = prepared_update.discard() {
            tracing::warn!("Failed to discard canceled update '{release_tag}': {error}");
          }
          if let Some(about_dialog) = &self.about_dialog {
            about_dialog.update(cx, |dialog, cx| {
              dialog.finish_update_check(format!("Update {release_tag} canceled."), false, cx);
            });
          }
        }
        self.refocus_active_terminal(window, cx);
        cx.notify();
      }
    }
  }

  /// Show import Alacritty config dialog
  pub fn show_import_alacritty_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    if self.import_alacritty_dialog.is_some() {
      return;
    }

    let dialog = cx.new(|cx| ImportAlacrittyDialog::new(window, cx));
    let subscription = cx.subscribe_in(&dialog, window, Self::on_import_alacritty_event);

    dialog.update(cx, |dialog, cx| {
      dialog.focus(window, cx);
    });

    self.import_alacritty_dialog = Some(dialog);
    self._import_alacritty_subscription = Some(subscription);
    cx.notify();
  }

  pub fn prompt_dump_ui_tree_path(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    if self.ui_tree_json_prompt_pending {
      return;
    }

    self.ui_tree_json_prompt_pending = true;

    let directory = Self::ui_tree_json_prompt_directory();
    let prompt = cx.prompt_for_new_path(&directory, Some(UI_TREE_JSON_FILENAME));
    let this = cx.weak_entity();

    window
      .spawn(cx, async move |cx| {
        let result = match prompt.await {
          Ok(result) => result.map_err(|err| format!("Failed to open UI tree save dialog: {err}")),
          Err(err) => Err(format!("UI tree save dialog did not return a path: {err}")),
        };

        let _ = this.update_in(cx, |this, window, cx| {
          this.handle_dump_ui_tree_prompt_result(result, window, cx);
        });
      })
      .detach();
  }

  pub fn prompt_load_ui_tree_path(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    if self.ui_tree_json_prompt_pending {
      return;
    }

    self.ui_tree_json_prompt_pending = true;

    let prompt = cx.prompt_for_paths(PathPromptOptions {
      files: true,
      directories: false,
      multiple: false,
      prompt: Some(UI_TREE_JSON_LOAD_PROMPT.into()),
    });
    let this = cx.weak_entity();

    window
      .spawn(cx, async move |cx| {
        let result = match prompt.await {
          Ok(Ok(Some(paths))) => Ok(paths.into_iter().next()),
          Ok(Ok(None)) => Ok(None),
          Ok(Err(err)) => Err(format!("Failed to open UI tree file picker: {err}")),
          Err(err) => Err(format!("UI tree file picker did not return a path: {err}")),
        };

        let _ = this.update_in(cx, |this, window, cx| {
          this.handle_load_ui_tree_prompt_result(result, window, cx);
        });
      })
      .detach();
  }

  fn ui_tree_json_prompt_directory() -> PathBuf {
    ::config::Config::get_config_file_path()
      .and_then(|path| path.parent().map(Path::to_path_buf))
      .filter(|path| !path.as_os_str().is_empty())
      .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| std::env::temp_dir()))
  }

  pub(crate) fn on_import_alacritty_event(
    &mut self,
    _dialog: &Entity<ImportAlacrittyDialog>,
    event: &ImportAlacrittyEvent,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    if let ImportAlacrittyEvent::Import(path_str) = event {
      let path = std::path::Path::new(path_str);
      match config::alacritty_import::import_alacritty_config(path) {
        Ok(result) => {
          // Save theme if present
          if let Some(ref theme) = result.theme {
            match config::alacritty_import::save_imported_theme(theme) {
              Ok(dest) => tracing::info!("Saved imported theme to {}", dest.display()),
              Err(e) => tracing::error!("Failed to save imported theme: {e}"),
            }
          }

          // Apply to running config
          let config = cx.global_mut::<::config::Config>();
          config::alacritty_import::apply_import(config, result);

          // Persist to disk
          if let Some(config_path) = ::config::Config::get_config_file_path() {
            // Backup current config before overwriting
            let backup_path = config_path.with_extension("toml.bak");
            match std::fs::copy(&config_path, &backup_path) {
              Ok(_) => tracing::info!("Backed up config to {}", backup_path.display()),
              Err(e) => tracing::warn!("Failed to backup config before import: {e}"),
            }

            let cfg = cx.global::<::config::Config>().clone();
            let config_str = toml::to_string_pretty(&cfg).unwrap_or_default();
            let content = format!(
              "# Kazeterm Configuration\n# Generated automatically\n\n{}",
              config_str
            );
            if let Err(e) = std::fs::write(&config_path, content) {
              tracing::error!("Failed to save config after import: {e}");
            } else {
              tracing::info!("Saved imported config to {}", config_path.display());
            }
          }
        }
        Err(e) => {
          tracing::error!("Alacritty config import failed: {e}");
        }
      }
    }

    self.import_alacritty_dialog = None;
    self._import_alacritty_subscription = None;
    self.refocus_active_terminal(window, cx);
    cx.notify();
  }

  fn handle_dump_ui_tree_prompt_result(
    &mut self,
    result: Result<Option<PathBuf>, String>,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    self.ui_tree_json_prompt_pending = false;

    let result =
      result.and_then(|path| path.map_or(Ok(()), |path| self.dump_ui_tree_to_path(&path, cx)));

    self.finish_ui_tree_json_prompt(result, window, cx);
  }

  fn handle_load_ui_tree_prompt_result(
    &mut self,
    result: Result<Option<PathBuf>, String>,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    self.ui_tree_json_prompt_pending = false;

    let result = result.and_then(|path| {
      path.map_or(Ok(()), |path| {
        self.load_ui_tree_from_path(&path, window, cx)
      })
    });

    self.finish_ui_tree_json_prompt(result, window, cx);
  }

  fn finish_ui_tree_json_prompt(
    &mut self,
    result: Result<(), String>,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    match result {
      Ok(()) => {
        self.refocus_active_terminal(window, cx);
        cx.notify();
      }
      Err(err) => {
        tracing::error!("UI tree JSON operation failed: {err}");
        self.show_shell_error_dialog(err, window, cx);
      }
    }
  }

  /// Helper to refocus the active terminal after closing dialogs
  pub fn refocus_active_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    self.focus_active_terminal(window, cx);
  }

  /// Show shell error dialog
  pub(crate) fn show_shell_error_dialog(
    &mut self,
    error_message: String,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    let dialog = cx.new(|cx| ShellErrorDialog::new(error_message, window, cx));
    let subscription = cx.subscribe_in(&dialog, window, Self::on_shell_error_event);

    self.shell_error_dialog = Some(dialog);
    self._shell_error_subscription = Some(subscription);
    cx.notify();
  }

  pub(crate) fn on_shell_error_event(
    &mut self,
    _dialog: &Entity<ShellErrorDialog>,
    _event: &ShellErrorCloseEvent,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    self.shell_error_dialog = None;
    self._shell_error_subscription = None;
    self.refocus_active_terminal(window, cx);
    cx.notify();
  }
}
