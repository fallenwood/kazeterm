use std::path::PathBuf;

use gpui::{Context, Focusable, Window};
use terminal::TerminalView;

use super::main_window::MainWindow;
use super::main_window_tab_item::TabItem;
use super::notifications::NotificationReason;
use crate::components::search_bar::SearchBarState;
use crate::components::split_pane::SplitContainer;

fn shell_name_for(program: &str) -> String {
  std::path::Path::new(program)
    .file_stem()
    .and_then(|name| name.to_str())
    .unwrap_or(program)
    .to_lowercase()
}

fn resolve_tab_launch(
  config: &::config::Config,
  profile_name: Option<&str>,
  working_directory: Option<String>,
) -> (String, Vec<String>, String, String, Option<PathBuf>) {
  if let Some(name) = profile_name {
    let profile = config.get_profile(name);

    let (program, args) = if let Some(profile) = profile {
      (profile.shell.clone(), profile.args.clone())
    } else {
      let ssh_hosts = ::config::get_ssh_hosts();
      if ssh_hosts.contains(&name.to_string()) {
        ("ssh".to_string(), vec![name.to_string()])
      } else {
        (config.get_shell(), vec![])
      }
    };

    let working_directory =
      working_directory.or(profile.and_then(|profile| profile.working_directory.clone()));
    let shell_name = shell_name_for(&program);

    return (
      program,
      args,
      name.to_string(),
      shell_name,
      get_working_directory_pathbuf(working_directory),
    );
  }

  if let Some(profile) = config.get_default_profile() {
    let shell = profile.shell.clone();
    let shell_name = shell_name_for(&shell);
    let working_directory =
      get_working_directory_pathbuf(working_directory.or_else(|| profile.working_directory.clone()));

    return (
      shell,
      profile.args.clone(),
      shell_name.clone(),
      shell_name,
      working_directory,
    );
  }

  let shell = config.get_shell();
  let shell_name = shell_name_for(&shell);

  (
    shell,
    vec![],
    shell_name.clone(),
    shell_name,
    get_working_directory_pathbuf(working_directory),
  )
}

