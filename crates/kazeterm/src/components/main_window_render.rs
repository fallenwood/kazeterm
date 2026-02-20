use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::{
  Icon, IconName, Sizable, StyledExt, TitleBar,
  button::{Button, ButtonVariants},
  h_flex,
  label::Label,
  menu::{ContextMenuExt, DropdownMenu, PopupMenu, PopupMenuItem},
};
use themeing::SettingsStore;

use super::main_window::MainWindow;
use super::main_window::TAB_LABEL_MAX_WIDTH;
use super::terminal_tab_bar::{TerminalTab, TerminalTabBar};
use crate::components::{dragged_tab::{DraggedTab, DraggedTabView}, main_window::TAB_LABEL_MIN_WIDTH};
use crate::components::shell_icon::ShellIcon;
use crate::components::tab_button::{TabButton, TabButtonClickEvent};

impl Render for MainWindow {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let search_visible = self.search_visible;
    let search_bar = self.search_bar.clone();
    let config = cx.global::<::config::Config>();
    let vertical_tabs = config.vertical_tabs;
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
      let vertical_tabs = vertical_tabs;
      cx.spawn(async move |_this, cx| {
        // Small delay to allow layout to complete
        // smol::Timer::after(std::time::Duration::from_millis(50)).await;
        cx.update(|_cx| {
          let max_offset = scroll_handle.max_offset();
          let offset = if vertical_tabs {
            gpui::point(px(0.0), -max_offset.height)
          } else {
            gpui::point(-max_offset.width, px(0.0))
          };
          scroll_handle.set_offset(offset);
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
      let vertical_tabs = vertical_tabs;
      cx.spawn(async move |_this, cx| {
        cx.update(|_cx| {
          if total_tabs > 0 && active_tab_ix < total_tabs {
            // Calculate the approximate position of the active tab
            // This is a simple approach - scroll proportionally based on tab index
            let max_offset = scroll_handle.max_offset();
            let scroll_ratio = active_tab_ix as f32 / total_tabs.max(1) as f32;
            let offset = if vertical_tabs {
              gpui::point(px(0.0), -max_offset.height * scroll_ratio)
            } else {
              gpui::point(-max_offset.width * scroll_ratio, px(0.0))
            };
            scroll_handle.set_offset(offset);
          }
        })
        .ok();
      })
      .detach();
    }

    let view = cx.entity();
    let menu_view = view.clone();

    let colors = setting_store.theme().colors().clone();

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
        } else if e.keystroke.modifiers.shift
          && e.keystroke.modifiers.control
          && e.keystroke.key == "d"
        {
          this.split_pane_horizontal(window, cx);
        } else if e.keystroke.modifiers.shift
          && e.keystroke.modifiers.control
          && e.keystroke.key == "e"
        {
          this.split_pane_vertical(window, cx);
        } else if e.keystroke.modifiers.shift
          && e.keystroke.modifiers.control
          && e.keystroke.key == "w"
        {
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
              .h_flex()
              .flex_1()
              .flex_basis(px(0.0))
              .min_w_0()
              .overflow_x_hidden()
              .when(!vertical_tabs, |this| {
                this.child(
                  TerminalTabBar::new("tabs")
                    .track_scroll(&self.tab_scroll_handle)
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
                        let has_bell = item
                          .split_container
                          .all_terminals()
                          .iter()
                          .any(|(_, t)| t.read(cx).has_bell());
                        let view = cx.entity();
                        let view_for_click = view.clone();
                        let all_terminals = item.split_container.all_terminals();
                        // Define colors for selected tab highlight
                        let selected_bg: gpui::Hsla = colors.tab_active_background;
                        let normal_bg = colors.tab_inactive_background;
                        let hover_bg = colors.element_hover;
                        let text_color = colors.text;
                        let text_muted = colors.text_muted;
                        let accent_color = colors.text_accent;
                        let warning_color = colors.terminal_ansi_yellow;

                        TerminalTab::new()
                          .selected(is_selected)
                          .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                            // Select this tab
                            view_for_click.update(cx, |this, cx| {
                              this.set_active_tab(tab_ix, window, cx);
                            });
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
                                      .mt_1()
                                      .h_full()
                                      .gap_1p5()
                                      .pl_2p5()
                                      .pr_1()
                                      .items_center()
                                      .min_w(px(TAB_LABEL_MIN_WIDTH))
                                      .max_w(px(TAB_LABEL_MAX_WIDTH))
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
                                          let view_duplicate = view.clone();
                                          let view_split_h = view.clone();
                                          let view_split_v = view.clone();
                                          let view_close_pane = view.clone();
                                          let view_move_left = view.clone();
                                          let view_move_right = view.clone();
                                          let view_close_others = view.clone();
                                          let view_close_right = view.clone();
                                          let view_close_tab = view.clone();
                                          menu
                                            .item(
                                              PopupMenuItem::new("Rename Tab")
                                                .icon(Icon::empty().path("icons/pencil.svg"))
                                                .on_click(move |_, window, cx| {
                                                  view_rename.update(cx, |this, cx| {
                                                    this.show_rename_dialog(tab_index, window, cx);
                                                  });
                                                }),
                                            )
                                            .item(
                                              PopupMenuItem::new("Duplicate Tab")
                                                .icon(Icon::empty().path("icons/copy.svg"))
                                                .on_click(move |_, window, cx| {
                                                  view_duplicate.update(cx, |this, cx| {
                                                    this.duplicate_tab(tab_index, window, cx);
                                                  });
                                                }),
                                            )
                                            .separator()
                                            .item(
                                              PopupMenuItem::new("Split Horizontal (Ctrl+Shift+D)")
                                                .icon(Icon::empty().path("icons/columns-2.svg"))
                                                .on_click(move |_, window, cx| {
                                                  view_split_h.update(cx, |this, cx| {
                                                    this.split_pane_horizontal(window, cx);
                                                  });
                                                }),
                                            )
                                            .item(
                                              PopupMenuItem::new("Split Vertical (Ctrl+Shift+E)")
                                                .icon(Icon::empty().path("icons/rows-2.svg"))
                                                .on_click(move |_, window, cx| {
                                                  view_split_v.update(cx, |this, cx| {
                                                    this.split_pane_vertical(window, cx);
                                                  });
                                                }),
                                            )
                                            .item(
                                              PopupMenuItem::new("Close Pane (Ctrl+Shift+W)")
                                                .icon(IconName::Close)
                                                .on_click(move |_, window, cx| {
                                                  view_close_pane.update(cx, |this, cx| {
                                                    this.close_active_pane(window, cx);
                                                  });
                                                }),
                                            )
                                            .separator()
                                            .item(
                                              PopupMenuItem::new("Move Left")
                                                .icon(IconName::ArrowLeft)
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
                                                .icon(IconName::ArrowRight)
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
                                                .icon(IconName::Close)
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
                                                .icon(IconName::Close)
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
                                            .item(
                                              PopupMenuItem::new("Close Tab")
                                                .icon(IconName::Close)
                                                .on_click(move |_, window, cx| {
                                                  view_close_tab.update(cx, |this, cx| {
                                                    this.remove_tab_by(tab_index, window, cx);
                                                  });
                                                }),
                                            )
                                        }
                                      }),
                                  ),
                              ),
                          )
                      })
                        .collect::<Vec<_>>(),
                    ),
                )
              })
              .child(
                h_flex()
                  .flex_shrink_0()
                  .gap_0()
                  .pl_1()
                  .child(
                    Button::new("new")
                      .ghost()
                      .small()
                      .icon(IconName::Plus)
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
                      .icon(IconName::ChevronDown)
                      .dropdown_menu({
                        let view_about = menu_view.clone();
                        move |menu: PopupMenu,
                              _window: &mut Window,
                              _cx: &mut Context<PopupMenu>| {
                          let mut menu = menu;

                          // Local profiles
                          for (name, shell_path) in local_profiles.iter() {
                            let profile_name = name.clone();
                            let shell_path = shell_path.clone();
                            let display_name = name.clone();
                            let view_clone = menu_view.clone();
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
                              .on_click(
                                move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                                  view_clone.update(cx, |this, cx| {
                                    this.insert_new_tab_with_profile(
                                      Some(&profile_name),
                                      None,
                                      window,
                                      cx,
                                    );
                                  });
                                },
                              ),
                            );
                          }

                          // Container profiles
                          if !container_profiles.is_empty() {
                            menu = menu.separator();
                            for (name, shell_path) in container_profiles.iter() {
                              let profile_name = name.clone();
                              let shell_path = shell_path.clone();
                              let display_name = name.clone();
                              let view_clone = menu_view.clone();
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
                                .on_click(
                                  move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                                    view_clone.update(cx, |this, cx| {
                                      this.insert_new_tab_with_profile(
                                        Some(&profile_name),
                                        None,
                                        window,
                                        cx,
                                      );
                                    });
                                  },
                                ),
                              );
                            }
                          }

                          // SSH Hosts
                          if !ssh_hosts.is_empty() {
                            menu = menu.separator();
                            for name in ssh_hosts.iter() {
                              let profile_name = name.clone();
                              let display_name = format!("[ssh] {}", name);
                              let view_clone = menu_view.clone();
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
                                .on_click(
                                  move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                                    view_clone.update(cx, |this, cx| {
                                      this.insert_new_tab_with_profile(
                                        Some(&profile_name),
                                        None,
                                        window,
                                        cx,
                                      );
                                    });
                                  },
                                ),
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
                            .on_click(
                              move |_: &ClickEvent, _window: &mut Window, cx: &mut App| {
                                let config_path = ::config::Config::get_config_path();
                                cx.open_url(&format!("file://{}", config_path.display()));
                              },
                            ),
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
                            .on_click(
                              |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                                if let Some(path) = ::config::Config::get_config_file_path() {
                                  cx.open_url(&format!("file://{}", path.display()));
                                }
                              },
                            ),
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
                            .on_click(
                              move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                                view_about.update(cx, |this, cx| {
                                  this.show_about_dialog(window, cx);
                                });
                              },
                            ),
                          );

                          menu
                        }
                      }),
                  ),
              ),
          ),
      )
      .child({
        let active_ix = self.active_tab_ix.unwrap_or_default();
        let content = div()
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
          });

        if vertical_tabs {
          h_flex()
            .flex_1()
            .size_full()
            .child(
              div()
                .h_full()
                .flex_shrink_0()
                .w(px(TAB_LABEL_MAX_WIDTH + 24.0))
                .p_1()
                .child(
                  TerminalTabBar::new("tabs-vertical")
                    .vertical(true)
                    .track_scroll(&self.tab_scroll_handle)
                    .children(
                      self
                        .items
                        .iter()
                        .enumerate()
                        .map(|(tab_ix, item)| {
                          let shell_icon = ShellIcon::new(&item.shell_path);
                          let tab_index = item.index;
                          let tab_title = item.display_title().to_string();
                          let is_selected = self.active_tab_ix == Some(tab_ix);
                          let has_bell = item
                            .split_container
                            .all_terminals()
                            .iter()
                            .any(|(_, t)| t.read(cx).has_bell());
                          let view_for_click = view.clone();
                          let all_terminals = item.split_container.all_terminals();
                          let selected_bg: gpui::Hsla = colors.tab_active_background;
                          let normal_bg = colors.tab_inactive_background;
                          let hover_bg = colors.element_hover;
                          let text_color = colors.text;
                          let text_muted = colors.text_muted;
                          let accent_color = colors.text_accent;
                          let warning_color = colors.terminal_ansi_yellow;

                          TerminalTab::new()
                            .selected(is_selected)
                            .fill_height(false)
                            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                              view_for_click.update(cx, |this, cx| {
                                this.set_active_tab(tab_ix, window, cx);
                              });
                              for (_, terminal) in &all_terminals {
                                terminal.update(cx, |terminal_view, cx| {
                                  terminal_view.clear_bell(cx);
                                });
                              }
                              cx.stop_propagation();
                            })
                            .child(
                              h_flex()
                                .w_full()
                                .gap_1p5()
                                .pl_2p5()
                                .pr_1()
                                .py_1()
                                .items_center()
                                .min_w(px(TAB_LABEL_MIN_WIDTH))
                                .max_w(px(TAB_LABEL_MAX_WIDTH))
                                .when(is_selected, |this| {
                                  this.bg(selected_bg).border_l_2().border_color(accent_color)
                                })
                                .when(!is_selected, |this| {
                                  this.bg(normal_bg).hover(|style| style.bg(hover_bg))
                                })
                                .rounded_md()
                                .child(
                                  div()
                                    .flex_shrink_0()
                                    .child(shell_icon.into_element(px(14.0))),
                                )
                                .when(has_bell, |this| {
                                  this.child(
                                    div().flex_shrink_0().child(
                                      Icon::new(IconName::Bell)
                                        .size_3()
                                        .text_color(warning_color),
                                    ),
                                  )
                                })
                                .child(
                                  div().flex_1().min_w_0().overflow_x_hidden().child(
                                    Label::new(tab_title.clone())
                                      .text_color(if is_selected { text_color } else { text_muted })
                                      .whitespace_nowrap(),
                                  ),
                                )
                                .child(
                                  TabButton::new("close-vertical", tab_index)
                                    .visible(true)
                                    .on_click(cx.listener(
                                      |this, e: &TabButtonClickEvent, window, cx| {
                                        this.remove_tab_by(e.index, window, cx);
                                      },
                                    )),
                                ),
                            )
                        })
                        .collect::<Vec<_>>(),
                    ),
                ),
            )
            .child(content)
            .into_any_element()
        } else {
          content.into_any_element()
        }
      })
  }
}
