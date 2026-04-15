use std::sync::atomic::AtomicUsize;

use gpui::*;
use gpui_component::Size;

use crate::components::about_dialog::AboutDialog;
use crate::components::close_confirm_dialog::CloseConfirmDialog;
use crate::components::import_alacritty_dialog::ImportAlacrittyDialog;
use crate::components::search_bar::SearchBar;
use crate::components::shell_error_dialog::ShellErrorDialog;
use crate::components::tab_rename_dialog::TabRenameDialog;
use crate::components::tab_switcher::TabSwitcher;
use crate::components::workspace_state::WorkspaceState;

pub(crate) use super::main_window_tab_item::TabItem;

const VERTICAL_TABBAR_WIDTH_RATIO: f32 = 0.175;

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
  pub(crate) last_known_ctrl_state: bool,
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
  /// Shell error dialog state
  pub(crate) shell_error_dialog: Option<Entity<ShellErrorDialog>>,
  pub(crate) _shell_error_subscription: Option<gpui::Subscription>,
  /// Tracks the last time an OS notification was sent, for throttling.
  pub(crate) last_notification_time: Option<std::time::Instant>,
  /// Whether the tab bar is currently visible
  pub(crate) tab_bar_visible: bool,
  /// Subscription for system appearance changes (used by ThemeMode::System)
  pub(crate) _appearance_subscription: gpui::Subscription,
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
    let config = cx.global::<::config::Config>();
    let vertical_tabbar_width = (window.bounds().size.width * VERTICAL_TABBAR_WIDTH_RATIO)
      .max(px(config.tab.get_vertical_tabbar_min_width()))
      .min(px(config.tab.get_vertical_tabbar_max_width()));

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
      last_known_ctrl_state: false,
      rename_dialog: None,
      _rename_dialog_subscription: None,
      close_confirm_dialog: None,
      _close_confirm_subscription: None,
      about_dialog: None,
      _about_dialog_subscription: None,
      import_alacritty_dialog: None,
      _import_alacritty_subscription: None,
      shell_error_dialog: None,
      _shell_error_subscription: None,
      last_notification_time: None,
      tab_bar_visible: true,
      _appearance_subscription: appearance_subscription,
    };

    // Try to restore previous workspace
    let config = cx.global::<::config::Config>();
    if config.window.restore_workspace {
      if let Some(state) = WorkspaceState::load() {
        main_window.restore_workspace(state, window, cx);
        WorkspaceState::delete();
        return main_window;
      }
    }

    main_window.insert_new_tab(window, cx);
    main_window
  }
}

impl Focusable for MainWindow {
  fn focus_handle(&self, _: &gpui::App) -> gpui::FocusHandle {
    self.focus_handle.clone()
  }
}
