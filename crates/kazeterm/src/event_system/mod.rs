//! Kazeterm-specific event handlers built on top of the shared event-system crate.

use gpui::{AnyWindowHandle, App, Context, WeakEntity, Window};
use kazeterm_event_system::EventBus;
use kazeterm_ui_tree::action::UIAction;
use kazeterm_ui_tree::node::{OverlayNode, SplitDirection as TreeSplitDirection};

pub use kazeterm_event_system::{
  AppEvent, EventSourceConfig, JsonEvent, send_event, try_send_event,
};

use crate::components::{MainWindow, PaneFocusDirection};

/// Build the default [`EventBus`] with all built-in Kazeterm handlers registered.
pub fn build_default_event_bus(source_config: EventSourceConfig) -> EventBus<MainWindow> {
  let mut bus: EventBus<MainWindow> = EventBus::new();
  let source_config_for_new_window = source_config;

  bus.subscribe("NewTerminalWithDefaultProfile", |mw, _event, window, cx| {
    dispatch_add_tab_event(mw, None, None, window, cx);
  });

  bus.subscribe("NewTerminalWithProfile", |mw, event, window, cx| {
    if let AppEvent::NewTerminalWithProfile {
      profile_name,
      working_directory,
    } = event
    {
      dispatch_add_tab_event(
        mw,
        Some(profile_name.as_str()),
        working_directory,
        window,
        cx,
      );
    }
  });

  bus.subscribe("CloseActiveTab", |mw, _event, window, cx| {
    let tab_index = mw
      .active_tab_ix
      .and_then(|active_ix| mw.items.get(active_ix))
      .map(|item| item.index);
    if let Some(tab_index) = tab_index {
      dispatch_close_tab_event(mw, tab_index, window, cx);
    }
  });

  bus.subscribe("CloseTab", |mw, event, window, cx| {
    if let AppEvent::CloseTab { tab_index } = event {
      dispatch_close_tab_event(mw, tab_index, window, cx);
    }
  });

  bus.subscribe("NextTab", |mw, _event, window, cx| {
    dispatch_tab_cycle_event(mw, true, window, cx);
  });

  bus.subscribe("PreviousTab", |mw, _event, window, cx| {
    dispatch_tab_cycle_event(mw, false, window, cx);
  });

  bus.subscribe("SwitchToTab", |mw, event, window, cx| {
    if let AppEvent::SwitchToTab { position } = event
      && position < mw.items.len()
    {
      dispatch_activate_tab_event(mw, position, window, cx);
    }
  });

  bus.subscribe("SplitHorizontal", |mw, _event, window, cx| {
    dispatch_split_pane_event(mw, TreeSplitDirection::Horizontal, window, cx);
  });

  bus.subscribe("SplitVertical", |mw, _event, window, cx| {
    dispatch_split_pane_event(mw, TreeSplitDirection::Vertical, window, cx);
  });

  bus.subscribe("CloseActivePane", |mw, _event, window, cx| {
    dispatch_close_active_pane_event(mw, window, cx);
  });

  bus.subscribe("FocusNextPane", |mw, _event, window, cx| {
    dispatch_cycle_pane_focus_event(mw, true, window, cx);
  });

  bus.subscribe("FocusPreviousPane", |mw, _event, window, cx| {
    dispatch_cycle_pane_focus_event(mw, false, window, cx);
  });

  bus.subscribe("FocusPaneUp", |mw, _event, window, cx| {
    dispatch_directional_pane_focus_event(mw, PaneFocusDirection::Up, window, cx);
  });

  bus.subscribe("FocusPaneDown", |mw, _event, window, cx| {
    dispatch_directional_pane_focus_event(mw, PaneFocusDirection::Down, window, cx);
  });

  bus.subscribe("FocusPaneLeft", |mw, _event, window, cx| {
    dispatch_directional_pane_focus_event(mw, PaneFocusDirection::Left, window, cx);
  });

  bus.subscribe("FocusPaneRight", |mw, _event, window, cx| {
    dispatch_directional_pane_focus_event(mw, PaneFocusDirection::Right, window, cx);
  });

  bus.subscribe("SwapSplitPanes", |mw, _event, window, cx| {
    dispatch_swap_panes_event(mw, window, cx);
  });

  bus.subscribe("ToggleSearch", |mw, _event, window, cx| {
    dispatch_toggle_search_event(mw, window, cx);
  });

  bus.subscribe("ToggleFullscreen", |_mw, _event, window, _cx| {
    window.toggle_fullscreen();
  });

  bus.subscribe("ToggleTabBar", |mw, _event, window, cx| {
    dispatch_toggle_tab_bar_event(mw, window, cx);
  });

  bus.subscribe("ShowAboutDialog", |mw, _event, window, cx| {
    dispatch_overlay_event(
      mw,
      OverlayNode::AboutDialog,
      "show about dialog",
      window,
      cx,
    );
  });

  bus.subscribe("ShowImportAlacrittyDialog", |mw, _event, window, cx| {
    dispatch_overlay_event(
      mw,
      OverlayNode::ImportAlacritty {
        path: String::new(),
        error: None,
      },
      "show import Alacritty dialog",
      window,
      cx,
    );
  });

  bus.subscribe("ReloadConfig", |_mw, _event, _window, cx| {
    crate::config_watcher::reload_config_and_theme_from_event(cx);
  });

  bus.subscribe("FocusActiveTerminal", |mw, _event, window, cx| {
    mw.refocus_active_terminal(window, cx);
  });

  bus.subscribe("NewWindow", move |_mw, _event, _window, cx| {
    crate::open_kazeterm_window(source_config_for_new_window.clone(), cx);
  });

  bus.subscribe("Quit", |_mw, _event, _window, cx| {
    cx.quit();
  });

  bus.subscribe("SendTextToTerminal", |mw, event, _window, cx| {
    if let AppEvent::SendTextToTerminal { text } = event
      && let Some(active_ix) = mw.active_tab_ix
      && let Some(item) = mw.items.get(active_ix)
      && let Some(terminal) = item.split_container.get_active_terminal()
    {
      terminal.update(cx, |view, cx| {
        view.terminal().update(cx, |term, _cx| {
          term.input(text.into_bytes());
        });
      });
    }
  });

  bus.subscribe("Custom", |_mw, event, _window, _cx| {
    if let AppEvent::Custom { name, data } = event {
      tracing::info!("Custom event received: {} = {}", name, data);
    }
  });

  bus.subscribe("DispatchUIAction", |mw, event, window, cx| {
    if let AppEvent::DispatchUIAction { action_json } = event {
      match serde_json::from_str::<kazeterm_ui_tree::action::UIAction>(&action_json) {
        Ok(action) => {
          if let Err(e) = mw.dispatch_ui_action(action, window, cx) {
            tracing::error!("Failed to dispatch UIAction: {e}");
          }
        }
        Err(e) => {
          tracing::error!("Failed to parse UIAction JSON: {e}");
        }
      }
    }
  });

  bus.subscribe("SnapshotUITree", |mw, _event, _window, cx| {
    match mw.snapshot_ui_tree(cx) {
      Ok(json) => {
        tracing::info!("UI tree snapshot:\n{}", json);
      }
      Err(e) => {
        tracing::error!("Failed to snapshot UI tree: {e}");
      }
    }
  });

  bus
}

