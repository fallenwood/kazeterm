use std::path::{Path, PathBuf};

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

fn working_directory_title(cwd: &str) -> Option<String> {
  let path = Path::new(cwd);
  path
    .file_name()
    .and_then(|name| name.to_str())
    .map(ToOwned::to_owned)
    .or_else(|| {
      let display = path.to_string_lossy();
      (!display.is_empty()).then(|| display.to_string())
    })
}

fn decorate_auto_tab_title(
  title: String,
  prompt_identity: Option<&str>,
  include_identity: bool,
) -> String {
  let prompt_identity = prompt_identity
    .map(str::trim)
    .filter(|identity| !identity.is_empty());
  if include_identity && let Some(prompt_identity) = prompt_identity {
    return format!("{title} - {prompt_identity}");
  }

  title
}

fn resolve_auto_tab_title(
  shell_name: &str,
  terminal_title: &str,
  process_name: Option<&str>,
  working_directory: Option<&str>,
  prompt_identity: Option<&str>,
) -> String {
  let normalized_shell = shell_name_for(shell_name);
  let normalized_process = process_name.map(shell_name_for);
  let normalized_title = (!terminal_title.is_empty()).then(|| shell_name_for(terminal_title));
  let cwd_title = working_directory.and_then(working_directory_title);

  let title_is_shell_derived = terminal_title.is_empty()
    || normalized_title.as_deref() == Some(normalized_shell.as_str())
    || normalized_title == normalized_process;
  let process_is_shell = normalized_process
    .as_deref()
    .is_none_or(|process_name| process_name == normalized_shell.as_str());

  if !process_is_shell {
    let primary_title = if !terminal_title.is_empty() && !title_is_shell_derived {
      terminal_title.to_string()
    } else if let Some(process_name) = process_name.filter(|name| !name.is_empty()) {
      process_name.to_string()
    } else {
      cwd_title.unwrap_or_else(|| shell_name.to_string())
    };

    return decorate_auto_tab_title(primary_title, prompt_identity, true);
  }

  if title_is_shell_derived && let Some(cwd_title) = cwd_title {
    return decorate_auto_tab_title(cwd_title, prompt_identity, true);
  }

  if !terminal_title.is_empty() {
    return decorate_auto_tab_title(terminal_title.to_string(), prompt_identity, false);
  }

  decorate_auto_tab_title(shell_name.to_string(), prompt_identity, true)
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
    let working_directory = get_working_directory_pathbuf(
      working_directory.or_else(|| profile.working_directory.clone()),
    );

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

pub(crate) fn tab_index_for_shortcut(total_tabs: usize, shortcut_number: usize) -> Option<usize> {
  match shortcut_number {
    1..=8 => {
      let index = shortcut_number - 1;
      (index < total_tabs).then_some(index)
    }
    9 => total_tabs.checked_sub(1),
    _ => None,
  }
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
    terminal.update(cx, |terminal_view, cx| {
      terminal_view.activate_cursor_blinking(window, cx);
    });
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
        if let Some(tab_pos) = this.items.iter().position(|item| {
          item
            .split_container
            .all_terminals()
            .iter()
            .any(|(_, t)| t.read(cx).index == tab_index)
        }) {
          // Skip update if user has set a custom title
          if this.items[tab_pos].custom_title.is_some() {
            return;
          }
          let shell_name = this.items[tab_pos]._shell_name.clone();
          let terminal = terminal_view.read(cx).terminal().clone();
          let (terminal_title, process_name, working_directory, prompt_identity) =
            terminal.update(cx, |term, _cx| {
              let working_directory = term.current_working_directory();
              (
                term.title_text.clone(),
                term.pty_info.current.as_ref().map(|info| info.name.clone()),
                working_directory,
                term.prompt_identity(),
              )
            });
          let new_title = resolve_auto_tab_title(
            &shell_name,
            &terminal_title,
            process_name.as_deref(),
            working_directory.as_deref(),
            prompt_identity.as_deref(),
          );
          if this.items[tab_pos].title != new_title {
            this.items[tab_pos].title = new_title;
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
          cx.quit();
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

  pub(crate) fn select_tab_by_shortcut(
    &mut self,
    shortcut_number: usize,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    if let Some(ix) = tab_index_for_shortcut(self.items.len(), shortcut_number) {
      self.set_active_tab(ix, window, cx);
    }
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

  use super::{
    resolve_auto_tab_title, resolve_tab_launch, tab_index_for_shortcut, working_directory_title,
  };

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

  #[test]
  fn tab_shortcut_indexes_match_requested_tab_for_1_through_8() {
    assert_eq!(tab_index_for_shortcut(8, 1), Some(0));
    assert_eq!(tab_index_for_shortcut(8, 4), Some(3));
    assert_eq!(tab_index_for_shortcut(8, 8), Some(7));
    assert_eq!(tab_index_for_shortcut(3, 4), None);
  }

  #[test]
  fn tab_shortcut_9_selects_last_tab() {
    assert_eq!(tab_index_for_shortcut(0, 9), None);
    assert_eq!(tab_index_for_shortcut(1, 9), Some(0));
    assert_eq!(tab_index_for_shortcut(5, 9), Some(4));
    assert_eq!(tab_index_for_shortcut(12, 9), Some(11));
  }

  #[test]
  fn working_directory_title_uses_leaf_name_when_available() {
    assert_eq!(
      working_directory_title("C:\\Users\\alice\\project"),
      Some("project".to_string())
    );
    assert_eq!(
      working_directory_title("/home/alice/project"),
      Some("project".to_string())
    );
  }

  #[test]
  fn working_directory_title_falls_back_to_root_path() {
    assert_eq!(working_directory_title("/"), Some("/".to_string()));
    assert_eq!(working_directory_title("C:\\"), Some("C:\\".to_string()));
  }

  #[test]
  fn auto_tab_title_prefers_cwd_for_shell_prompt() {
    let title = resolve_auto_tab_title(
      "pwsh",
      "pwsh",
      Some("pwsh"),
      Some("C:\\Users\\alice\\project"),
      Some("alice@workstation"),
    );

    assert_eq!(title, "project - alice@workstation");
  }

  #[test]
  fn auto_tab_title_keeps_running_process_title() {
    let title = resolve_auto_tab_title(
      "pwsh",
      "nvim",
      Some("nvim"),
      Some("C:\\Users\\alice\\project"),
      Some("alice@workstation"),
    );

    assert_eq!(title, "nvim - alice@workstation");
  }

  #[test]
  fn auto_tab_title_preserves_explicit_shell_title() {
    let title = resolve_auto_tab_title(
      "bash",
      "custom session name",
      Some("bash"),
      Some("/home/alice/project"),
      Some("alice@workstation"),
    );

    assert_eq!(title, "custom session name");
  }

  #[test]
  fn auto_tab_title_uses_explicit_process_title_with_identity() {
    let title = resolve_auto_tab_title(
      "pwsh",
      "file.rs - nvim",
      Some("nvim"),
      Some("C:\\Users\\alice\\project"),
      Some("alice@workstation"),
    );

    assert_eq!(title, "file.rs - nvim - alice@workstation");
  }
}
