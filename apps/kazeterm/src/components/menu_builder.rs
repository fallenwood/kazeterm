use gpui::*;
use gpui_kit::component::{
  Icon, IconName, h_flex,
  menu::{PopupMenu, PopupMenuItem},
};
use kazeterm_ui_tree::node::{TabGroupColor, TabGroupNode};
use themeing::SettingsStore;

use super::main_window::MainWindow;
use super::shell_icon::ShellIcon;

#[cfg(test)]
#[path = "menu_builder_tests.rs"]
mod tests;

pub(super) fn scrollable_menu(
  menu: PopupMenu,
  window: &mut Window,
  cx: &mut Context<PopupMenu>,
) -> PopupMenu {
  let viewport = window.viewport_size();
  // Leave room for the popup's window-edge margins and border.
  let max_height = (viewport.height - px(20.0)).max(Pixels::ZERO);

  // PopupMenu fixes explicit sizes at creation; dismiss on resize to avoid stale bounds.
  let bounds_subscription = cx.observe_window_bounds(window, move |menu, window, cx| {
    if window.viewport_size() != viewport {
      let focus = menu.focus_handle(cx);
      window.defer(cx, move |window, cx| {
        if focus.contains_focused(window, cx) {
          focus.dispatch_action(&gpui_kit::base::actions::Cancel, window, cx);
        }
      });
    }
  });
  cx.on_release(move |_, _| drop(bounds_subscription))
    .detach();

  menu.scrollable(true).max_h(max_height)
}

