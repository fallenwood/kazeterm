use gpui::*;
use gpui_component::{
  Icon, IconName, h_flex,
  menu::{PopupMenu, PopupMenuItem},
};
use themeing::SettingsStore;

use super::main_window::MainWindow;
use super::shell_icon::ShellIcon;

#[allow(clippy::too_many_arguments)]
pub(super) fn build_tab_context_menu(
  menu: PopupMenu,
  view: Entity<MainWindow>,
  tab_index: usize,
  tab_ix: usize,
  is_first: bool,
  is_last: bool,
  total_tabs: usize,
  move_prev_label: &'static str,
  move_prev_icon: IconName,
  move_next_label: &'static str,
  move_next_icon: IconName,
) -> PopupMenu {
  let view_rename = view.clone();
  let view_duplicate = view.clone();
  let view_split_h = view.clone();
  let view_split_v = view.clone();
  let view_close_pane = view.clone();
  let view_focus_next = view.clone();
  let view_focus_prev = view.clone();
  let view_swap_panes = view.clone();
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
    .separator()
    .item(
      PopupMenuItem::new(move_prev_label)
        .icon(move_prev_icon)
        .disabled(is_first)
        .on_click(move |_, _window, cx| {
          view_move_left.update(cx, |this, cx| {
            this.move_tab_left(tab_ix, cx);
          });
        }),
    )
    .item(
      PopupMenuItem::new(move_next_label)
        .icon(move_next_icon)
        .disabled(is_last)
        .on_click(move |_, _window, cx| {
          view_move_right.update(cx, |this, cx| {
            this.move_tab_right(tab_ix, cx);
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
            this.close_other_tabs(tab_index, cx);
          });
        }),
    )
    .item(
      PopupMenuItem::new("Close Tabs to Right")
        .icon(IconName::Close)
        .disabled(is_last)
        .on_click(move |_, _window, cx| {
          view_close_right.update(cx, |this, cx| {
            this.close_tabs_to_right(tab_ix, cx);
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

pub(super) fn build_new_tab_menu(
  mut menu: PopupMenu,
  view: Entity<MainWindow>,
  local_profiles: &[(String, String)],
  container_profiles: &[(String, String)],
  ssh_hosts: &[String],
  profile_shortcuts: &[String],
) -> PopupMenu {
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

  // Config & About
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

  menu = menu.separator();
  let view_import = view.clone();
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
