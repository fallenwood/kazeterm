use std::path::PathBuf;

use gpui::{Context, Focusable, Window};
use terminal::TerminalView;

use super::main_window::MainWindow;
use super::main_window_tab_item::TabItem;
use crate::components::split_pane::SplitContainer;

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
      if let Some(name) = profile_name {
        let profile = config.get_profile(name);

        let (program, args) = if let Some(p) = profile {
          (p.shell.clone(), p.args.clone())
        } else {
          let ssh_hosts = ::config::get_ssh_hosts();
          if ssh_hosts.contains(&name.to_string()) {
            ("ssh".to_string(), vec![name.to_string()])
          } else {
            (config.get_shell(), vec![])
          }
        };

        let working_directory =
          working_directory.or(profile.map(|e| e.working_directory.clone()).flatten());
        let shell_name = std::path::Path::new(&program)
          .file_stem()
          .and_then(|n| n.to_str())
          .unwrap_or(&program)
          .to_lowercase();
        let working_directory = get_working_directory_pathbuf(working_directory);

        (program, args, name.to_string(), shell_name, working_directory)
      } else {
        let shell = config.get_shell().clone();
        let shell_name = std::path::Path::new(&shell)
          .file_stem()
          .and_then(|n| n.to_str())
          .unwrap_or(&shell)
          .to_lowercase();
        let working_directory = get_working_directory_pathbuf(working_directory);

        (
          shell,
          vec![],
          shell_name.clone(),
          shell_name,
          working_directory,
        )
      };

    let terminal = crate::components::terminal_window::new_terminal_window_with_shell(
      window,
      index,
      &shell_program,
      shell_args,
      working_directory,
      cx,
    );
    let subscription = cx.subscribe_in(&terminal, window, Self::subscribe_terminal_view_event);

    let split_container = SplitContainer::new(terminal.clone());

    let item = TabItem {
      index,
      title: tab_title,
      custom_title: None,
      shell_path: shell_program,
      _shell_name: shell_name,
      split_container,
      _subscription: subscription,
    };
    this.items.push(item);
    this.active_tab_ix = Some(this.items.len() - 1);

    // Focus the terminal
    terminal.update(cx, |view, _cx| {
      window.focus(&view.focus_handle);
    });

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
    terminal
      .read(cx)
      .terminal()
      .read(cx)
      .pty_info
      .current
      .as_ref()
      .map(|info| info.cwd.to_string_lossy().to_string())
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
          item.split_container.all_terminals().iter().any(|(_, t)| t.read(cx).index == *terminal_index)
        });

        if let Some(tab_pos) = tab_position {
          // Get the tab index before mutably borrowing
          let tab_index = this.items[tab_pos].index;

          // Try to close just the pane within the split container
          let should_close_tab = {
            let item = &mut this.items[tab_pos];
            !item.split_container.close_pane_by_terminal_index(*terminal_index, cx)
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
        // Check if any terminal has bell and play sound
        let has_bell = terminal_view.read(cx).has_bell();
        if has_bell {
          this.play_bell_sound();
        }
        cx.notify();
      }
      terminal::TerminalEvent::UpdateTab => {
        // Update tab title only if no custom title is set
        let tab_index = terminal_view.read(cx).index;
        if let Some(item) = this.items.iter_mut().find(|item| {
          item.split_container.all_terminals().iter().any(|(_, t)| t.read(cx).index == tab_index)
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

  pub(crate) fn play_bell_sound(&self) {
    #[cfg(target_os = "windows")]
    {
      std::thread::spawn(|| {
        use windows::Win32::Media::Audio::{PlaySoundW, SND_ALIAS, SND_ASYNC};
        use windows::core::w;
        unsafe {
          // Play the Windows default notification sound (SystemAsterisk)
          let _ = PlaySoundW(w!("SystemAsterisk"), None, SND_ALIAS | SND_ASYNC);
        }
      });
    }
    #[cfg(not(target_os = "windows"))]
    {
      // TODO: not working
      print!("\x07");
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
        if config.close_on_last_tab {
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

  pub(crate) fn set_active_tab(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
    self.active_tab_ix = Some(ix);

    // Update search bar with the new active terminal
    if let Some(terminal) = self.active_terminal() {
      let terminal_clone = terminal.clone();
      self.search_bar.update(cx, |search_bar, _cx| {
        search_bar.set_terminal_view(terminal_clone.clone());
      });

      // Focus the terminal
      Self::focus_terminal(window, &terminal, cx);
    }

    cx.notify();
  }
}

pub(crate) fn get_working_directory_pathbuf(working_directory: Option<String>) -> Option<PathBuf> {
  if let Some(working_directory) = working_directory {
    let path = std::path::Path::new(&working_directory);
    if path.exists() && path.is_dir() {
      Some(path.to_path_buf())
    } else {
      std::env::home_dir()
    }
  } else {
    std::env::home_dir()
  }
}
