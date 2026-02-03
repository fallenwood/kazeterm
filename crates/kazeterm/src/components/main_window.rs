use std::{path::PathBuf, sync::atomic::AtomicUsize};

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::{
  Icon, IconName, Selectable, Sizable, Size, TitleBar,
  button::{Button, ButtonVariants},
  h_flex,
  label::Label,
  menu::{ContextMenuExt, DropdownMenu, PopupMenu, PopupMenuItem},
  tab::{Tab, TabBar},
};
use terminal::TerminalView;
use themeing::SettingsStore;

use crate::components::about_dialog::{AboutDialog, AboutDialogCloseEvent};
use crate::components::close_confirm_dialog::{CloseConfirmDialog, CloseConfirmEvent};
use crate::components::dragged_tab::{DraggedTab, DraggedTabView};
use crate::components::search_bar::{SearchBar, SearchBarCloseEvent};
use crate::components::shell_icon::ShellIcon;
use crate::components::tab_button::{TabButton, TabButtonClickEvent};
use crate::components::split_pane::{SplitContainer, SplitDirection};
use crate::components::tab_rename_dialog::{TabRenameDialog, TabRenameEvent};
use crate::components::tab_switcher::{TabSwitcher, TabSwitcherItem};

/// Maximum width for tab labels before truncation
const TAB_LABEL_MAX_WIDTH: f32 = 150.0;

pub struct TabItem {
  index: usize,
  title: String,
  /// Custom title set by the user. When Some, auto-title updates are ignored.
  custom_title: Option<String>,
  shell_path: String,
  _shell_name: String,
  split_container: SplitContainer,
  _subscription: gpui::Subscription,
}

impl TabItem {
  /// Returns the display title (custom title if set, otherwise the auto-assigned title)
  fn display_title(&self) -> &str {
    self.custom_title.as_deref().unwrap_or(&self.title)
  }
}

pub struct MainWindow {
  focus_handle: FocusHandle,
  active_tab_ix: Option<usize>,
  size: Size,
  items: Vec<TabItem>,
  tab_index: AtomicUsize,
  search_visible: bool,
  search_bar: Entity<SearchBar>,
  _search_bar_subscription: gpui::Subscription,
  tab_scroll_handle: gpui::ScrollHandle,
  scroll_tabs_to_end: bool,
  scroll_to_active_tab: bool,
  last_bounds: Option<gpui::Bounds<Pixels>>,
  tab_switcher_visible: bool,
  tab_switcher: Option<Entity<TabSwitcher>>,
  tab_switcher_selection: usize,
  last_known_ctrl_state: bool,
  /// Tab rename dialog state
  rename_dialog: Option<Entity<TabRenameDialog>>,
  _rename_dialog_subscription: Option<gpui::Subscription>,
  /// Close confirmation dialog state
  close_confirm_dialog: Option<Entity<CloseConfirmDialog>>,
  _close_confirm_subscription: Option<gpui::Subscription>,
  /// About dialog state
  about_dialog: Option<Entity<AboutDialog>>,
  _about_dialog_subscription: Option<gpui::Subscription>,
}

impl MainWindow {
  pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
    let entity = cx.new(|cx| Self::new(window, cx));

    // Register window close interception for Alt+F4 and system close button
    let main_window = entity.clone();
    window.on_window_should_close(cx, move |window, cx| {
      main_window.update(cx, |this, cx| {
        if this.is_close_confirm_visible() {
          // Dialog already showing, prevent close
          false
        } else {
          // Show the confirmation dialog
          this.show_close_confirm_dialog(window, cx);
          false // Prevent immediate close
        }
      })
    });