fn dispatch_event_ui_action(
  mw: &mut MainWindow,
  action: UIAction,
  action_name: &str,
  window: &mut Window,
  cx: &mut Context<MainWindow>,
) {
  mw.dispatch_default_ui_action(action, action_name, window, cx);
}

fn active_ui_tree_tab_id(mw: &MainWindow) -> Option<String> {
  mw.active_tab_ix
    .and_then(|tab_ix| mw.items.get(tab_ix))
    .map(|item| item.ui_tree_id.clone())
}

fn active_ui_tree_tab_and_pane_ids(mw: &MainWindow) -> Option<(String, String)> {
  let item = mw.active_tab_ix.and_then(|tab_ix| mw.items.get(tab_ix))?;
  let pane_id = item
    .split_container
    .active_pane_id
    .map(|pane_id| format!("pane-{}", pane_id.0))?;
  Some((item.ui_tree_id.clone(), pane_id))
}

fn dispatch_add_tab_event(
  mw: &mut MainWindow,
  profile_name: Option<&str>,
  working_directory: Option<String>,
  window: &mut Window,
  cx: &mut Context<MainWindow>,
) {
  let Some(window_id) = mw.sync_ui_tree_and_window_id(cx) else {
    return;
  };
  let action = MainWindow::build_add_tab_ui_action(window_id, profile_name, working_directory, cx);
  dispatch_event_ui_action(mw, action, "add tab", window, cx);
}

