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

use crate::components::dragged_tab::{DraggedTab, DraggedTabView};
use crate::components::search_bar::{SearchBar, SearchBarCloseEvent};
use crate::components::shell_icon::ShellIcon;
use crate::components::tab_button::{TabButton, TabButtonClickEvent};

pub struct TabItem {
  index: usize,
  title: String,
  shell_path: String,
  _shell_name: String,
  terminal: Entity<terminal::TerminalView>,
  _subscription: gpui::Subscription,
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
}

impl MainWindow {
  pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
    cx.new(|cx| Self::new(window, cx))
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
      let terminal = item.terminal.clone();
      self.search_bar.update(cx, |search_bar, _cx| {
        search_bar.set_terminal_view(terminal.clone());
      });

      // Focus the terminal
      window.focus(&terminal.focus_handle(cx));
    }

    cx.notify();
  }

  fn toggle_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    self.search_visible = !self.search_visible;
    if self.search_visible {
      if let Some(active_ix) = self.active_tab_ix {
        if let Some(item) = self.items.get(active_ix) {
          let terminal = item.terminal.clone();
          self.search_bar.update(cx, |search_bar, _cx| {
            search_bar.set_terminal_view(terminal);
          });
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
          let terminal = item.terminal.clone();
          window.focus(&terminal.focus_handle(cx));
        }
      }
    }

    cx.notify();
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

    let (shell_path, tab_title, shell_name, working_directory) = if let Some(name) = profile_name {
      let profile = config.get_profile(name);
      let shell: String = profile
        .map(|e| e.shell.clone())
        .unwrap_or_else(|| config.get_shell());
      let working_directory =
        working_directory.or(profile.map(|e| e.working_directory.clone()).flatten());
      let shell_name = std::path::Path::new(&shell)
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or(&shell)
        .to_lowercase();
      let working_directory = get_working_directory_pathbuf(working_directory);

      (shell, name.to_string(), shell_name, working_directory)
    } else {
      let shell = config.get_shell().clone();
      let shell_name = std::path::Path::new(&shell)
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or(&shell)
        .to_lowercase();
      let working_directory = get_working_directory_pathbuf(working_directory);

      (shell, shell_name.clone(), shell_name, working_directory)
    };

    let terminal = super::terminal_window::new_terminal_window_with_shell(
      window,
      index,
      &shell_path,
      working_directory,
      cx,
    );
    let subscription = cx.subscribe_in(&terminal, window, Self::subscribe_terminal_view_event);

    let item = TabItem {
      index,
      title: tab_title,
      shell_path,
      _shell_name: shell_name,
      terminal: terminal.clone(),
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
      terminal::TerminalEvent::CloseTerminal(tab_index) => {
        this.remove_tab_by(*tab_index, window, cx);
        cx.notify();
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
        // Update tab title
        let tab_index = terminal_view.read(cx).index;
        if let Some(item) = this.items.iter_mut().find(|item| item.index == tab_index) {
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
}

impl Focusable for MainWindow {
  fn focus_handle(&self, _: &gpui::App) -> gpui::FocusHandle {
    self.focus_handle.clone()
  }
}

impl Render for MainWindow {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let search_visible = self.search_visible;
    let search_bar = self.search_bar.clone();
    let config = cx.global::<::config::Config>();
    let setting_store = cx.global::<SettingsStore>();
    let profile_names: Vec<String> = config
      .get_profile_names()
      .iter()
      .map(|s| s.to_string())
      .collect();

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

    let view = cx.entity();

    let theme = setting_store.theme();
    let colors = theme.colors();

    div()
      .flex()
      .flex_col()
      .size_full()
      .key_context("MainWindow")
      .on_key_down(cx.listener(move |this, e: &KeyDownEvent, window, cx| {
        if e.keystroke.modifiers.shift && e.keystroke.modifiers.control && e.keystroke.key == "f" {
          this.toggle_search(window, cx);
        } else if e.keystroke.key == "Escape" && this.search_visible {
          this.toggle_search(window, cx);
        }
      }))
      .child(
        TitleBar::new()
          .on_close_window(|_: &ClickEvent, window: &mut Window, _cx: &mut App| {
            window.remove_window();
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
                        let tab_title = item.title.clone();
                        let total_tabs = self.items.len();
                        let is_first = tab_ix == 0;
                        let is_last = tab_ix == total_tabs - 1;
                        let is_selected = self.active_tab_ix == Some(tab_ix);
                        let has_bell = item.terminal.read(cx).has_bell();
                        let view = cx.entity();
                        let terminal_for_click = item.terminal.clone();
                        // Define colors for selected tab highlight
                        let selected_bg: gpui::Hsla = colors.tab_active_background;
                        let normal_bg = colors.tab_inactive_background;
                        Tab::new()
                          .selected(is_selected)
                          .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            // Clear bell when clicking on tab
                            terminal_for_click.update(cx, |terminal_view, cx| {
                              terminal_view.clear_bell(cx);
                            });
                            // Prevent TitleBar from starting window drag when clicking on tabs
                            cx.stop_propagation();
                          })
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
                                  cx.new(|_cx| DraggedTabView::new(dragged.title.clone(), dragged.shell_path.clone()))
                                },
                              )
                              .drag_over::<DraggedTab>(move |style, _dragged, _window, _cx| {
                                style.bg(selected_bg.blend(gpui::black().opacity(0.1)))
                              })
                              .on_drop(cx.listener(move |this, dragged: &DraggedTab, _window, cx| {
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
                              }))
                              .child(
                                h_flex()
                                  .gap_2()
                                  .px_2()
                                  .py_1()
                                  .items_center()
                                  .border_color(colors.border)
                                  .bg(if is_selected { selected_bg } else { normal_bg })
                                  .rounded_md()
                                  .child(shell_icon.into_element(px(14.0)))
                                  .when(has_bell, |this| {
                                    this
                                      .child(Icon::new(IconName::Bell).size_3().text_color(colors.text))
                                  })
                                  .child(Label::new(tab_title.clone()).text_color(colors.text))
                                  .child(TabButton::new("close", tab_index).on_click(cx.listener(
                                    |this, e: &TabButtonClickEvent, window, cx| {
                                      let tab_index = e.index;
                                      this.remove_tab_by(tab_index, window, cx);
                                    },
                                  )))
                                  .on_mouse_down(MouseButton::Right, |_, _, cx| {
                                    cx.stop_propagation();
                                  })
                                  .context_menu({
                                let view = view.clone();
                                move |menu, _window, _cx| {
                                  let view_move_left = view.clone();
                                  let view_move_right = view.clone();
                                  let view_close_others = view.clone();
                                  let view_close_right = view.clone();
                                  let view_close_tab = view.clone();
                                  menu
                                    .item(
                                      PopupMenuItem::new("Move Left").disabled(is_first).on_click(
                                        move |_, _window, cx| {
                                          view_move_left.update(cx, |this, cx| {
                                            if tab_ix > 0 {
                                              this.items.swap(tab_ix, tab_ix - 1);
                                              this.active_tab_ix = Some(tab_ix - 1);
                                              cx.notify();
                                            }
                                          });
                                        },
                                      ),
                                    )
                                    .item(
                                      PopupMenuItem::new("Move Right").disabled(is_last).on_click(
                                        move |_, _window, cx| {
                                          view_move_right.update(cx, |this, cx| {
                                            if tab_ix + 1 < this.items.len() {
                                              this.items.swap(tab_ix, tab_ix + 1);
                                              this.active_tab_ix = Some(tab_ix + 1);
                                              cx.notify();
                                            }
                                          });
                                        },
                                      ),
                                    )
                                    .separator()
                                    .item(
                                      PopupMenuItem::new("Close Other Tabs")
                                        .disabled(total_tabs <= 1)
                                        .on_click(move |_, _window, cx| {
                                          view_close_others.update(cx, |this, cx| {
                                            let keep_index = tab_index;
                                            this.items.retain(|tab| tab.index == keep_index);
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
                              )
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
                  .dropdown_menu(
                    move |menu: PopupMenu, _window: &mut Window, _cx: &mut Context<PopupMenu>| {
                      let mut menu = menu;

                      for name in profile_names.iter() {
                        let profile_name = name.clone();
                        let view_clone = view.clone();
                        menu = menu.item(PopupMenuItem::new(name.clone()).on_click(
                          move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                            view_clone.update(cx, |this, cx| {
                              this.insert_new_tab_with_profile(Some(&profile_name), None, window, cx);
                            });
                          },
                        ));
                      }

                      menu = menu.separator();
                      menu = menu.item(PopupMenuItem::new("Open Config").on_click(
                        |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                          if let Some(path) = ::config::Config::get_config_file_path() {
                            cx.open_url(&format!("file://{}", path.display()));
                          }
                        },
                      ));

                      menu
                    },
                  ),
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
              .map(|i| i.terminal.clone().into_any_element())
              .unwrap_or_else(|| {
                eprintln!(
                  "render: NO ITEM FOUND at index {}, showing empty div",
                  active_ix
                );
                div().into_any_element()
              }),
          )
          .when(search_visible, |this| this.child(search_bar))
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