impl MainWindow {
  pub fn insert_new_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    self.insert_new_tab_with_profile(None, None, window, cx);
  }

  /// Duplicates a tab by creating a new tab with the same shell and working directory
  pub fn duplicate_tab(&mut self, tab_index: usize, window: &mut Window, cx: &mut Context<Self>) {
    // Find the tab by index
    let tab = self.items.iter().find(|item| item.index == tab_index);
    if let Some(tab) = tab {
      let shell_path = tab.shell_path.clone();

      // Get the current working directory from the active terminal
      let working_directory = tab
        .split_container
        .get_active_terminal()
        .and_then(|terminal| Self::terminal_working_directory(&terminal, cx));

      // Create a new tab with the same shell and working directory
      self.insert_new_tab_with_profile(Some(&shell_path), working_directory, window, cx);
    }
  }

  pub fn insert_new_tab_with_profile(
    &mut self,
    profile_name: Option<&str>,
    working_directory: Option<String>,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    let this = self;
    let index = this
      .tab_index
      .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

    let config = cx.global::<::config::Config>();
    let (shell_program, shell_args, tab_title, shell_name, working_directory) =
      resolve_tab_launch(config, profile_name, working_directory);

    let terminal = match crate::components::terminal_window::new_terminal_window_with_shell(
      window,
      index,
      &shell_program,
      shell_args.clone(),
      working_directory,
      cx,
    ) {
      Ok(terminal) => terminal,
      Err(err) => {
        tracing::error!("Failed to start shell: {err}");
        this.show_shell_error_dialog(err, window, cx);
        return;
      }
    };
    let subscription = cx.subscribe_in(&terminal, window, Self::subscribe_terminal_view_event);

    let split_container = SplitContainer::new(terminal.clone());

    let item = TabItem {
      index,
      title: tab_title,
      custom_title: None,
      shell_path: shell_program,
      shell_args,
      _shell_name: shell_name,
      split_container,
      _subscription: subscription,
      search_bar_state: SearchBarState::default(),
    };
    this.items.push(item);

    // Use set_active_tab to properly save old tab's search state and focus the new tab
    let new_tab_ix = this.items.len() - 1;
    this.set_active_tab(new_tab_ix, window, cx);

    // Mark that we need to scroll tabs to the end after next render
    this.scroll_tabs_to_end = true;

    cx.notify();
  }

  pub(crate) fn active_terminal(&self) -> Option<gpui::Entity<TerminalView>> {
    self
      .active_tab_ix
      .and_then(|active_ix| self.items.get(active_ix))
      .and_then(|item| item.split_container.get_active_terminal())
  }

  pub(crate) fn active_tab_item_mut(&mut self) -> Option<&mut TabItem> {
    self
      .active_tab_ix
      .and_then(|active_ix| self.items.get_mut(active_ix))
  }

  pub(crate) fn focus_terminal(
    window: &mut Window,
    terminal: &gpui::Entity<TerminalView>,
    cx: &mut Context<Self>,
  ) {
    window.focus(&terminal.focus_handle(cx));
  }

  pub(crate) fn focus_active_terminal(&self, window: &mut Window, cx: &mut Context<Self>) {
    if let Some(terminal) = self.active_terminal() {
      Self::focus_terminal(window, &terminal, cx);
    }
  }

  pub(crate) fn active_terminal_working_directory(&self, cx: &mut Context<Self>) -> Option<String> {
    self
      .active_terminal()
      .and_then(|terminal| Self::terminal_working_directory(&terminal, cx))
  }

  pub(crate) fn terminal_working_directory(
    terminal: &gpui::Entity<TerminalView>,
    cx: &mut Context<Self>,
  ) -> Option<String> {
    // Use update() to get mutable access so we can force a fresh OS refresh.
    let terminal_entity = terminal.read(cx).terminal().clone();
    terminal_entity.update(cx, |term, _cx| term.current_working_directory())
  }

  pub(crate) fn subscribe_terminal_view_event(
    this: &mut MainWindow,
    terminal_view: &gpui::Entity<TerminalView>,
    event: &terminal::TerminalEvent,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    match event {
      terminal::TerminalEvent::CloseTerminal(terminal_index) => {
        // Find the tab containing this terminal
        let tab_position = this.items.iter().position(|item| {
          item
            .split_container
            .all_terminals()
            .iter()
            .any(|(_, t)| t.read(cx).index == *terminal_index)
        });

        if let Some(tab_pos) = tab_position {
          // Get the tab index before mutably borrowing
          let tab_index = this.items[tab_pos].index;

          // Try to close just the pane within the split container
          let should_close_tab = {
            let item = &mut this.items[tab_pos];
            !item
              .split_container
              .close_pane_by_terminal_index(*terminal_index, cx)
          };

          if should_close_tab {
            // This was the last pane, so close the entire tab
            this.remove_tab_by(tab_index, window, cx);
          } else {
            // Successfully closed a pane (but not the last one)
            // Focus the newly active terminal
            if let Some(terminal) = this.items[tab_pos].split_container.get_active_terminal() {
              Self::focus_terminal(window, &terminal, cx);
            }
            cx.notify();
          }
        }
      }
      terminal::TerminalEvent::Wakeup => {
        let has_bell = terminal_view.read(cx).has_bell();
        if has_bell {
          this.play_bell_sound();
          // Bell also serves as a supplementary notification trigger
          // (catches subtask completions in interactive programs like Copilot CLI).
          this.maybe_send_notification(&terminal_view, NotificationReason::Bell, cx);
        }
        cx.notify();
      }
      terminal::TerminalEvent::CommandFinished => {
        // Prompt returned: notify when a long-running command finishes.
        this.maybe_send_notification(&terminal_view, NotificationReason::CommandFinished, cx);
      }
      terminal::TerminalEvent::UpdateTab => {
        // Update tab title only if no custom title is set
        let tab_index = terminal_view.read(cx).index;
        if let Some(item) = this.items.iter_mut().find(|item| {
          item
            .split_container
            .all_terminals()
            .iter()
            .any(|(_, t)| t.read(cx).index == tab_index)
        }) {
          // Skip update if user has set a custom title
          if item.custom_title.is_some() {
            return;
          }
          let new_title = terminal_view
            .read(cx)
            .terminal()
            .read(cx)
            .title_text
            .clone();
          if item.title != new_title {
            item.title = new_title;
            cx.notify();
          }
        }
      }
    }
  }

  pub fn remove_tab_by(&mut self, tab_index: usize, window: &mut Window, cx: &mut Context<Self>) {
    // Find the position of the tab to remove
    let removed_pos = self.items.iter().position(|item| item.index == tab_index);

    if let Some(pos) = removed_pos {
      self.items.remove(pos);

      // If no tabs left, either close the window or insert a new tab
      if self.items.is_empty() {
        let config = cx.global::<::config::Config>();
        if config.tab.close_on_last {
          window.remove_window();
        } else {
          self.insert_new_tab(window, cx);
        }
        return;
      }

      // Determine the new active tab index after removal
      let new_active_ix = if let Some(active_ix) = self.active_tab_ix {
        if active_ix == pos {
          // The active tab was removed; select the next tab (at same position), or the last if we removed the last tab
          pos.min(self.items.len() - 1)
        } else if active_ix > pos {
          // A tab before the active tab was removed; adjust the index
          active_ix - 1
        } else {
          // If active_ix < pos, no adjustment needed
          active_ix
        }
      } else {
        // No active tab was set, default to first
        0
      };

      // Set the active tab and focus it
      self.set_active_tab(new_active_ix, window, cx);
    }

    cx.notify();
  }

  pub(crate) fn move_tab_left(&mut self, tab_ix: usize, cx: &mut Context<Self>) {
    if tab_ix > 0 {
      self.items.swap(tab_ix, tab_ix - 1);
      self.active_tab_ix = Some(tab_ix - 1);
      cx.notify();
    }
  }

  pub(crate) fn move_tab_right(&mut self, tab_ix: usize, cx: &mut Context<Self>) {
    if tab_ix + 1 < self.items.len() {
      self.items.swap(tab_ix, tab_ix + 1);
      self.active_tab_ix = Some(tab_ix + 1);
      cx.notify();
    }
  }

  pub(crate) fn close_other_tabs(&mut self, keep_tab_index: usize, cx: &mut Context<Self>) {
    self.items.retain(|tab| tab.index == keep_tab_index);
    self.active_tab_ix = Some(0);
    cx.notify();
  }

  pub(crate) fn close_tabs_to_right(&mut self, tab_ix: usize, cx: &mut Context<Self>) {
    let right_ix = tab_ix + 1;
    if right_ix < self.items.len() {
      self.items.truncate(right_ix);
      self.active_tab_ix = Some(tab_ix);
      cx.notify();
    }
  }

  pub(crate) fn set_active_tab(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
    // Save current tab's search state before switching
    if let Some(old_ix) = self.active_tab_ix {
      if old_ix < self.items.len() {
        let saved = self.search_bar.read(cx).save_state(self.search_visible, cx);
        self.items[old_ix].search_bar_state = saved;
      }
    }

    self.active_tab_ix = Some(ix);

    // Restore new tab's search state
    let new_state = self.items[ix].search_bar_state.clone();
    self.search_visible = new_state.visible;

    if let Some(terminal) = self.active_terminal() {
      let terminal_clone = terminal.clone();
      self.search_bar.update(cx, |search_bar, cx| {
        search_bar.set_terminal_view(terminal_clone);
        search_bar.restore_state(&new_state, window, cx);
      });

      if !self.search_visible {
        Self::focus_terminal(window, &terminal, cx);
      } else {
        self.search_bar.update(cx, |search_bar, cx| {
          search_bar.focus(window, cx);
        });
      }
    }

    cx.notify();
  }
}