fn dispatch_close_tab_event(
  mw: &mut MainWindow,
  tab_index: usize,
  window: &mut Window,
  cx: &mut Context<MainWindow>,
) {
  if let Some(action) = mw.build_close_tab_ui_action(tab_index, cx) {
    dispatch_event_ui_action(mw, action, "close tab", window, cx);
  }
}

fn dispatch_tab_cycle_event(
  mw: &mut MainWindow,
  forward: bool,
  window: &mut Window,
  cx: &mut Context<MainWindow>,
) {
  let Some(window_id) = mw.sync_ui_tree_and_window_id(cx) else {
    return;
  };
  let action = if forward {
    UIAction::NextTab { window_id }
  } else {
    UIAction::PreviousTab { window_id }
  };
  let action_name = if forward { "next tab" } else { "previous tab" };
  dispatch_event_ui_action(mw, action, action_name, window, cx);
}

fn dispatch_activate_tab_event(
  mw: &mut MainWindow,
  tab_index: usize,
  window: &mut Window,
  cx: &mut Context<MainWindow>,
) {
  let Some(window_id) = mw.sync_ui_tree_and_window_id(cx) else {
    return;
  };
  dispatch_event_ui_action(
    mw,
    UIAction::ActivateTab {
      window_id,
      tab_index,
    },
    "activate tab",
    window,
    cx,
  );
}

fn dispatch_split_pane_event(
  mw: &mut MainWindow,
  direction: TreeSplitDirection,
  window: &mut Window,
  cx: &mut Context<MainWindow>,
) {
  if mw.active_tab_has_hidden_panes() {
    match direction {
      TreeSplitDirection::Horizontal => mw.split_pane_horizontal(window, cx),
      TreeSplitDirection::Vertical => mw.split_pane_vertical(window, cx),
    }
    return;
  }

  mw.sync_active_pane_from_focus(window, cx);

  let Some(window_id) = mw.sync_ui_tree_and_window_id(cx) else {
    return;
  };
  let Some((tab_id, pane_id)) = active_ui_tree_tab_and_pane_ids(mw) else {
    return;
  };
  let working_directory = mw.active_terminal_working_directory(cx);
  let (shell_path, shell_args) = mw
    .active_tab_ix
    .and_then(|tab_ix| mw.items.get(tab_ix))
    .map(|item| (item.shell_path.clone(), item.shell_args.clone()))
    .unwrap_or_else(|| {
      (
        cx.global::<::config::Config>().get_shell().clone(),
        Vec::new(),
      )
    });
  let action_name = match direction {
    TreeSplitDirection::Horizontal => "split pane horizontally",
    TreeSplitDirection::Vertical => "split pane vertically",
  };

  dispatch_event_ui_action(
    mw,
    UIAction::SplitPane {
      window_id,
      tab_id,
      pane_id,
      direction,
      shell_path,
      shell_args,
      working_directory,
    },
    action_name,
    window,
    cx,
  );
}

