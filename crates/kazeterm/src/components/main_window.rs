use std::sync::atomic::AtomicUsize;

use gpui::*;
use gpui_component::Size;
use kazeterm_ui_tree::action::UIAction;

use crate::components::about_dialog::AboutDialog;
use crate::components::close_confirm_dialog::CloseConfirmDialog;
use crate::components::import_alacritty_dialog::ImportAlacrittyDialog;
use crate::components::search_bar::SearchBar;
use crate::components::shell_error_dialog::ShellErrorDialog;
use crate::components::tab_rename_dialog::TabRenameDialog;
use crate::components::tab_switcher::TabSwitcher;
use crate::reconciler::UITreeStore;

pub(crate) use super::main_window_tab_item::TabItem;

const VERTICAL_TABBAR_WIDTH_RATIO: f32 = 0.175;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct KeyDebugModifiers {
  pub control: bool,
  pub shift: bool,
  pub alt: bool,
  pub platform: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct KeyDebugPressedKey {
  pub raw_key: String,
  pub modifiers: KeyDebugModifiers,
  pub action: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct KeyDebugRecentKey {
  pub raw_key: String,
  pub modifiers: KeyDebugModifiers,
  pub shortcut: String,
  pub action: Option<String>,
  pub expires_at: std::time::Instant,
}

pub struct MainWindow {
  pub(crate) focus_handle: FocusHandle,
  pub(crate) active_tab_ix: Option<usize>,
  #[allow(dead_code)]
  pub(crate) size: Size,
  pub(crate) items: Vec<TabItem>,
  pub(crate) tab_index: AtomicUsize,
  pub(crate) search_visible: bool,
  pub(crate) search_bar: Entity<SearchBar>,
  pub(crate) _search_bar_subscription: gpui::Subscription,
  pub(crate) tab_scroll_handle: gpui::ScrollHandle,
  pub(crate) scroll_tabs_to_end: bool,
  pub(crate) scroll_to_active_tab: bool,
  pub(crate) last_bounds: Option<gpui::Bounds<Pixels>>,
  pub(crate) tab_switcher_visible: bool,
  pub(crate) tab_switcher: Option<Entity<TabSwitcher>>,
  pub(crate) tab_switcher_selection: usize,
  pub(crate) vertical_tabbar_width: Pixels,
  pub(crate) key_debug_modifiers: KeyDebugModifiers,
  pub(crate) key_debug_pressed_keys: Vec<KeyDebugPressedKey>,
  pub(crate) key_debug_recent_keys: Vec<KeyDebugRecentKey>,
  /// Tab rename dialog state
  pub(crate) rename_dialog: Option<Entity<TabRenameDialog>>,
  pub(crate) _rename_dialog_subscription: Option<gpui::Subscription>,
  /// Close confirmation dialog state
  pub(crate) close_confirm_dialog: Option<Entity<CloseConfirmDialog>>,
  pub(crate) _close_confirm_subscription: Option<gpui::Subscription>,
  /// About dialog state
  pub(crate) about_dialog: Option<Entity<AboutDialog>>,
  pub(crate) _about_dialog_subscription: Option<gpui::Subscription>,
  /// Import Alacritty config dialog state
  pub(crate) import_alacritty_dialog: Option<Entity<ImportAlacrittyDialog>>,
  pub(crate) _import_alacritty_subscription: Option<gpui::Subscription>,
  /// Whether a UITree JSON file picker is currently active.
  pub(crate) ui_tree_json_prompt_pending: bool,
  /// Shell error dialog state
  pub(crate) shell_error_dialog: Option<Entity<ShellErrorDialog>>,
  pub(crate) _shell_error_subscription: Option<gpui::Subscription>,
  /// Tracks the last time an OS notification was sent, for throttling.
  pub(crate) last_notification_time: Option<std::time::Instant>,
  /// Whether the tab bar is currently visible
  pub(crate) tab_bar_visible: bool,
  /// Subscription for system appearance changes (used by ThemeMode::System)
  pub(crate) _appearance_subscription: gpui::Subscription,
  /// Data-driven UI tree store for serialization, diffing, and external API.
  pub(crate) ui_tree: UITreeStore,
  /// Guards against re-dispatching while tree diffs are being reconciled.
  pub(crate) reconciling_ui_tree: bool,
}

impl MainWindow {
  pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
    let entity = cx.new(|cx| Self::new(window, cx));
    let window_handle = window.window_handle();

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

    let main_window = entity.clone();
    cx.defer(move |cx| {
      let _ = cx.update_window(window_handle, |_root, window, cx| {
        main_window.update(cx, |this, cx| {
          this.focus_active_terminal(window, cx);
        });
      });
    });

    entity
  }

  pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
    let index = 0;
    let tab_index: AtomicUsize = AtomicUsize::new(index);

    let search_bar = cx.new(|cx| SearchBar::new(window, cx));
    let search_bar_subscription = cx.subscribe_in(&search_bar, window, Self::on_search_bar_event);
    let config = cx.global::<::config::Config>();
    let ui_font_size = config.font.ui_size;
    let vertical_tabbar_width = (window.bounds().size.width * VERTICAL_TABBAR_WIDTH_RATIO)
      .max(px(config.tab.get_vertical_tabbar_min_width(ui_font_size)))
      .min(px(config.tab.get_vertical_tabbar_max_width(ui_font_size)));

    let appearance_subscription = window.observe_window_appearance(|window, cx| {
      let config = cx.global::<::config::Config>().clone();
      if matches!(config.colors.theme_mode, ::config::ThemeMode::System) {
        let is_dark = matches!(
          window.appearance(),
          gpui::WindowAppearance::Dark | gpui::WindowAppearance::VibrantDark
        );
        let settings = crate::config::create_settings_store(&config, is_dark);
        cx.set_global(settings);
        themeing::SettingsStore::init_gpui_component_theme(cx);
      }
    });

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
      vertical_tabbar_width,
      key_debug_modifiers: KeyDebugModifiers::default(),
      key_debug_pressed_keys: Vec::new(),
      key_debug_recent_keys: Vec::new(),
      rename_dialog: None,
      _rename_dialog_subscription: None,
      close_confirm_dialog: None,
      _close_confirm_subscription: None,
      about_dialog: None,
      _about_dialog_subscription: None,
      import_alacritty_dialog: None,
      _import_alacritty_subscription: None,
      ui_tree_json_prompt_pending: false,
      shell_error_dialog: None,
      _shell_error_subscription: None,
      last_notification_time: None,
      tab_bar_visible: true,
      _appearance_subscription: appearance_subscription,
      ui_tree: UITreeStore::new(),
      reconciling_ui_tree: false,
    };

    // Try to restore previous workspace
    let config = cx.global::<::config::Config>();
    if config.window.restore_workspace {
      if let Some(tree) = UITreeStore::load_workspace() {
        main_window.reconciling_ui_tree = true;
        main_window.restore_from_ui_tree(&tree, window, cx);
        main_window.reconciling_ui_tree = false;
        main_window.ui_tree = UITreeStore::from_tree(tree);
        UITreeStore::delete_workspace();
        return main_window;
      }
    }

    main_window.insert_new_tab(window, cx);
    main_window
  }

  /// Capture the current GPUI state into the UI tree.
  /// Call this before snapshotting or after external changes to sync state.
  pub fn sync_ui_tree(&mut self, cx: &mut Context<Self>) {
    let mut tree_store = std::mem::replace(&mut self.ui_tree, UITreeStore::new());
    tree_store.capture_from_main_window(self, cx);
    self.ui_tree = tree_store;
  }

  /// Dump the current UI tree as a JSON string.
  pub fn snapshot_ui_tree(&mut self, cx: &mut Context<Self>) -> Result<String, serde_json::Error> {
    self.sync_ui_tree(cx);
    self.ui_tree.to_json()
  }

  /// Dump the current UI tree as a `serde_json::Value`.
  pub fn snapshot_ui_tree_value(
    &mut self,
    cx: &mut Context<Self>,
  ) -> Result<serde_json::Value, serde_json::Error> {
    self.sync_ui_tree(cx);
    self.ui_tree.to_json_value()
  }

  pub(crate) fn dump_ui_tree_to_path(
    &mut self,
    path: &std::path::Path,
    cx: &mut Context<Self>,
  ) -> Result<(), String> {
    let json = self
      .snapshot_ui_tree(cx)
      .map_err(|err| format!("Failed to serialize UI tree: {err}"))?;

    if let Some(parent) = path.parent()
      && !parent.as_os_str().is_empty()
    {
      std::fs::create_dir_all(parent)
        .map_err(|err| format!("Failed to create directory '{}': {err}", parent.display()))?;
    }

    std::fs::write(path, json).map_err(|err| {
      format!(
        "Failed to write UI tree JSON to '{}': {err}",
        path.display()
      )
    })?;
    tracing::info!("Dumped UI tree JSON to {}", path.display());
    Ok(())
  }

  pub(crate) fn sync_ui_tree_and_window_id(&mut self, cx: &mut Context<Self>) -> Option<String> {
    self.sync_ui_tree(cx);
    self.ui_tree.window_id().map(ToOwned::to_owned)
  }

  pub(crate) fn dispatch_default_ui_action(
    &mut self,
    action: UIAction,
    action_name: &str,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    if let Err(err) = self.dispatch_ui_action(action, window, cx) {
      tracing::error!("Failed to {action_name} via UITree: {err}");
    }
  }

  /// Apply a `UIAction` through the tree, producing diffs and reconciling
  /// them back into the live GPUI state.
  pub fn dispatch_ui_action(
    &mut self,
    action: kazeterm_ui_tree::action::UIAction,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) -> Result<(), anyhow::Error> {
    if self.ui_tree.window_id().is_none() {
      self.sync_ui_tree(cx);
    }
    let mut tree_store = std::mem::replace(&mut self.ui_tree, UITreeStore::new());
    let result = tree_store.dispatch(action, self, window, cx);
    self.ui_tree = tree_store;
    result
  }

  /// Load a full UI tree from JSON into the store.
  pub fn load_ui_tree_json(&mut self, json: &str) -> Result<(), serde_json::Error> {
    self.ui_tree.load_json(json)
  }
}

impl Focusable for MainWindow {
  fn focus_handle(&self, _: &gpui::App) -> gpui::FocusHandle {
    self.focus_handle.clone()
  }
}
