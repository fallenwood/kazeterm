use config::KeybindingList;
use gpui::*;
use gpui_component::{
  Icon, IconName,
  menu::{PopupMenu, PopupMenuItem},
};
use terminal::TerminalView;

use super::main_window::MainWindow;

/// Format configured keybindings for display in menu items.
fn kb_hint(keybinding: &KeybindingList) -> String {
  keybinding.display_text()
}

pub(super) fn build_terminal_context_menu(
  menu: PopupMenu,
  terminal_view: &Entity<TerminalView>,
  main_window: &Entity<MainWindow>,
  window: &mut Window,
  cx: &mut Context<PopupMenu>,
) -> PopupMenu {
  let kb = cx
    .try_global::<config::Config>()
    .map(|c| c.keybindings.clone())
    .unwrap_or_default();

  let active_tab_ix = main_window.read(cx).active_tab_ix;
  let has_hidden_panes = main_window.read(cx).active_tab_has_hidden_panes();
  let can_toggle_hidden_panes = main_window.read(cx).active_tab_can_toggle_hidden_panes();
  let toggle_hidden_panes_label = if has_hidden_panes {
    "Restore Hidden Panes"
  } else {
    "Hide Other Panes"
  };

  // --- Top-level: Copy & Paste ---
  let tv_copy = terminal_view.clone();
  let tv_paste = terminal_view.clone();
  let copy_hint = kb_hint(&kb.copy);
  let paste_hint = kb_hint(&kb.paste);

  let menu = menu
    .item(
      PopupMenuItem::new(format!("Copy ({})", copy_hint))
        .icon(IconName::Copy)
        .on_click(move |_, _, cx| {
          tv_copy.update(cx, |view, cx| {
            view.terminal.update(cx, |term, cx| {
              term.copy_and_clear_selection(cx);
            });
          });
        }),
    )
    .item(
      PopupMenuItem::new(format!("Paste ({})", paste_hint))
        .icon(Icon::empty().path("icons/clipboard-paste.svg"))
        .on_click(move |_, _, cx| {
          if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            tv_paste.update(cx, |view, cx| {
              view.terminal.update(cx, |term, _| {
                term.input(text.into_bytes());
              });
            });
          }
        }),
    )
    .separator();

  // --- Clear Screen & Search (top-level) ---
  let tv_clear = terminal_view.clone();
  let mw_search = main_window.clone();
  let search_hint = kb_hint(&kb.toggle_search);

  let menu = menu
    .item(
      PopupMenuItem::new("Clear Screen")
        .icon(IconName::Delete)
        .on_click(move |_, _, cx| {
          tv_clear.update(cx, |view, cx| {
            view.terminal.update(cx, |term, _| {
              term.input(b"\x0c".as_slice());
            });
          });
        }),
    )
    .item(
      PopupMenuItem::new(format!("Search ({})", search_hint))
        .icon(IconName::Search)
        .on_click(move |_, window, cx| {
          mw_search.update(cx, |this, cx| {
            this.toggle_search(window, cx);
          });
        }),
    )
    .separator();

  // --- Split Panes submenu ---
  let mw_split_h = main_window.clone();
  let mw_split_v = main_window.clone();
  let mw_close_pane = main_window.clone();
  let mw_focus_next = main_window.clone();
  let mw_focus_prev = main_window.clone();
  let mw_swap = main_window.clone();
  let mw_toggle_hidden = main_window.clone();
  let split_h_hint = kb_hint(&kb.split_horizontal);
  let split_v_hint = kb_hint(&kb.split_vertical);
  let close_pane_hint = kb_hint(&kb.close_pane);
  let focus_next_hint = kb_hint(&kb.focus_next_pane);
  let focus_prev_hint = kb_hint(&kb.focus_previous_pane);
  let swap_hint = kb_hint(&kb.swap_split_panes);
  let toggle_hidden_hint = kb_hint(&kb.toggle_hidden_panes);

  let menu = menu.submenu("Split Panes", window, cx, move |menu, _window, _cx| {
    let mw_split_h = mw_split_h.clone();
    let mw_split_v = mw_split_v.clone();
    let mw_close_pane = mw_close_pane.clone();
    let mw_focus_next = mw_focus_next.clone();
    let mw_focus_prev = mw_focus_prev.clone();
    let mw_swap = mw_swap.clone();
    let mw_toggle_hidden = mw_toggle_hidden.clone();

    menu
      .item(
        PopupMenuItem::new(format!("Split Horizontal ({})", split_h_hint))
          .icon(Icon::empty().path("icons/columns-2.svg"))
          .on_click(move |_, window, cx| {
            mw_split_h.update(cx, |this, cx| {
              this.split_pane_horizontal(window, cx);
            });
          }),
      )
      .item(
        PopupMenuItem::new(format!("Split Vertical ({})", split_v_hint))
          .icon(Icon::empty().path("icons/rows-2.svg"))
          .on_click(move |_, window, cx| {
            mw_split_v.update(cx, |this, cx| {
              this.split_pane_vertical(window, cx);
            });
          }),
      )
      .item(
        PopupMenuItem::new(format!("Focus Next Pane ({})", focus_next_hint))
          .icon(IconName::ArrowRight)
          .on_click(move |_, window, cx| {
            mw_focus_next.update(cx, |this, cx| {
              this.focus_next_pane(window, cx);
            });
          }),
      )
      .item(
        PopupMenuItem::new(format!("Focus Previous Pane ({})", focus_prev_hint))
          .icon(IconName::ArrowLeft)
          .on_click(move |_, window, cx| {
            mw_focus_prev.update(cx, |this, cx| {
              this.focus_prev_pane(window, cx);
            });
          }),
      )
      .item(
        PopupMenuItem::new(format!("Swap Panes ({})", swap_hint))
          .icon(Icon::empty().path("icons/arrow-left-right.svg"))
          .on_click(move |_, window, cx| {
            mw_swap.update(cx, |this, cx| {
              this.swap_split_panes(window, cx);
            });
          }),
      )
      .item(
        PopupMenuItem::new(format!(
          "{} ({})",
          toggle_hidden_panes_label, toggle_hidden_hint
        ))
        .icon(if has_hidden_panes {
          IconName::Undo
        } else {
          IconName::Maximize
        })
        .disabled(!can_toggle_hidden_panes)
        .on_click(move |_, window, cx| {
          mw_toggle_hidden.update(cx, |this, cx| {
            this.toggle_hidden_split_panes(window, cx);
          });
        }),
      )
      .separator()
      .item(
        PopupMenuItem::new(format!("Close Pane ({})", close_pane_hint))
          .icon(IconName::Close)
          .on_click(move |_, window, cx| {
            mw_close_pane.update(cx, |this, cx| {
              this.close_active_pane(window, cx);
            });
          }),
      )
  });

  // --- Tabs submenu ---
  let mw_new_tab = main_window.clone();
  let mw_dup_tab = main_window.clone();
  let mw_rename_tab = main_window.clone();
  let mw_close_tab = main_window.clone();

  let menu = menu.submenu("Tabs", window, cx, move |menu, _window, _cx| {
    let mw_new_tab = mw_new_tab.clone();
    let mw_dup_tab = mw_dup_tab.clone();
    let mw_rename_tab = mw_rename_tab.clone();
    let mw_close_tab = mw_close_tab.clone();

    menu
      .item(
        PopupMenuItem::new("New Tab")
          .icon(IconName::Plus)
          .on_click(move |_, window, cx| {
            mw_new_tab.update(cx, |this, cx| {
              this.insert_new_tab(window, cx);
            });
          }),
      )
      .item(
        PopupMenuItem::new("Duplicate Tab")
          .icon(Icon::empty().path("icons/copy.svg"))
          .disabled(active_tab_ix.is_none())
          .on_click(move |_, window, cx| {
            if let Some(ix) = active_tab_ix {
              mw_dup_tab.update(cx, |this, cx| {
                this.duplicate_tab(ix, window, cx);
              });
            }
          }),
      )
      .item(
        PopupMenuItem::new("Rename Tab")
          .icon(Icon::empty().path("icons/pencil.svg"))
          .disabled(active_tab_ix.is_none())
          .on_click(move |_, window, cx| {
            if let Some(ix) = active_tab_ix {
              mw_rename_tab.update(cx, |this, cx| {
                this.show_rename_dialog(ix, window, cx);
              });
            }
          }),
      )
      .separator()
      .item(
        PopupMenuItem::new("Close Tab")
          .icon(IconName::Close)
          .disabled(active_tab_ix.is_none())
          .on_click(move |_, window, cx| {
            if let Some(ix) = active_tab_ix {
              mw_close_tab.update(cx, |this, cx| {
                this.remove_tab_by(ix, window, cx);
              });
            }
          }),
      )
  });

  // --- Window submenu ---
  let mw_fullscreen = main_window.clone();
  let mw_close_window = main_window.clone();
  let fullscreen_hint = kb_hint(&kb.toggle_fullscreen);
  let zoom_in_hint = kb_hint(&kb.zoom_in);
  let zoom_out_hint = kb_hint(&kb.zoom_out);
  let zoom_reset_hint = kb_hint(&kb.zoom_reset);

  let menu = menu.submenu("Window", window, cx, move |menu, _window, _cx| {
    let mw_fullscreen = mw_fullscreen.clone();
    let mw_close_window = mw_close_window.clone();

    menu
      .item(
        PopupMenuItem::new("New Window")
          .icon(IconName::Plus)
          .on_click(move |_, _, _cx| {
            if let Ok(exe) = std::env::current_exe() {
              let _ = std::process::Command::new(exe).spawn();
            }
          }),
      )
      .separator()
      .item(
        PopupMenuItem::new(format!("Toggle Fullscreen ({})", fullscreen_hint))
          .icon(IconName::Maximize)
          .on_click(move |_, window, cx| {
            mw_fullscreen.update(cx, |_, _| {
              window.toggle_fullscreen();
            });
          }),
      )
      .separator()
      .item(
        PopupMenuItem::new(format!("Zoom In ({})", zoom_in_hint))
          .icon(IconName::Plus)
          .on_click(move |_, _, cx| {
            themeing::ZoomState::update_global(cx, |zoom, _| zoom.zoom_in());
          }),
      )
      .item(
        PopupMenuItem::new(format!("Zoom Out ({})", zoom_out_hint))
          .icon(IconName::Minus)
          .on_click(move |_, _, cx| {
            themeing::ZoomState::update_global(cx, |zoom, _| zoom.zoom_out());
          }),
      )
      .item(
        PopupMenuItem::new(format!("Zoom Reset ({})", zoom_reset_hint))
          .icon(IconName::Undo)
          .on_click(move |_, _, cx| {
            themeing::ZoomState::update_global(cx, |zoom, _| zoom.reset());
          }),
      )
      .separator()
      .item(
        PopupMenuItem::new("Close Window")
          .icon(IconName::Close)
          .on_click(move |_, window, cx| {
            mw_close_window.update(cx, |this, cx| {
              this.show_close_confirm_dialog(window, cx);
            });
          }),
      )
  });

  // --- Configuration submenu ---
  #[allow(unused)]
  let mw_import = main_window.clone();

  let menu = menu.submenu("Configuration", window, cx, move |menu, _window, _cx| {
    #[allow(unused)]
    let mw_import = mw_import.clone();

    menu
      .item(
        PopupMenuItem::new("Open Config Path")
          .icon(IconName::Folder)
          .on_click(move |_, _, cx| {
            let config_path = config::Config::get_config_path();
            cx.open_url(&format!("file://{}", config_path.display()));
          }),
      )
      .item(
        PopupMenuItem::new("Open Config File")
          .icon(IconName::File)
          .on_click(|_, _, cx| {
            if let Some(path) = config::Config::get_config_file_path() {
              cx.open_url(&format!("file://{}", path.display()));
            }
          }),
      )
  });

  menu
}