fn dispatch_close_active_pane_event(
  mw: &mut MainWindow,
  window: &mut Window,
  cx: &mut Context<MainWindow>,
) {
  if mw.active_tab_has_hidden_panes() {
    mw.close_active_pane(window, cx);
    return;
  }

  if mw
    .active_tab_ix
    .and_then(|tab_ix| mw.items.get(tab_ix))
    .is_some_and(|item| item.split_container.root.count_panes() <= 1)
  {
    return;
  }

  mw.sync_active_pane_from_focus(window, cx);

  let Some(window_id) = mw.sync_ui_tree_and_window_id(cx) else {
    return;
  };
  let Some((tab_id, pane_id)) = active_ui_tree_tab_and_pane_ids(mw) else {
    return;
  };

  dispatch_event_ui_action(
    mw,
    UIAction::ClosePane {
      window_id,
      tab_id,
      pane_id,
    },
    "close pane",
    window,
    cx,
  );
}

fn dispatch_cycle_pane_focus_event(
  mw: &mut MainWindow,
  forward: bool,
  window: &mut Window,
  cx: &mut Context<MainWindow>,
) {
  if mw.active_tab_has_hidden_panes() {
    if forward {
      mw.focus_next_pane(window, cx);
    } else {
      mw.focus_prev_pane(window, cx);
    }
    return;
  }

  mw.sync_active_pane_from_focus(window, cx);

  let Some(window_id) = mw.sync_ui_tree_and_window_id(cx) else {
    return;
  };
  let Some(tab_id) = active_ui_tree_tab_id(mw) else {
    return;
  };
  let action = if forward {
    UIAction::FocusNextPane { window_id, tab_id }
  } else {
    UIAction::FocusPreviousPane { window_id, tab_id }
  };
  let action_name = if forward {
    "focus next pane"
  } else {
    "focus previous pane"
  };

  dispatch_event_ui_action(mw, action, action_name, window, cx);
}

fn fallback_directional_pane_focus(
  mw: &mut MainWindow,
  direction: PaneFocusDirection,
  window: &mut Window,
  cx: &mut Context<MainWindow>,
) {
  match direction {
    PaneFocusDirection::Up => mw.focus_pane_up(window, cx),
    PaneFocusDirection::Down => mw.focus_pane_down(window, cx),
    PaneFocusDirection::Left => mw.focus_pane_left(window, cx),
    PaneFocusDirection::Right => mw.focus_pane_right(window, cx),
  }
}

fn directional_pane_focus_action_name(direction: PaneFocusDirection) -> &'static str {
  match direction {
    PaneFocusDirection::Up => "focus pane up",
    PaneFocusDirection::Down => "focus pane down",
    PaneFocusDirection::Left => "focus pane left",
    PaneFocusDirection::Right => "focus pane right",
  }
}

fn dispatch_directional_pane_focus_event(
  mw: &mut MainWindow,
  direction: PaneFocusDirection,
  window: &mut Window,
  cx: &mut Context<MainWindow>,
) {
  if mw.active_tab_has_hidden_panes() {
    fallback_directional_pane_focus(mw, direction, window, cx);
    return;
  }

  mw.sync_active_pane_from_focus(window, cx);

  let Some(window_id) = mw.sync_ui_tree_and_window_id(cx) else {
    return;
  };
  let Some((tab_id, pane_id)) = mw
    .active_tab_ix
    .and_then(|tab_ix| mw.items.get(tab_ix))
    .and_then(|item| {
      item
        .split_container
        .pane_in_direction(direction)
        .map(|pane_id| (item.ui_tree_id.clone(), format!("pane-{}", pane_id.0)))
    })
  else {
    return;
  };

  dispatch_event_ui_action(
    mw,
    UIAction::FocusPane {
      window_id,
      tab_id,
      pane_id,
    },
    directional_pane_focus_action_name(direction),
    window,
    cx,
  );
}

