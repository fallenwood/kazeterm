//! Kazeterm-specific event handlers built on top of the shared event-system crate.

use gpui::{AnyWindowHandle, App, WeakEntity};
use kazeterm_event_system::EventBus;

pub use kazeterm_event_system::{
  AppEvent, EventSourceConfig, JsonEvent, send_event, try_send_event,
};

use crate::components::MainWindow;

/// Build the default [`EventBus`] with all built-in Kazeterm handlers registered.
pub fn build_default_event_bus(source_config: EventSourceConfig) -> EventBus<MainWindow> {
  let mut bus: EventBus<MainWindow> = EventBus::new();
  let source_config_for_new_window = source_config;

  bus.subscribe("NewTerminalWithDefaultProfile", |mw, _event, window, cx| {
    mw.insert_new_tab(window, cx);
  });

  bus.subscribe("NewTerminalWithProfile", |mw, event, window, cx| {
    if let AppEvent::NewTerminalWithProfile {
      profile_name,
      working_directory,
    } = event
    {
      mw.insert_new_tab_with_profile(Some(&profile_name), working_directory, window, cx);
    }
  });

  bus.subscribe("CloseActiveTab", |mw, _event, window, cx| {
    if let Some(active_ix) = mw.active_tab_ix
      && let Some(item) = mw.items.get(active_ix)
    {
      mw.remove_tab_by(item.index, window, cx);
    }
  });

  bus.subscribe("CloseTab", |mw, event, window, cx| {
    if let AppEvent::CloseTab { tab_index } = event {
      mw.remove_tab_by(tab_index, window, cx);
    }
  });

  bus.subscribe("NextTab", |mw, _event, window, cx| {
    if !mw.items.is_empty() {
      let current_ix = mw.active_tab_ix.unwrap_or(0);
      let next_ix = (current_ix + 1) % mw.items.len();
      mw.set_active_tab(next_ix, window, cx);
    }
  });

  bus.subscribe("PreviousTab", |mw, _event, window, cx| {
    if !mw.items.is_empty() {
      let current_ix = mw.active_tab_ix.unwrap_or(0);
      let prev_ix = if current_ix == 0 {
        mw.items.len() - 1
      } else {
        current_ix - 1
      };
      mw.set_active_tab(prev_ix, window, cx);
    }
  });

  bus.subscribe("SwitchToTab", |mw, event, window, cx| {
    if let AppEvent::SwitchToTab { position } = event
      && position < mw.items.len()
    {
      mw.set_active_tab(position, window, cx);
    }
  });

  bus.subscribe("SplitHorizontal", |mw, _event, window, cx| {
    mw.split_pane_horizontal(window, cx);
  });

  bus.subscribe("SplitVertical", |mw, _event, window, cx| {
    mw.split_pane_vertical(window, cx);
  });

  bus.subscribe("CloseActivePane", |mw, _event, window, cx| {
    mw.close_active_pane(window, cx);
  });

  bus.subscribe("FocusNextPane", |mw, _event, window, cx| {
    mw.focus_next_pane(window, cx);
  });

  bus.subscribe("FocusPreviousPane", |mw, _event, window, cx| {
    mw.focus_prev_pane(window, cx);
  });

  bus.subscribe("FocusPaneUp", |mw, _event, window, cx| {
    mw.focus_pane_up(window, cx);
  });

  bus.subscribe("FocusPaneDown", |mw, _event, window, cx| {
    mw.focus_pane_down(window, cx);
  });

  bus.subscribe("FocusPaneLeft", |mw, _event, window, cx| {
    mw.focus_pane_left(window, cx);
  });

  bus.subscribe("FocusPaneRight", |mw, _event, window, cx| {
    mw.focus_pane_right(window, cx);
  });

  bus.subscribe("SwapSplitPanes", |mw, _event, window, cx| {
    mw.swap_split_panes(window, cx);
  });

  bus.subscribe("ToggleSearch", |mw, _event, window, cx| {
    mw.toggle_search(window, cx);
  });

  bus.subscribe("ToggleFullscreen", |_mw, _event, window, _cx| {
    window.toggle_fullscreen();
  });

  bus.subscribe("ToggleTabBar", |mw, _event, _window, cx| {
    mw.toggle_tab_bar(_window, cx);
  });

  bus.subscribe("ShowAboutDialog", |mw, _event, window, cx| {
    mw.show_about_dialog(window, cx);
  });

  bus.subscribe("ShowImportAlacrittyDialog", |mw, _event, window, cx| {
    mw.show_import_alacritty_dialog(window, cx);
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