pub(crate) fn get_working_directory_pathbuf(working_directory: Option<String>) -> Option<PathBuf> {
  tracing::debug!(
    "get_working_directory_pathbuf: input={:?}",
    working_directory
  );
  if let Some(working_directory) = working_directory {
    let path = std::path::Path::new(&working_directory);
    if path.exists() && path.is_dir() {
      Some(path.to_path_buf())
    } else {
      tracing::debug!("path does not exist or is not a dir, falling back to $HOME");
      std::env::home_dir()
    }
  } else {
    std::env::home_dir()
  }
}

#[cfg(test)]
mod tests {
  use config::{Config, Profile, TerminalConfig};

  use super::resolve_tab_launch;

  #[test]
  fn resolve_profile_launch_uses_default_profile_args() {
    let config = Config {
      terminal: TerminalConfig {
        default_profile: Some("login-shell".to_string()),
        ..TerminalConfig::default()
      },
      profiles: vec![Profile {
        name: "login-shell".to_string(),
        shell: "bash".to_string(),
        args: vec!["--login".to_string(), "-i".to_string()],
        working_directory: None,
      }],
      ..Config::default()
    };

    let (shell, args, tab_title, shell_name, _) = resolve_tab_launch(&config, None, None);

    assert_eq!(shell, "bash");
    assert_eq!(args, vec!["--login", "-i"]);
    assert_eq!(tab_title, "bash");
    assert_eq!(shell_name, "bash");
  }

  #[test]
  fn resolve_profile_launch_uses_selected_profile_args() {
    let config = Config {
      profiles: vec![Profile {
        name: "login-shell".to_string(),
        shell: "bash".to_string(),
        args: vec!["--login".to_string()],
        working_directory: None,
      }],
      ..Config::default()
    };

    let (shell, args, tab_title, shell_name, _) =
      resolve_tab_launch(&config, Some("login-shell"), None);

    assert_eq!(shell, "bash");
    assert_eq!(args, vec!["--login"]);
    assert_eq!(tab_title, "login-shell");
    assert_eq!(shell_name, "bash");
  }
}