fn dispatch_swap_panes_event(
  mw: &mut MainWindow,
  window: &mut Window,
  cx: &mut Context<MainWindow>,
) {
  if mw.active_tab_has_hidden_panes() {
    mw.swap_split_panes(window, cx);
    return;
  }

  mw.sync_active_pane_from_focus(window, cx);

  let Some(window_id) = mw.sync_ui_tree_and_window_id(cx) else {
    return;
  };
  let Some(tab_id) = active_ui_tree_tab_id(mw) else {
    return;
  };

  dispatch_event_ui_action(
    mw,
    UIAction::SwapPanes { window_id, tab_id },
    "swap panes",
    window,
    cx,
  );
}

fn dispatch_toggle_search_event(
  mw: &mut MainWindow,
  window: &mut Window,
  cx: &mut Context<MainWindow>,
) {
  let Some(window_id) = mw.sync_ui_tree_and_window_id(cx) else {
    return;
  };
  dispatch_event_ui_action(
    mw,
    UIAction::ToggleSearch { window_id },
    "toggle search",
    window,
    cx,
  );
}

fn dispatch_toggle_tab_bar_event(
  mw: &mut MainWindow,
  window: &mut Window,
  cx: &mut Context<MainWindow>,
) {
  let Some(window_id) = mw.sync_ui_tree_and_window_id(cx) else {
    return;
  };
  dispatch_event_ui_action(
    mw,
    UIAction::ToggleTabBar { window_id },
    "toggle tab bar",
    window,
    cx,
  );
}

fn dispatch_overlay_event(
  mw: &mut MainWindow,
  overlay: OverlayNode,
  action_name: &str,
  window: &mut Window,
  cx: &mut Context<MainWindow>,
) {
  let Some(window_id) = mw.sync_ui_tree_and_window_id(cx) else {
    return;
  };
  dispatch_event_ui_action(
    mw,
    UIAction::ShowOverlay { window_id, overlay },
    action_name,
    window,
    cx,
  );
}

/// Initialize the shared event-system runtime with Kazeterm's default handler set.
pub fn start_event_system(
  main_window: WeakEntity<MainWindow>,
  window_handle: AnyWindowHandle,
  source_config: EventSourceConfig,
  cx: &mut App,
) {
  let event_bus = build_default_event_bus(source_config.clone());
  kazeterm_event_system::start_event_system(
    main_window,
    window_handle,
    source_config,
    event_bus,
    cx,
  );
}

#[cfg(test)]
mod tests {
  use super::build_default_event_bus;
  use kazeterm_event_system::EventSourceConfig;

  #[test]
  fn test_default_event_bus_has_all_handlers() {
    let bus = build_default_event_bus(EventSourceConfig::None);

    let expected_events = [
      "NewTerminalWithDefaultProfile",
      "NewTerminalWithProfile",
      "CloseActiveTab",
      "CloseTab",
      "NextTab",
      "PreviousTab",
      "SwitchToTab",
      "SplitHorizontal",
      "SplitVertical",
      "CloseActivePane",
      "FocusNextPane",
      "FocusPreviousPane",
      "FocusPaneUp",
      "FocusPaneDown",
      "FocusPaneLeft",
      "FocusPaneRight",
      "SwapSplitPanes",
      "ToggleSearch",
      "ToggleFullscreen",
      "ToggleTabBar",
      "ShowAboutDialog",
      "ShowImportAlacrittyDialog",
      "ReloadConfig",
      "FocusActiveTerminal",
      "NewWindow",
      "Quit",
      "SendTextToTerminal",
      "Custom",
      "DispatchUIAction",
      "SnapshotUITree",
    ];

    for event in expected_events {
      assert!(
        bus.handler_count(event) > 0,
        "expected at least one handler for {}",
        event
      );
    }
  }
}
