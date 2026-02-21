use std::sync::atomic::AtomicUsize;

use gpui::*;
use gpui_component::Size;

use crate::components::about_dialog::AboutDialog;
use crate::components::close_confirm_dialog::CloseConfirmDialog;
use crate::components::search_bar::SearchBar;
use crate::components::tab_rename_dialog::TabRenameDialog;
use crate::components::tab_switcher::TabSwitcher;

pub(crate) use super::main_window_tab_item::TabItem;

pub(crate) const TAB_LABEL_MIN_WIDTH: f32 = 60.0;
pub(crate) const TAB_LABEL_MAX_WIDTH: f32 = 200.0;
pub(crate) const VERTICAL_TABBAR_DEFAULT_WIDTH: f32 = TAB_LABEL_MAX_WIDTH + 24.0;
pub(crate) const VERTICAL_TABBAR_MIN_WIDTH: f32 = TAB_LABEL_MIN_WIDTH + 24.0;

pub struct MainWindow {
  pub(crate) focus_handle: FocusHandle,
  pub(crate) active_tab_ix: Option<usize>,
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
      vertical_tabbar_width: px(VERTICAL_TABBAR_DEFAULT_WIDTH),
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
}

impl Focusable for MainWindow {
  fn focus_handle(&self, _: &gpui::App) -> gpui::FocusHandle {
    self.focus_handle.clone()
  }
}