fn group_color_icon(color: TabGroupColor, cx: &Context<PopupMenu>) -> Icon {
  let colors = cx.global::<SettingsStore>().theme().colors();
  let color = match color {
    TabGroupColor::Blue => colors.terminal_ansi_blue,
    TabGroupColor::Green => colors.terminal_ansi_green,
    TabGroupColor::Yellow => colors.terminal_ansi_yellow,
    TabGroupColor::Red => colors.terminal_ansi_red,
    TabGroupColor::Purple => colors.terminal_ansi_magenta,
    TabGroupColor::Cyan => colors.terminal_ansi_cyan,
  };
  Icon::empty().path("icons/circle.svg").text_color(color)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn build_tab_context_menu(
  menu: PopupMenu,
  view: Entity<MainWindow>,
  tab_index: usize,
  tab_ix: usize,
  is_pinned: bool,
  is_first: bool,
  is_last: bool,
  can_close_other_tabs: bool,
  can_close_tabs_to_right: bool,
  move_prev_label: &'static str,
  move_prev_icon: IconName,
  move_next_label: &'static str,
  move_next_icon: IconName,
  has_hidden_panes: bool,
  can_toggle_hidden_panes: bool,
  window: &mut Window,
  cx: &mut Context<PopupMenu>,
) -> PopupMenu {
  let view_rename = view.clone();
  let view_duplicate = view.clone();
  let view_toggle_pin = view.clone();
  let view_split_h = view.clone();
  let view_split_v = view.clone();
  let view_close_pane = view.clone();
  let view_focus_next = view.clone();
  let view_focus_prev = view.clone();
  let view_swap_panes = view.clone();
  let view_toggle_hidden = view.clone();
  let view_move_left = view.clone();
  let view_move_right = view.clone();
  let view_close_others = view.clone();
  let view_close_right = view.clone();
  let view_close_tab = view.clone();
  let toggle_hidden_panes_label = if has_hidden_panes {
    "Restore Hidden Panes"
  } else {
    "Hide Other Panes"
  };
  let pin_tab_label = if is_pinned { "Unpin Tab" } else { "Pin Tab" };
  let (current_group, groups) = {
    let main = view.read(cx);
    (
      main
        .items
        .iter()
        .find(|item| item.index == tab_index)
        .and_then(|item| item.group_id.clone()),
      main.groups.clone(),
    )
  };
  let mut menu = scrollable_menu(menu, window, cx);
  if !is_pinned {
    if current_group.is_none() {
      let target = view.clone();
      menu = menu.item(
        PopupMenuItem::new("Create Group")
          .icon(Icon::empty().path("icons/folder.svg"))
          .on_click(move |_, window, cx| {
            target.update(cx, |this, cx| this.create_tab_group(tab_index, window, cx));
          }),
      );
    } else {
      let target = view.clone();
      menu = menu.item(
        PopupMenuItem::new("Remove from Group")
          .icon(Icon::empty().path("icons/minus.svg"))
          .on_click(move |_, window, cx| {
            target.update(cx, |this, cx| this.ungroup_tab(tab_index, window, cx));
          }),
      );
    }
    if groups.iter().any(|g| Some(&g.id) != current_group.as_ref()) {
      let target = view.clone();
      menu = menu.submenu_with_icon(
        Some(Icon::empty().path("icons/folder.svg")),
        "Move to Group",
        window,
        cx,
        move |mut submenu, _, cx| {
          for group in &groups {
            if Some(&group.id) == current_group.as_ref() {
              continue;
            }
            let id = group.id.clone();
            let title = group.display_name();
            let icon = group_color_icon(group.color, cx);
            let target = target.clone();
            submenu = submenu.item(PopupMenuItem::new(title).icon(icon).on_click(
              move |_, window, cx| {
                target.update(cx, |this, cx| {
                  this.move_tab_to_group(tab_index, id.clone(), window, cx)
                });
              },
            ));
          }
          submenu
        },
      );
    }
    menu = menu.separator();
  }

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
    .item(
      PopupMenuItem::new(pin_tab_label)
        .icon(Icon::empty().path("icons/pin.svg"))
        .on_click(move |_, window, cx| {
          view_toggle_pin.update(cx, |this, cx| {
            this.set_tab_pinned(tab_index, !is_pinned, window, cx);
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
    .item(
      PopupMenuItem::new("Focus Next Pane")
        .icon(IconName::ArrowRight)
        .on_click(move |_, window, cx| {
          view_focus_next.update(cx, |this, cx| {
            this.focus_next_pane(window, cx);
          });
        }),
    )
    .item(
      PopupMenuItem::new("Focus Previous Pane")
        .icon(IconName::ArrowLeft)
        .on_click(move |_, window, cx| {
          view_focus_prev.update(cx, |this, cx| {
            this.focus_prev_pane(window, cx);
          });
        }),
    )
    .item(
      PopupMenuItem::new("Swap Panes")
        .icon(Icon::empty().path("icons/arrow-left-right.svg"))
        .on_click(move |_, window, cx| {
          view_swap_panes.update(cx, |this, cx| {
            this.swap_split_panes(window, cx);
          });
        }),
    )
    .item(
      PopupMenuItem::new(toggle_hidden_panes_label)
        .icon(if has_hidden_panes {
          IconName::Undo
        } else {
          IconName::Maximize
        })
        .disabled(!can_toggle_hidden_panes)
        .on_click(move |_, window, cx| {
          view_toggle_hidden.update(cx, |this, cx| {
            this.toggle_hidden_split_panes(window, cx);
          });
        }),
    )
    .separator()
    .item(
      PopupMenuItem::new(move_prev_label)
        .icon(move_prev_icon)
        .disabled(is_first)
        .on_click(move |_, window, cx| {
          view_move_left.update(cx, |this, cx| {
            this.move_tab_left(tab_ix, window, cx);
          });
        }),
    )
    .item(
      PopupMenuItem::new(move_next_label)
        .icon(move_next_icon)
        .disabled(is_last)
        .on_click(move |_, window, cx| {
          view_move_right.update(cx, |this, cx| {
            this.move_tab_right(tab_ix, window, cx);
          });
        }),
    )
    .separator()
    .item(
      PopupMenuItem::new("Close Other Tabs")
        .icon(IconName::Close)
        .disabled(!can_close_other_tabs)
        .on_click(move |_, window, cx| {
          view_close_others.update(cx, |this, cx| {
            this.close_other_tabs(tab_index, window, cx);
          });
        }),
    )
    .item(
      PopupMenuItem::new("Close Tabs to Right")
        .icon(IconName::Close)
        .disabled(!can_close_tabs_to_right)
        .on_click(move |_, window, cx| {
          view_close_right.update(cx, |this, cx| {
            this.close_tabs_to_right(tab_ix, window, cx);
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

pub(super) fn build_group_context_menu(
  menu: PopupMenu,
  view: Entity<MainWindow>,
  group: TabGroupNode,
  window: &mut Window,
  cx: &mut Context<PopupMenu>,
) -> PopupMenu {
  let rename = view.clone();
  let delete = view.clone();
  let id = group.id.clone();
  let color_group_id = group.id.clone();
  let delete_id = group.id;
  scrollable_menu(menu, window, cx)
    .item(
      PopupMenuItem::new("Rename Group")
        .icon(Icon::empty().path("icons/pencil.svg"))
        .on_click(move |_, window, cx| {
          rename.update(cx, |this, cx| {
            this.show_group_rename(id.clone(), window, cx)
          });
        }),
    )
    .submenu_with_icon(
      Some(Icon::empty().path("icons/palette.svg")),
      "Group Color",
      window,
      cx,
      move |mut menu, _, cx| {
        for (name, color) in [
          ("Blue", TabGroupColor::Blue),
          ("Green", TabGroupColor::Green),
          ("Yellow", TabGroupColor::Yellow),
          ("Red", TabGroupColor::Red),
          ("Purple", TabGroupColor::Purple),
          ("Cyan", TabGroupColor::Cyan),
        ] {
          let view = view.clone();
          let id = color_group_id.clone();
          let icon = group_color_icon(color, cx);
          menu = menu.item(
            PopupMenuItem::new(name)
              .icon(icon)
              .on_click(move |_, window, cx| {
                view.update(cx, |this, cx| {
                  this.set_group_color(id.clone(), color, window, cx)
                });
              }),
          );
        }
        menu
      },
    )
    .item(
      PopupMenuItem::new("Delete Group…")
        .icon(Icon::empty().path("icons/delete.svg"))
        .on_click(move |_, window, cx| {
          delete.update(cx, |this, cx| {
            this.show_group_delete(delete_id.clone(), window, cx)
          });
        }),
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn build_new_tab_menu(
  menu: PopupMenu,
  view: Entity<MainWindow>,
  local_profiles: &[(String, String)],
  container_profiles: &[(String, String)],
  ssh_hosts: &[String],
  profile_shortcuts: &[String],
  window: &mut Window,
  cx: &mut Context<PopupMenu>,
) -> PopupMenu {
  let mut menu = scrollable_menu(menu, window, cx);

  // Local profiles
  for (idx, (name, shell_path)) in local_profiles.iter().enumerate() {
    let profile_name = name.clone();
    let shell_path = shell_path.clone();
    let display_name = name.clone();
    let shortcut_text = profile_shortcuts.get(idx).cloned().unwrap_or_default();
    let view_clone = view.clone();
    menu = menu.item(
      PopupMenuItem::element(move |_window, cx| {
        let colors = cx.global::<SettingsStore>().theme().colors();
        let shell_icon = ShellIcon::new(&shell_path);
        let mut row = h_flex()
          .w_full()
          .gap_2()
          .items_center()
          .justify_between()
          .child(
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
              .child(display_name.clone()),
          );
        if !shortcut_text.is_empty() {
          row = row.child(
            div()
              .pl_4()
              .text_color(colors.text_muted)
              .text_size(px(11.0))
              .child(shortcut_text.clone()),
          );
        }
        row.into_any_element()
      })
      .on_click(move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
        view_clone.update(cx, |this, cx| {
          this.insert_new_tab_with_profile(Some(&profile_name), None, window, cx);
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
            this.insert_new_tab_with_profile(Some(&profile_name), None, window, cx);
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
            this.insert_new_tab_with_profile(Some(&profile_name), None, window, cx);
          });
        }),
      );
    }
  }

  // Settings & config
  menu = menu.separator();
  let view_settings = view.clone();
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
            .child(Icon::empty().path("icons/settings.svg").size_4()),
        )
        .child("Settings")
        .into_any_element()
    })
    .on_click(move |_, window, cx| {
      view_settings.update(cx, |this, cx| {
        this.show_settings_page(window, cx);
      });
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

  menu = menu.separator();
  let view_import = view.clone();
  let view_dump_ui_tree = view.clone();
  let view_load_ui_tree = view.clone();
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
        .child("Import Alacritty Config")
        .into_any_element()
    })
    .on_click(move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
      view_import.update(cx, |this, cx| {
        this.show_import_alacritty_dialog(window, cx);
      });
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
            .child(Icon::new(IconName::ArrowDown).size_4()),
        )
        .child("Dump UI Tree JSON")
        .into_any_element()
    })
    .on_click(move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
      view_dump_ui_tree.update(cx, |this, cx| {
        this.prompt_dump_ui_tree_path(window, cx);
      });
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
            .child(Icon::new(IconName::ArrowUp).size_4()),
        )
        .child("Load UI Tree JSON")
        .into_any_element()
    })
    .on_click(move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
      view_load_ui_tree.update(cx, |this, cx| {
        this.prompt_load_ui_tree_path(window, cx);
      });
    }),
  );

  menu = menu.separator();
  let view_about = view.clone();
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