    entity
  }

  fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
    let index = 0;
    let tab_index: AtomicUsize = AtomicUsize::new(index);

    let search_bar = cx.new(|cx| SearchBar::new(window, cx));
    let search_bar_subscription = cx.subscribe_in(&search_bar, window, Self::on_search_bar_event);

    let mut main_window = Self {
      focus_handle: cx.focus_handle(),
      active_tab_ix: None,
      size: Size::default(),
      items: vec![],
      tab_index,
      search_visible: false,
      search_bar,
      _search_bar_subscription: search_bar_subscription,
      tab_scroll_handle: gpui::ScrollHandle::new(),
      scroll_tabs_to_end: false,
      scroll_to_active_tab: false,
      last_bounds: None,
      tab_switcher_visible: false,
      tab_switcher: None,
      tab_switcher_selection: 0,
      last_known_ctrl_state: false,
      rename_dialog: None,
      _rename_dialog_subscription: None,
      close_confirm_dialog: None,
      _close_confirm_subscription: None,
      about_dialog: None,
      _about_dialog_subscription: None,
    };
    main_window.insert_new_tab(window, cx);
    main_window
  }

  fn on_search_bar_event(
    &mut self,
    _search_bar: &Entity<SearchBar>,
    _event: &SearchBarCloseEvent,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    self.toggle_search(window, cx);
  }

  fn set_active_tab(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
    self.active_tab_ix = Some(ix);

    // Update search bar with the new active terminal
    if let Some(item) = self.items.get(ix) {
      if let Some(terminal) = item.split_container.get_active_terminal() {
        let terminal_clone = terminal.clone();
        self.search_bar.update(cx, |search_bar, _cx| {
          search_bar.set_terminal_view(terminal_clone.clone());
        });

        // Focus the terminal
        window.focus(&terminal.focus_handle(cx));
      }
    }

    cx.notify();
  }

  fn toggle_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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

  fn show_tab_switcher(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
    if self.items.len() <= 1 {
      return;
    }

    let was_visible = self.tab_switcher_visible;

    if !self.tab_switcher_visible {
      // First time showing the switcher - initialize selection
      let current_ix = self.active_tab_ix.unwrap_or(0);
      self.tab_switcher_selection = if forward {
        (current_ix + 1) % self.items.len()
      } else {
        if current_ix == 0 {
          self.items.len() - 1
        } else {
          current_ix - 1
        }
      };
      self.tab_switcher_visible = true;
    } else {
      // Switcher already visible - cycle selection
      if forward {
        self.tab_switcher_selection = (self.tab_switcher_selection + 1) % self.items.len();
      } else {
        self.tab_switcher_selection = if self.tab_switcher_selection == 0 {
          self.items.len() - 1
        } else {
          self.tab_switcher_selection - 1
        };
      }
    }

    // Switch to the selected tab immediately
    self.set_active_tab(self.tab_switcher_selection, window, cx);
    self.update_tab_switcher(cx);
    cx.notify();

    // Start polling if we just showed the switcher
    if !was_visible {
      self.start_ctrl_polling(cx);
    }
  }

  fn start_ctrl_polling(&self, cx: &mut Context<Self>) {
    // Poll to detect when Ctrl is released
    cx.spawn(async move |this_weak, cx| {
      loop {
        smol::Timer::after(std::time::Duration::from_millis(50)).await;

        // Check if Ctrl is still pressed (always true for now)
        let ctrl_pressed = true; // Can't reliably check on Wayland without X11 libs

        let should_hide = cx
          .update(|_cx| {
            if let Some(this) = this_weak.upgrade() {
              let switcher_visible = this.read(_cx).tab_switcher_visible;
              switcher_visible
            } else {
              false
            }
          })
          .unwrap_or(false);

        if !should_hide {
          break;
        }
      }
    })
    .detach();
  }

  fn hide_tab_switcher(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
    if self.tab_switcher_visible {
      self.tab_switcher_visible = false;
      self.tab_switcher = None;
      cx.notify();
    }
  }

  fn update_tab_switcher(&mut self, cx: &mut Context<Self>) {
    let items: Vec<TabSwitcherItem> = self
      .items
      .iter()
      .enumerate()
      .map(|(ix, item)| TabSwitcherItem {
        index: item.index,
        title: item.display_title().to_string(),
        shell_path: item.shell_path.clone(),
        is_selected: ix == self.tab_switcher_selection,
      })
      .collect();

    let tab_switcher = cx.new(|_cx| TabSwitcher::new(items, self.tab_switcher_selection));
    self.tab_switcher = Some(tab_switcher);
  }

  pub fn insert_new_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    self.insert_new_tab_with_profile(None, None, window, cx);
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

    let terminal = super::terminal_window::new_terminal_window_with_shell(
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

  fn subscribe_terminal_view_event(
    this: &mut MainWindow,
    terminal_view: &Entity<TerminalView>,
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
              window.focus(&terminal.focus_handle(cx));
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

  fn play_bell_sound(&self) {
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

      // If no tabs left, insert a new one
      if self.items.is_empty() {
        self.insert_new_tab(window, cx);
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

  pub fn split_pane_horizontal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    self.split_pane(SplitDirection::Horizontal, window, cx);
  }

  pub fn split_pane_vertical(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    self.split_pane(SplitDirection::Vertical, window, cx);
  }

  fn split_pane(&mut self, direction: SplitDirection, window: &mut Window, cx: &mut Context<Self>) {
    if let Some(active_tab_ix) = self.active_tab_ix {
      if let Some(item) = self.items.get_mut(active_tab_ix) {
        // Get the active terminal's working directory
        let working_directory = if let Some(active_terminal) = item.split_container.get_active_terminal() {
          active_terminal.read(cx).terminal().read(cx).pty_info.current.as_ref().map(|info| {
            info.cwd.to_string_lossy().to_string()
          })
        } else {
          None
        };

        // Create a new terminal with the same shell
        let index = self
          .tab_index
          .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let config = cx.global::<::config::Config>();
        let shell = config.get_shell().clone();
        let working_directory_path = get_working_directory_pathbuf(working_directory);

        let new_terminal = super::terminal_window::new_terminal_window_with_shell(
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
    if let Some(active_tab_ix) = self.active_tab_ix {
      if let Some(item) = self.items.get_mut(active_tab_ix) {
        if item.split_container.close_active_pane() {
          // Focus the newly active terminal
          if let Some(terminal) = item.split_container.get_active_terminal() {
            window.focus(&terminal.focus_handle(cx));
          }
          cx.notify();
        }
      }
    }
  }

  fn show_rename_dialog(&mut self, tab_index: usize, window: &mut Window, cx: &mut Context<Self>) {
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
    dialog.update(cx, |dialog, cx| {
      dialog.focus(window, cx);
    });

    self.rename_dialog = Some(dialog);
    self._rename_dialog_subscription = Some(subscription);
    cx.notify();
  }

  fn on_rename_dialog_event(
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
    if let Some(active_ix) = self.active_tab_ix {
      if let Some(item) = self.items.get(active_ix) {
        if let Some(terminal) = item.split_container.get_active_terminal() {
          window.focus(&terminal.focus_handle(cx));
        }
      }
    }

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

  fn on_close_confirm_event(
    &mut self,
    _dialog: &Entity<CloseConfirmDialog>,
    event: &CloseConfirmEvent,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    match event {
      CloseConfirmEvent::Confirm => {
        // User confirmed, close the window
        self.close_confirm_dialog = None;
        self._close_confirm_subscription = None;
        window.remove_window();
      }
      CloseConfirmEvent::Cancel => {
        // User cancelled, just close the dialog
        self.close_confirm_dialog = None;
        self._close_confirm_subscription = None;

        // Refocus the terminal
        if let Some(active_ix) = self.active_tab_ix {
          if let Some(item) = self.items.get(active_ix) {
            if let Some(terminal) = item.split_container.get_active_terminal() {
              window.focus(&terminal.focus_handle(cx));
            }
          }
        }

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

  fn on_about_dialog_event(
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
    if let Some(active_ix) = self.active_tab_ix {
      if let Some(item) = self.items.get(active_ix) {
        if let Some(terminal) = item.split_container.get_active_terminal() {
          window.focus(&terminal.focus_handle(cx));
        }
      }
    }

    cx.notify();
  }
}

impl Focusable for MainWindow {
  fn focus_handle(&self, _: &gpui::App) -> gpui::FocusHandle {
    self.focus_handle.clone()
  }
}

impl Render for MainWindow {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let search_visible = self.search_visible;
    let search_bar = self.search_bar.clone();
    let config = cx.global::<::config::Config>();
    let setting_store = cx.global::<SettingsStore>();
    let local_profiles = config.get_local_profiles_with_shells();
    let container_profiles = config.get_container_profiles_with_shells();
    let ssh_hosts = ::config::Config::get_ssh_hosts();

    // Get current window bounds to detect resize
    let current_bounds = window.bounds();
    let bounds_changed = self.last_bounds.map_or(true, |last| last != current_bounds);

    if bounds_changed {
      self.last_bounds = Some(current_bounds);
      // Set flag to scroll active tab into view on resize
      self.scroll_to_active_tab = true;
    }

    if self.scroll_tabs_to_end {
      self.scroll_tabs_to_end = false;
      let scroll_handle = self.tab_scroll_handle.clone();
      cx.spawn(async move |_this, cx| {
        // Small delay to allow layout to complete
        // smol::Timer::after(std::time::Duration::from_millis(50)).await;
        cx.update(|_cx| {
          let max_offset = scroll_handle.max_offset();
          scroll_handle.set_offset(gpui::point(-max_offset.width, px(0.0)));
        })
        .ok();
      })
      .detach();
    }

    if self.scroll_to_active_tab {
      self.scroll_to_active_tab = false;
      let scroll_handle = self.tab_scroll_handle.clone();
      let active_tab_ix = self.active_tab_ix.unwrap_or_default();
      let total_tabs = self.items.len();
      cx.spawn(async move |_this, cx| {
        cx.update(|_cx| {
          if total_tabs > 0 && active_tab_ix < total_tabs {
            // Calculate the approximate position of the active tab
            // This is a simple approach - scroll proportionally based on tab index
            let max_offset = scroll_handle.max_offset();
            let scroll_ratio = active_tab_ix as f32 / total_tabs.max(1) as f32;
            let target_offset = -max_offset.width * scroll_ratio;
            scroll_handle.set_offset(gpui::point(target_offset, px(0.0)));
          }
        })
        .ok();
      })
      .detach();
    }

    let view = cx.entity();

    let theme = setting_store.theme();
    let colors = theme.colors();

    div()
      .flex()
      .flex_col()
      .size_full()
      .key_context("MainWindow")
      .on_key_down(cx.listener(move |this, e: &KeyDownEvent, window, cx| {
        // Track Ctrl state
        this.last_known_ctrl_state = e.keystroke.modifiers.control;

        if e.keystroke.modifiers.control && e.keystroke.key == "tab" {
          // Ctrl+Tab or Ctrl+Shift+Tab - just switch tabs without showing switcher
          let forward = !e.keystroke.modifiers.shift;
          let current_ix = this.active_tab_ix.unwrap_or(0);
          let next_ix = if forward {
            (current_ix + 1) % this.items.len()
          } else {
            if current_ix == 0 {
              this.items.len() - 1
            } else {
              current_ix - 1
            }
          };
          this.set_active_tab(next_ix, window, cx);
        } else if e.keystroke.modifiers.shift
          && e.keystroke.modifiers.control
          && e.keystroke.key == "f"
        {
          this.toggle_search(window, cx);
        } else if e.keystroke.key == "Escape" && this.search_visible {
          this.toggle_search(window, cx);
        } else if e.keystroke.modifiers.shift && e.keystroke.modifiers.control && e.keystroke.key == "d" {
          this.split_pane_horizontal(window, cx);
        } else if e.keystroke.modifiers.shift && e.keystroke.modifiers.control && e.keystroke.key == "e" {
          this.split_pane_vertical(window, cx);
        } else if e.keystroke.modifiers.shift && e.keystroke.modifiers.control && e.keystroke.key == "w" {
          this.close_active_pane(window, cx);
        }
      }))
      .on_key_up(cx.listener(move |this, e: &KeyUpEvent, _window, _cx| {
        // Track Ctrl state
        this.last_known_ctrl_state = e.keystroke.modifiers.control;
      }))
      .on_mouse_move(cx.listener(move |this, _e: &MouseMoveEvent, _window, _cx| {
        // Track Ctrl state from mouse events
        this.last_known_ctrl_state = _e.modifiers.control;
      }))
      .on_mouse_down(
        MouseButton::Left,
        cx.listener(move |_this, _e: &MouseDownEvent, _window, _cx| {
          // No-op
        }),
      )
      .on_mouse_down(
        MouseButton::Right,
        cx.listener(move |_this, _e: &MouseDownEvent, _window, _cx| {
          // No-op
        }),
      )
      .child(
        TitleBar::new()
          .on_close_window({
            let main_window = view.clone();
            move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
              main_window.update(cx, |this, cx| {
                this.show_close_confirm_dialog(window, cx);
              });
            }
          })
          .child(
            div()
              .flex_1()
              .flex_basis(px(0.0))
              .min_w_0()
              .overflow_x_hidden()
              .child(
                TabBar::new("tabs")
                  .min_w_0()
                  .underline()
                  .track_scroll(&self.tab_scroll_handle)
                  .with_size(self.size)
                  .selected_index(self.active_tab_ix.unwrap_or_default())
                  .on_click(cx.listener(|this, index: &usize, window, cx| {
                    this.set_active_tab(*index, window, cx);
                  }))
                  .children(
                    self
                      .items
                      .iter()
                      .enumerate()
                      .map(|(tab_ix, item)| {
                        let shell_icon = ShellIcon::new(&item.shell_path);
                        let tab_index = item.index;
                        let tab_title = item.display_title().to_string();
                        let total_tabs = self.items.len();
                        let is_first = tab_ix == 0;
                        let is_last = tab_ix == total_tabs - 1;
                        let is_selected = self.active_tab_ix == Some(tab_ix);
                        let has_bell = item.split_container.all_terminals().iter().any(|(_, t)| t.read(cx).has_bell());
                        let view = cx.entity();
                        let all_terminals = item.split_container.all_terminals();
                        // Define colors for selected tab highlight
                        let selected_bg: gpui::Hsla = colors.tab_active_background;
                        let normal_bg = colors.tab_inactive_background;
                        let hover_bg = colors.element_hover;
                        let text_color = colors.text;
                        let text_muted = colors.text_muted;
                        let accent_color = colors.text_accent;
                        let warning_color = colors.terminal_ansi_yellow;

                        Tab::new()
                          .selected(is_selected)
                          .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            // Clear bell when clicking on tab
                            for (_, terminal) in &all_terminals {
                              terminal.update(cx, |terminal_view, cx| {
                                terminal_view.clear_bell(cx);
                              });
                            }
                            // Prevent TitleBar from starting window drag when clicking on tabs
                            cx.stop_propagation();
                          })
                          .child(
                            div()
                              .id(ElementId::NamedInteger(
                                "tab-container".into(),
                                tab_ix as u64,
                              ))
                              .group("tab-item")
                              .child(
                                div()
                                  .id(ElementId::NamedInteger("tab-drag".into(), tab_ix as u64))
                                  .cursor(CursorStyle::OpenHand)
                                  .on_drag(
                                    DraggedTab {
                                      from_ix: tab_ix,
                                      title: tab_title.clone(),
                                      shell_path: item.shell_path.clone(),
                                    },
                                    |dragged: &DraggedTab, _offset, _window, cx| {
                                      cx.new(|_cx| {
                                        DraggedTabView::new(
                                          dragged.title.clone(),
                                          dragged.shell_path.clone(),
                                        )
                                      })
                                    },
                                  )
                                  .drag_over::<DraggedTab>(move |style, _dragged, _window, _cx| {
                                    // Visual feedback during drag - show drop indicator
                                    style
                                      .bg(accent_color.opacity(0.15))
                                      .border_l_2()
                                      .border_color(accent_color)
                                  })
                                  .on_drop(cx.listener(
                                    move |this, dragged: &DraggedTab, _window, cx| {
                                      let from_ix = dragged.from_ix;
                                      let to_ix = tab_ix;
                                      if from_ix != to_ix {
                                        // Remove the item from the original position and insert at new position
                                        let item = this.items.remove(from_ix);
                                        let insert_ix = if from_ix < to_ix { to_ix } else { to_ix };
                                        this.items.insert(insert_ix, item);
                                        // Update active tab index
                                        if let Some(active) = this.active_tab_ix {
                                          if active == from_ix {
                                            this.active_tab_ix = Some(insert_ix);
                                          } else if from_ix < active && active <= to_ix {
                                            this.active_tab_ix = Some(active - 1);
                                          } else if to_ix <= active && active < from_ix {
                                            this.active_tab_ix = Some(active + 1);
                                          }
                                        }
                                        cx.notify();
                                      }
                                    },
                                  ))
                                  .child(
                                    h_flex()
                                      .id(ElementId::NamedInteger(
                                        "tab-inner".into(),
                                        tab_ix as u64,
                                      ))
                                      .gap_1p5()
                                      .pl_2p5()
                                      .pr_1()
                                      .py_1()
                                      .items_center()
                                      .min_w(px(60.0))
                                      .max_w(px(TAB_LABEL_MAX_WIDTH + 50.0))
                                      // Background styling
                                      .when(is_selected, |this| {
                                        this.bg(selected_bg).border_b_2().border_color(accent_color)
                                      })
                                      .when(!is_selected, |this| {
                                        this.bg(normal_bg).hover(|style| style.bg(hover_bg))
                                      })
                                      .rounded_t_md()
                                      // Shell icon
                                      .child(
                                        div()
                                          .flex_shrink_0()
                                          .child(shell_icon.into_element(px(14.0))),
                                      )
                                      // Bell indicator
                                      .when(has_bell, |this| {
                                        this.child(
                                          div().flex_shrink_0().child(
                                            Icon::new(IconName::Bell)
                                              .size_3()
                                              .text_color(warning_color),
                                          ),
                                        )
                                      })
                                      // Tab label with text truncation
                                      .child(
                                        div().flex_1().min_w_0().overflow_x_hidden().child(
                                          Label::new(tab_title.clone())
                                            .text_color(if is_selected {
                                              text_color
                                            } else {
                                              text_muted
                                            })
                                            .whitespace_nowrap(),
                                        ),
                                      )
                                      // Close button - visible on hover or when selected
                                      .child({
                                        let close_visible = is_selected;
                                        div()
                                          .flex_shrink_0()
                                          .when(!close_visible, |this| {
                                            this
                                              .invisible()
                                              .group_hover("tab-item", |style| style.visible())
                                          })
                                          .child(
                                            TabButton::new("close", tab_index)
                                              .visible(true)
                                              .on_click(cx.listener(
                                                |this, e: &TabButtonClickEvent, window, cx| {
                                                  let tab_index = e.index;
                                                  this.remove_tab_by(tab_index, window, cx);
                                                },
                                              )),
                                          )
                                      })
                                      .on_mouse_down(MouseButton::Right, |_, _, cx| {
                                        cx.stop_propagation();
                                      })
                                      .context_menu({
                                        let view = view.clone();
                                        move |menu, _window, _cx| {
                                          let view_rename = view.clone();
                                          let view_split_h = view.clone();
                                          let view_split_v = view.clone();
                                          let view_close_pane = view.clone();
                                          let view_move_left = view.clone();
                                          let view_move_right = view.clone();
                                          let view_close_others = view.clone();
                                          let view_close_right = view.clone();
                                          let view_close_tab = view.clone();
                                          menu
                                            .item(PopupMenuItem::new("Rename Tab").on_click(
                                              move |_, window, cx| {
                                                view_rename.update(cx, |this, cx| {
                                                  this.show_rename_dialog(tab_index, window, cx);
                                                });
                                              },
                                            ))
                                            .separator()
                                            .item(
                                              PopupMenuItem::new("Split Horizontal (Ctrl+Shift+D)").on_click(
                                                move |_, window, cx| {
                                                  view_split_h.update(cx, |this, cx| {
                                                    this.split_pane_horizontal(window, cx);
                                                  });
                                                },
                                              ),
                                            )
                                            .item(
                                              PopupMenuItem::new("Split Vertical (Ctrl+Shift+E)").on_click(
                                                move |_, window, cx| {
                                                  view_split_v.update(cx, |this, cx| {
                                                    this.split_pane_vertical(window, cx);
                                                  });
                                                },
                                              ),
                                            )
                                            .item(
                                              PopupMenuItem::new("Close Pane (Ctrl+Shift+W)").on_click(
                                                move |_, window, cx| {
                                                  view_close_pane.update(cx, |this, cx| {
                                                    this.close_active_pane(window, cx);
                                                  });
                                                },
                                              ),
                                            )
                                            .separator()
                                            .item(
                                              PopupMenuItem::new("Move Left")
                                                .disabled(is_first)
                                                .on_click(move |_, _window, cx| {
                                                  view_move_left.update(cx, |this, cx| {
                                                    if tab_ix > 0 {
                                                      this.items.swap(tab_ix, tab_ix - 1);
                                                      this.active_tab_ix = Some(tab_ix - 1);
                                                      cx.notify();
                                                    }
                                                  });
                                                }),
                                            )
                                            .item(
                                              PopupMenuItem::new("Move Right")
                                                .disabled(is_last)
                                                .on_click(move |_, _window, cx| {
                                                  view_move_right.update(cx, |this, cx| {
                                                    if tab_ix + 1 < this.items.len() {
                                                      this.items.swap(tab_ix, tab_ix + 1);
                                                      this.active_tab_ix = Some(tab_ix + 1);
                                                      cx.notify();
                                                    }
                                                  });
                                                }),
                                            )
                                            .separator()
                                            .item(
                                              PopupMenuItem::new("Close Other Tabs")
                                                .disabled(total_tabs <= 1)
                                                .on_click(move |_, _window, cx| {
                                                  view_close_others.update(cx, |this, cx| {
                                                    let keep_index = tab_index;
                                                    this
                                                      .items
                                                      .retain(|tab| tab.index == keep_index);
                                                    this.active_tab_ix = Some(0);
                                                    cx.notify();
                                                  });
                                                }),
                                            )
                                            .item(
                                              PopupMenuItem::new("Close Tabs to Right")
                                                .disabled(is_last)
                                                .on_click(move |_, _window, cx| {
                                                  view_close_right.update(cx, |this, cx| {
                                                    let right_ix = tab_ix + 1;
                                                    if right_ix < this.items.len() {
                                                      this.items.truncate(right_ix);
                                                      this.active_tab_ix = Some(tab_ix);
                                                      cx.notify();
                                                    }
                                                  });
                                                }),
                                            )
                                            .item(PopupMenuItem::new("Close Tab").on_click(
                                              move |_, window, cx| {
                                                view_close_tab.update(cx, |this, cx| {
                                                  this.remove_tab_by(tab_index, window, cx);
                                                });
                                              },
                                            ))
                                        }
                                      }),
                                  ),
                              ),
                          )
                      })
                      .collect::<Vec<_>>(),
                  ),
              ),
          )
          .child(
            h_flex()
              .flex_shrink_0()
              .gap_0()
              .child(
                Button::new("new")
                  .ghost()
                  .small()
                  .label("+")
                  .on_mouse_down(MouseButton::Left, |_, _, cx| {
                    cx.stop_propagation();
                  })
                  .on_click(cx.listener(|this, _e, window, cx| {
                    this.insert_new_tab(window, cx);
                  })),
              )
              .child(
                Button::new("more")
                  .ghost()
                  .small()
                  .label("∨")
                  .dropdown_menu({
                    let view_about = view.clone();
                    move |menu: PopupMenu, _window: &mut Window, _cx: &mut Context<PopupMenu>| {
                      let mut menu = menu;

                      // Local profiles
                      for (name, shell_path) in local_profiles.iter() {
                        let profile_name = name.clone();
                        let shell_path = shell_path.clone();
                        let display_name = name.clone();
                        let view_clone = view.clone();
                        menu = menu.item(
                          PopupMenuItem::element(move |_window, _cx| {
                            let shell_icon = ShellIcon::new(&shell_path);
                            h_flex()
                              .gap_2()
                              .items_center()
                              .child(
                                div()
                                  .w(px(16.0))
                                  .h(px(16.0))
                                  .flex()
                                  .items_center()
                                  .justify_center()
                                  .child(shell_icon.into_element(px(16.0))),
                              )
                              .child(display_name.clone())
                              .into_any_element()
                          })
                          .on_click(move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                            view_clone.update(cx, |this, cx| {
                              this.insert_new_tab_with_profile(
                                Some(&profile_name),
                                None,
                                window,
                                cx,
                              );
                            });
                          }),
                        );
                      }

                      // Container profiles
                      if !container_profiles.is_empty() {
                        menu = menu.separator();
                        for (name, shell_path) in container_profiles.iter() {
                          let profile_name = name.clone();
                          let shell_path = shell_path.clone();
                          let display_name = name.clone();
                          let view_clone = view.clone();
                          menu = menu.item(
                            PopupMenuItem::element(move |_window, _cx| {
                              let shell_icon = ShellIcon::new(&shell_path);
                              h_flex()
                                .gap_2()
                                .items_center()
                                .child(
                                  div()
                                    .w(px(16.0))
                                    .h(px(16.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(shell_icon.into_element(px(16.0))),
                                )
                                .child(display_name.clone())
                                .into_any_element()
                            })
                            .on_click(move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                              view_clone.update(cx, |this, cx| {
                                this.insert_new_tab_with_profile(
                                  Some(&profile_name),
                                  None,
                                  window,
                                  cx,
                                );
                              });
                            }),
                          );
                        }
                      }

                      // SSH Hosts
                      if !ssh_hosts.is_empty() {
                        menu = menu.separator();
                        for name in ssh_hosts.iter() {
                          let profile_name = name.clone();
                          let display_name = format!("[ssh] {}", name);
                          let view_clone = view.clone();
                          menu = menu.item(
                            PopupMenuItem::element(move |_window, _cx| {
                              h_flex()
                                .gap_2()
                                .items_center()
                                .child(
                                  div()
                                    .w(px(16.0))
                                    .h(px(16.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(Icon::new(IconName::Globe).size_4()),
                                )
                                .child(display_name.clone())
                                .into_any_element()
                            })
                            .on_click(move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                              view_clone.update(cx, |this, cx| {
                                this.insert_new_tab_with_profile(
                                  Some(&profile_name),
                                  None,
                                  window,
                                  cx,
                                );
                              });
                            }),
                          );
                        }
                      }

                      menu = menu.separator();
                      menu = menu.item(
                        PopupMenuItem::element(|_window, _cx| {
                          h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                              div()
                                .w(px(16.0))
                                .h(px(16.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(Icon::new(IconName::Folder).size_4()),
                            )
                            .child("Open Config Path")
                            .into_any_element()
                        })
                        .on_click(move |_: &ClickEvent, _window: &mut Window, cx: &mut App| {
                          let config_path = ::config::Config::get_config_path();
                          cx.open_url(&format!("file://{}", config_path.display()));
                        }),
                      );
                      menu = menu.item(
                        PopupMenuItem::element(|_window, _cx| {
                          h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                              div()
                                .w(px(16.0))
                                .h(px(16.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(Icon::new(IconName::File).size_4()),
                            )
                            .child("Open Config File")
                            .into_any_element()
                        })
                        .on_click(|_: &ClickEvent, _: &mut Window, cx: &mut App| {
                          if let Some(path) = ::config::Config::get_config_file_path() {
                            cx.open_url(&format!("file://{}", path.display()));
                          }
                        }),
                      );

                      // About section
                      menu = menu.separator();
                      let view_about = view_about.clone();
                      menu = menu.item(
                        PopupMenuItem::element(|_window, _cx| {
                          h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                              div()
                                .w(px(16.0))
                                .h(px(16.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(Icon::new(IconName::Info).size_4()),
                            )
                            .child("About")
                            .into_any_element()
                        })
                        .on_click(move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                          view_about.update(cx, |this, cx| {
                            this.show_about_dialog(window, cx);
                          });
                        }),
                      );

                      menu
                    }
                  }),
              ),
          ),
      )
      .child({
        let active_ix = self.active_tab_ix.unwrap_or_default();
        div()
          .flex_1()
          .size_full()
          .child(
            self
              .items
              .get(active_ix)
              .map(|item| item.split_container.render(window, cx))
              .unwrap_or_else(|| {
                tracing::warn!(
                  "render: NO ITEM FOUND at index {}, showing empty div",
                  active_ix
                );
                div().into_any_element()
              }),
          )
          .when(search_visible, |this| this.child(search_bar))
          .when(self.tab_switcher_visible, |this| {
            if let Some(tab_switcher) = &self.tab_switcher {
              this.child(tab_switcher.clone())
            } else {
              this
            }
          })
          .when(self.rename_dialog.is_some(), |this| {
            if let Some(rename_dialog) = &self.rename_dialog {
              this.child(rename_dialog.clone())
            } else {
              this
            }
          })
          .when(self.close_confirm_dialog.is_some(), |this| {
            if let Some(close_confirm_dialog) = &self.close_confirm_dialog {
              this.child(close_confirm_dialog.clone())
            } else {
              this
            }
          })
          .when(self.about_dialog.is_some(), |this| {
            if let Some(about_dialog) = &self.about_dialog {
              this.child(about_dialog.clone())
            } else {
              this
            }
          })
      })
  }
}

fn get_working_directory_pathbuf(working_directory: Option<String>) -> Option<PathBuf> {
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
