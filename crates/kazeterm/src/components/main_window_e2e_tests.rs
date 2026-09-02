//! End-to-end tests that boot a real `MainWindow` with a fake terminal
//! session factory. These exercise the full tab-management / event flow
//! without spawning any child processes.
//!
//! NOTE: these tests share a process-global factory override, so they must
//! run serially. A dedicated `Mutex` enforces that.
#![cfg(test)]

use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gpui::{TestAppContext, WindowHandle};
use kazeterm_ui_tree::action::UIAction;
use kazeterm_ui_tree::node::{OverlayNode, UITree};

use crate::components::MainWindow;
use crate::components::terminal_window::{
  clear_terminal_session_factory_for_testing, set_terminal_session_factory_for_testing,
};
use crate::components::transitions::{UI_TRANSITION_FRAME_DURATION, UI_TRANSITION_FRAMES};
use crate::event_system::{AppEvent, EventSourceConfig, build_default_event_bus};
use terminal::test_support::fake_terminal_session;

/// Global serializer: e2e tests install a process-global factory, so only
/// one may run at a time.
pub(super) fn test_lock() -> MutexGuard<'static, ()> {
  static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
  LOCK
    .get_or_init(|| Mutex::new(()))
    .lock()
    .unwrap_or_else(|p| p.into_inner())
}

/// Records every call the MainWindow makes into the terminal-session factory.
#[derive(Default)]
pub(super) struct FactoryCalls {
  pub(super) programs: Vec<String>,
  args: Vec<Vec<String>>,
}

pub(super) fn install_fake_factory() -> Arc<Mutex<FactoryCalls>> {
  let calls = Arc::new(Mutex::new(FactoryCalls::default()));
  let calls_clone = calls.clone();
  set_terminal_session_factory_for_testing(Box::new(move |program, args, _cwd, _cfg| {
    let mut locked = calls_clone.lock().unwrap();
    locked.programs.push(program);
    locked.args.push(args);
    drop(locked);
    let (term, events, _writes, _resizes) = fake_terminal_session(80, 24);
    Ok((term, events))
  }));
  calls
}

fn temp_ui_tree_json_path(name: &str) -> std::path::PathBuf {
  let unique = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .expect("system time should be after unix epoch")
    .as_nanos();
  std::env::temp_dir().join(format!("kazeterm-{name}-{unique}.json"))
}

fn dispatch_app_event(window: &WindowHandle<MainWindow>, event: AppEvent, cx: &mut TestAppContext) {
  window
    .update(cx, move |root: &mut MainWindow, window, cx| {
      let bus = build_default_event_bus(EventSourceConfig::None);
      assert_eq!(bus.dispatch(root, event, window, cx), 1);
    })
    .expect("event dispatch should succeed");
}

fn advance_ui_transition(cx: &mut TestAppContext, frames: u32) {
  for _ in 0..frames {
    cx.executor().advance_clock(UI_TRANSITION_FRAME_DURATION);
    cx.run_until_parked();
  }
}

#[gpui::test]
fn main_window_creates_initial_tab_with_fake_factory(cx: &mut TestAppContext) {
  let _guard = test_lock();
  crate::test_support::init_test_app(cx);
  let calls = install_fake_factory();

  let _window = cx.add_window(|window, cx| MainWindow::new(window, cx));
  cx.run_until_parked();

  let call_count = calls.lock().unwrap().programs.len();
  assert!(
    call_count >= 1,
    "expected MainWindow to invoke the terminal factory at least once (got {call_count})"
  );

  clear_terminal_session_factory_for_testing();
}

#[gpui::test]
fn event_target_tracks_the_active_window(cx: &mut TestAppContext) {
  let _guard = test_lock();
  crate::test_support::init_test_app(cx);
  install_fake_factory();

  let first = cx.add_window(|window, cx| MainWindow::new(window, cx));
  let second = cx.add_window(|window, cx| MainWindow::new(window, cx));
  let first_view = first.root(cx).expect("first window root should exist");
  let second_view = second.root(cx).expect("second window root should exist");
  let first_handle = first
    .update(cx, |_root, window, cx| {
      let handle = window.window_handle();
      crate::window_manager::register_window(handle, &first_view, cx);
      handle
    })
    .expect("first window registration should succeed");
  let second_handle = second
    .update(cx, |_root, window, cx| {
      let handle = window.window_handle();
      crate::window_manager::register_window(handle, &second_view, cx);
      handle
    })
    .expect("second window registration should succeed");
  drop(first_view);
  drop(second_view);

  first
    .update(cx, |_root, window, _cx| window.activate_window())
    .expect("first window activation should succeed");
  cx.run_until_parked();
  let active_handle = cx.update(|cx| {
    crate::window_manager::active_event_target(cx)
      .expect("an active event target should exist")
      .1
  });
  assert!(active_handle == first_handle);

  second
    .update(cx, |_root, window, _cx| window.activate_window())
    .expect("second window activation should succeed");
  cx.run_until_parked();
  let active_handle = cx.update(|cx| {
    crate::window_manager::active_event_target(cx)
      .expect("an active event target should exist")
      .1
  });
  assert!(active_handle == second_handle);

  second
    .update(cx, |_root, window, _cx| window.remove_window())
    .expect("second window removal should succeed");
  cx.run_until_parked();
  let fallback_handle = cx.update(|cx| {
    crate::window_manager::active_event_target(cx)
      .expect("a fallback event target should exist")
      .1
  });
  assert!(fallback_handle == first_handle);

  clear_terminal_session_factory_for_testing();
}

#[gpui::test]
fn resize_ui_action_transitions_to_target_size(cx: &mut TestAppContext) {
  let _guard = test_lock();
  crate::test_support::init_test_app(cx);
  install_fake_factory();

  let window = cx.add_window(|window, cx| MainWindow::new(window, cx));
  cx.run_until_parked();

  let initial_size = window
    .update(cx, |_root, window, _cx| window.bounds().size)
    .expect("reading initial window size should succeed");
  let target_size = gpui::size(
    initial_size.width + gpui::px(240.0),
    initial_size.height + gpui::px(160.0),
  );

  window
    .update(cx, |root, window, cx| {
      let window_id = root
        .sync_ui_tree_and_window_id(cx)
        .expect("UI tree should contain a window");
      root
        .dispatch_ui_action(
          UIAction::ResizeWindow {
            window_id,
            width: f32::from(target_size.width),
            height: f32::from(target_size.height),
          },
          window,
          cx,
        )
        .expect("resize action should dispatch");
    })
    .expect("window update should succeed");

  let size_before_first_frame = window
    .update(cx, |_root, window, _cx| window.bounds().size)
    .expect("reading pre-animation size should succeed");
  assert_eq!(size_before_first_frame, initial_size);

  cx.run_until_parked();
  advance_ui_transition(cx, 1);

  let intermediate_size = window
    .update(cx, |_root, window, _cx| window.bounds().size)
    .expect("reading intermediate window size should succeed");
  assert_ne!(intermediate_size, initial_size);
  assert_ne!(intermediate_size, target_size);

  advance_ui_transition(cx, UI_TRANSITION_FRAMES - 1);

  let final_size = window
    .update(cx, |_root, window, _cx| window.bounds().size)
    .expect("reading final window size should succeed");
  assert_eq!(final_size, target_size);

  clear_terminal_session_factory_for_testing();
}

#[gpui::test]
fn vertical_sidebar_visibility_transitions_in_both_directions(cx: &mut TestAppContext) {
  let _guard = test_lock();
  crate::test_support::init_test_app(cx);
  cx.update(|cx| {
    let mut config = cx.global::<::config::Config>().clone();
    config.tab.vertical = true;
    cx.set_global(config);
  });
  install_fake_factory();

  let window = cx.add_window(|window, cx| MainWindow::new(window, cx));
  cx.run_until_parked();

  let expanded_width = window
    .update(cx, |root, _window, _cx| root.vertical_tabbar_render_width)
    .expect("reading expanded sidebar width should succeed");
  assert!(expanded_width > gpui::Pixels::ZERO);

  window
    .update(cx, |root, window, cx| {
      root.toggle_tab_bar(window, cx);
    })
    .expect("hiding tab bar should succeed");
  let width_before_first_frame = window
    .update(cx, |root, _window, _cx| root.vertical_tabbar_render_width)
    .expect("reading pre-animation sidebar width should succeed");
  assert_eq!(width_before_first_frame, expanded_width);

  cx.run_until_parked();
  advance_ui_transition(cx, 1);
  let shrinking_width = window
    .update(cx, |root, _window, _cx| root.vertical_tabbar_render_width)
    .expect("reading shrinking sidebar width should succeed");
  assert!(shrinking_width > gpui::Pixels::ZERO);
  assert!(shrinking_width < expanded_width);

  advance_ui_transition(cx, UI_TRANSITION_FRAMES - 1);
  let hidden_width = window
    .update(cx, |root, _window, _cx| root.vertical_tabbar_render_width)
    .expect("reading hidden sidebar width should succeed");
  assert_eq!(hidden_width, gpui::Pixels::ZERO);

  window
    .update(cx, |root, window, cx| {
      root.toggle_tab_bar(window, cx);
    })
    .expect("showing tab bar should succeed");
  cx.run_until_parked();
  advance_ui_transition(cx, UI_TRANSITION_FRAMES);

  let restored_width = window
    .update(cx, |root, _window, _cx| root.vertical_tabbar_render_width)
    .expect("reading restored sidebar width should succeed");
  assert_eq!(restored_width, expanded_width);

  clear_terminal_session_factory_for_testing();
}

#[gpui::test]
fn configuration_change_fades_ui_and_expands_vertical_sidebar(cx: &mut TestAppContext) {
  let _guard = test_lock();
  crate::test_support::init_test_app(cx);
  install_fake_factory();

  let window = cx.add_window(|window, cx| MainWindow::new(window, cx));
  cx.run_until_parked();
  let view = window
    .root(cx)
    .expect("main window should have a root view");
  window
    .update(cx, |_root, window, cx| {
      crate::window_manager::register_window(window.window_handle(), &view, cx);
    })
    .expect("registering main window should succeed");

  let config = cx.update(|cx| {
    let mut config = cx.global::<::config::Config>().clone();
    config.tab.vertical = true;
    cx.set_global(config.clone());
    config
  });
  cx.update(|cx| {
    crate::window_manager::transition_configuration_change(&config, cx);
  });

  let (initial_opacity, initial_width, target_width) = window
    .update(cx, |root, _window, _cx| {
      (
        root.ui_transition_opacity,
        root.vertical_tabbar_render_width,
        root.vertical_tabbar_width,
      )
    })
    .expect("reading initial configuration transition should succeed");
  assert!(initial_opacity < 1.0);
  assert_eq!(initial_width, gpui::Pixels::ZERO);

  cx.run_until_parked();
  advance_ui_transition(cx, 1);
  let (intermediate_opacity, intermediate_width) = window
    .update(cx, |root, _window, _cx| {
      (
        root.ui_transition_opacity,
        root.vertical_tabbar_render_width,
      )
    })
    .expect("reading intermediate configuration transition should succeed");
  assert!(intermediate_opacity > initial_opacity);
  assert!(intermediate_opacity < 1.0);
  assert!(intermediate_width > gpui::Pixels::ZERO);
  assert!(intermediate_width < target_width);

  advance_ui_transition(cx, UI_TRANSITION_FRAMES - 1);
  let (final_opacity, final_width) = window
    .update(cx, |root, _window, _cx| {
      (
        root.ui_transition_opacity,
        root.vertical_tabbar_render_width,
      )
    })
    .expect("reading completed configuration transition should succeed");
  assert_eq!(final_opacity, 1.0);
  assert_eq!(final_width, target_width);

  clear_terminal_session_factory_for_testing();
}

#[gpui::test]
fn structural_change_uses_configured_animation_parameters(cx: &mut TestAppContext) {
  let _guard = test_lock();
  crate::test_support::init_test_app(cx);
  cx.update(|cx| {
    let mut config = cx.global::<::config::Config>().clone();
    config.animation.duration_ms = 60;
    config.animation.frame_interval_ms = 20;
    config.animation.easing = ::config::AnimationEasing::Linear;
    config.animation.fade_start_opacity = 0.4;
    cx.set_global(config);
  });
  install_fake_factory();

  let window = cx.add_window(|window, cx| MainWindow::new(window, cx));
  cx.run_until_parked();

  window
    .update(cx, |root, window, cx| root.insert_new_tab(window, cx))
    .expect("adding a tab should succeed");
  let initial_opacity = window
    .update(cx, |root, _window, _cx| root.ui_transition_opacity)
    .expect("reading initial transition opacity should succeed");
  assert!((initial_opacity - 0.4).abs() < 0.001);

  cx.run_until_parked();
  cx.executor().advance_clock(Duration::from_millis(20));
  cx.run_until_parked();
  let first_frame_opacity = window
    .update(cx, |root, _window, _cx| root.ui_transition_opacity)
    .expect("reading intermediate transition opacity should succeed");
  assert!((first_frame_opacity - 0.6).abs() < 0.001);

  for _ in 0..2 {
    cx.executor().advance_clock(Duration::from_millis(20));
    cx.run_until_parked();
  }
  let final_opacity = window
    .update(cx, |root, _window, _cx| root.ui_transition_opacity)
    .expect("reading final transition opacity should succeed");
  assert_eq!(final_opacity, 1.0);

  clear_terminal_session_factory_for_testing();
}

#[gpui::test]
fn disabled_animation_applies_changes_immediately(cx: &mut TestAppContext) {
  let _guard = test_lock();
  crate::test_support::init_test_app(cx);
  cx.update(|cx| {
    let mut config = cx.global::<::config::Config>().clone();
    config.tab.vertical = true;
    config.animation.enabled = false;
    cx.set_global(config);
  });
  install_fake_factory();

  let window = cx.add_window(|window, cx| MainWindow::new(window, cx));
  cx.run_until_parked();

  window
    .update(cx, |root, window, cx| root.toggle_tab_bar(window, cx))
    .expect("hiding the tab bar should succeed");
  let (sidebar_width, opacity) = window
    .update(cx, |root, _window, _cx| {
      (
        root.vertical_tabbar_render_width,
        root.ui_transition_opacity,
      )
    })
    .expect("reading immediate transition state should succeed");
  assert_eq!(sidebar_width, gpui::Pixels::ZERO);
  assert_eq!(opacity, 1.0);

  let initial_size = window
    .update(cx, |_root, window, _cx| window.bounds().size)
    .expect("reading initial size should succeed");
  let target_size = gpui::size(
    initial_size.width + gpui::px(120.0),
    initial_size.height + gpui::px(80.0),
  );
  window
    .update(cx, |root, window, cx| {
      let window_id = root
        .sync_ui_tree_and_window_id(cx)
        .expect("UI tree should contain a window");
      root
        .dispatch_ui_action(
          UIAction::ResizeWindow {
            window_id,
            width: f32::from(target_size.width),
            height: f32::from(target_size.height),
          },
          window,
          cx,
        )
        .expect("resize action should dispatch");
    })
    .expect("window update should succeed");
  let resized = window
    .update(cx, |_root, window, _cx| window.bounds().size)
    .expect("reading resized window should succeed");
  assert_eq!(resized, target_size);

  clear_terminal_session_factory_for_testing();
}

#[gpui::test]
fn insert_new_tab_increments_item_count(cx: &mut TestAppContext) {
  let _guard = test_lock();
  crate::test_support::init_test_app(cx);
  let calls = install_fake_factory();

  let window = cx.add_window(|window, cx| MainWindow::new(window, cx));
  cx.run_until_parked();

  let initial = calls.lock().unwrap().programs.len();

  let view = window.root(cx).unwrap();
  window
    .update(cx, |root: &mut MainWindow, window, cx| {
      root.insert_new_tab(window, cx);
      root.insert_new_tab(window, cx);
    })
    .expect("update should succeed");
  cx.run_until_parked();

  let final_count = calls.lock().unwrap().programs.len();
  assert_eq!(
    final_count,
    initial + 2,
    "expected two additional factory calls after insert_new_tab ×2"
  );

  view.read_with(cx, |mw, _| {
    assert!(
      mw.items.len() >= 3,
      "expected at least 3 tab items, got {}",
      mw.items.len()
    );
    assert_eq!(
      mw.ui_tree.tree().windows[0].tabs.len(),
      mw.items.len(),
      "expected UITree tab count to stay in sync with live tabs",
    );
  });

  clear_terminal_session_factory_for_testing();
}

#[gpui::test]
fn split_pane_reuses_existing_terminal_session(cx: &mut TestAppContext) {
  let _guard = test_lock();
  crate::test_support::init_test_app(cx);
  let calls = install_fake_factory();

  let window = cx.add_window(|window, cx| MainWindow::new(window, cx));
  cx.run_until_parked();

  let initial_call_count = calls.lock().unwrap().programs.len();
  let existing_terminal_index = window
    .update(cx, |root: &mut MainWindow, _window, cx| {
      root.items[0].split_container.all_terminals()[0]
        .1
        .read(cx)
        .index
    })
    .expect("reading initial terminal index should succeed");

  window
    .update(cx, |root: &mut MainWindow, window, cx| {
      root.split_pane_horizontal(window, cx);
    })
    .expect("split_pane_horizontal should succeed");
  cx.run_until_parked();

  let final_call_count = calls.lock().unwrap().programs.len();
  assert_eq!(
    final_call_count,
    initial_call_count + 1,
    "expected splitting to spawn only the new pane and keep the existing terminal session",
  );

  let terminal_indices = window
    .update(cx, |root: &mut MainWindow, _window, cx| {
      root.items[0]
        .split_container
        .all_terminals()
        .into_iter()
        .map(|(_, terminal)| terminal.read(cx).index)
        .collect::<Vec<_>>()
    })
    .expect("reading split terminal indexes should succeed");
  assert_eq!(terminal_indices.len(), 2);
  assert!(
    terminal_indices.contains(&existing_terminal_index),
    "expected the original terminal entity to remain in the split tree",
  );

  clear_terminal_session_factory_for_testing();
}

#[gpui::test]
fn split_panes_can_hide_split_again_and_restore(cx: &mut TestAppContext) {
  let _guard = test_lock();
  crate::test_support::init_test_app(cx);
  install_fake_factory();

  let window = cx.add_window(|window, cx| MainWindow::new(window, cx));
  cx.run_until_parked();

  window
    .update(cx, |root: &mut MainWindow, window, cx| {
      root.split_pane_horizontal(window, cx);
    })
    .expect("split_pane_horizontal should succeed");
  cx.run_until_parked();

  let view = window.root(cx).unwrap();
  view.read_with(cx, |mw, _| {
    let split_container = &mw.items[0].split_container;
    assert_eq!(split_container.all_terminals().len(), 2);
    assert_eq!(split_container.visible_pane_count(), 2);
    assert!(!split_container.has_hidden_panes());
    assert_eq!(
      mw.ui_tree.tree().windows[0].tabs[0]
        .pane_tree
        .terminal_count(),
      2,
      "expected UITree pane tree to reflect the split immediately",
    );
  });

  window
    .update(cx, |root: &mut MainWindow, window, cx| {
      root.toggle_hidden_split_panes(window, cx);
    })
    .expect("toggle_hidden_split_panes should hide other panes");
  cx.run_until_parked();

  view.read_with(cx, |mw, _| {
    let split_container = &mw.items[0].split_container;
    assert_eq!(split_container.all_terminals().len(), 2);
    assert_eq!(split_container.visible_pane_count(), 1);
    assert!(split_container.has_hidden_panes());
  });

  window
    .update(cx, |root: &mut MainWindow, window, cx| {
      root.split_pane_vertical(window, cx);
    })
    .expect("split_pane_vertical should succeed while other panes are hidden");
  cx.run_until_parked();

  view.read_with(cx, |mw, _| {
    let split_container = &mw.items[0].split_container;
    assert_eq!(split_container.all_terminals().len(), 3);
    assert_eq!(split_container.visible_pane_count(), 2);
    assert!(split_container.has_hidden_panes());
  });

  window
    .update(cx, |root: &mut MainWindow, window, cx| {
      root.toggle_hidden_split_panes(window, cx);
    })
    .expect("toggle_hidden_split_panes should restore hidden panes");
  cx.run_until_parked();

  view.read_with(cx, |mw, _| {
    let split_container = &mw.items[0].split_container;
    assert_eq!(split_container.all_terminals().len(), 3);
    assert_eq!(split_container.visible_pane_count(), 3);
    assert!(!split_container.has_hidden_panes());
  });

  clear_terminal_session_factory_for_testing();
}

#[gpui::test]
fn dump_ui_tree_to_file_writes_json_snapshot(cx: &mut TestAppContext) {
  let _guard = test_lock();
  crate::test_support::init_test_app(cx);
  install_fake_factory();
  let dump_path = temp_ui_tree_json_path("dump-ui-tree");

  let window = cx.add_window(|window, cx| MainWindow::new(window, cx));
  cx.run_until_parked();

  window
    .update(cx, |root: &mut MainWindow, window, cx| {
      root.split_pane_horizontal(window, cx);
      root
        .dump_ui_tree_to_path(&dump_path, cx)
        .expect("dump_ui_tree_to_path should succeed");
    })
    .expect("update should succeed");
  cx.run_until_parked();

  let json = std::fs::read_to_string(&dump_path).expect("dumped JSON file should exist");
  let tree: UITree = serde_json::from_str(&json).expect("dumped JSON should parse as UITree");
  assert_eq!(tree.windows.len(), 1);
  assert_eq!(tree.windows[0].tabs.len(), 1);
  assert_eq!(tree.windows[0].tabs[0].pane_tree.terminal_count(), 2);

  let _ = std::fs::remove_file(&dump_path);
  clear_terminal_session_factory_for_testing();
}

#[gpui::test]
fn dump_ui_tree_picker_writes_json_snapshot(cx: &mut TestAppContext) {
  let _guard = test_lock();
  crate::test_support::init_test_app(cx);
  install_fake_factory();
  let dump_path = temp_ui_tree_json_path("dump-ui-tree-picker");

  let window = cx.add_window(|window, cx| MainWindow::new(window, cx));
  cx.run_until_parked();

  window
    .update(cx, |root: &mut MainWindow, window, cx| {
      root.split_pane_horizontal(window, cx);
      root.prompt_dump_ui_tree_path(window, cx);
    })
    .expect("update should succeed");
  cx.run_until_parked();

  assert!(cx.did_prompt_for_new_path());
  cx.simulate_new_path_selection(|_| Some(dump_path.clone()));
  cx.run_until_parked();

  let json = std::fs::read_to_string(&dump_path).expect("dumped JSON file should exist");
  let tree: UITree = serde_json::from_str(&json).expect("dumped JSON should parse as UITree");
  assert_eq!(tree.windows.len(), 1);
  assert_eq!(tree.windows[0].tabs.len(), 1);
  assert_eq!(tree.windows[0].tabs[0].pane_tree.terminal_count(), 2);

  let view = window.root(cx).unwrap();
  view.read_with(cx, |mw, _| {
    assert!(
      !mw.ui_tree_json_prompt_pending,
      "expected picker pending state to clear after save selection",
    );
  });

  let _ = std::fs::remove_file(&dump_path);
  clear_terminal_session_factory_for_testing();
}

#[gpui::test]
fn load_ui_tree_from_file_replaces_existing_window_state(cx: &mut TestAppContext) {
  let _guard = test_lock();
  crate::test_support::init_test_app(cx);
  install_fake_factory();
  let dump_path = temp_ui_tree_json_path("load-ui-tree");

  let window = cx.add_window(|window, cx| MainWindow::new(window, cx));
  cx.run_until_parked();

  window
    .update(cx, |root: &mut MainWindow, window, cx| {
      root.split_pane_horizontal(window, cx);
      root
        .dump_ui_tree_to_path(&dump_path, cx)
        .expect("dump_ui_tree_to_path should succeed");
      root.insert_new_tab(window, cx);
      root.insert_new_tab(window, cx);
    })
    .expect("update should succeed");
  cx.run_until_parked();

  window
    .update(cx, |root: &mut MainWindow, window, cx| {
      root
        .load_ui_tree_from_path(&dump_path, window, cx)
        .expect("load_ui_tree_from_path should succeed");
    })
    .expect("update should succeed");
  cx.run_until_parked();

  let view = window.root(cx).unwrap();
  view.read_with(cx, |mw, _| {
    assert_eq!(mw.items.len(), 1);
    assert_eq!(mw.items[0].split_container.all_terminals().len(), 2);
    assert_eq!(mw.ui_tree.tree().windows.len(), 1);
    assert_eq!(mw.ui_tree.tree().windows[0].tabs.len(), 1);
    assert_eq!(
      mw.ui_tree.tree().windows[0].tabs[0]
        .pane_tree
        .terminal_count(),
      2
    );
  });

  let _ = std::fs::remove_file(&dump_path);
  clear_terminal_session_factory_for_testing();
}

#[gpui::test]
fn event_bus_show_about_dialog_updates_ui_tree_overlay(cx: &mut TestAppContext) {
  let _guard = test_lock();
  crate::test_support::init_test_app(cx);
  install_fake_factory();

  let window = cx.add_window(|window, cx| MainWindow::new(window, cx));
  cx.run_until_parked();

  dispatch_app_event(&window, AppEvent::ShowAboutDialog, cx);
  cx.run_until_parked();

  let view = window.root(cx).unwrap();
  view.read_with(cx, |mw, _| {
    assert!(mw.about_dialog.is_some());
    assert_eq!(
      mw.ui_tree.tree().windows[0].overlay.as_ref(),
      Some(&OverlayNode::AboutDialog),
    );
  });

  clear_terminal_session_factory_for_testing();
}

#[gpui::test]
fn event_bus_focus_pane_left_updates_ui_tree_focus(cx: &mut TestAppContext) {
  let _guard = test_lock();
  crate::test_support::init_test_app(cx);
  install_fake_factory();

  let window = cx.add_window(|window, cx| MainWindow::new(window, cx));
  cx.run_until_parked();

  window
    .update(cx, |root: &mut MainWindow, window, cx| {
      root.split_pane_vertical(window, cx);
    })
    .expect("split_pane_vertical should succeed");
  cx.run_until_parked();

  let view = window.root(cx).unwrap();
  let expected_left_pane_id = view.read_with(cx, |mw, _| {
    let tab = &mw.ui_tree.tree().windows[0].tabs[0];
    let pane_ids = tab.pane_tree.terminal_ids();
    assert_eq!(pane_ids.len(), 2);
    assert_eq!(tab.pane_tree.focused_pane_id(), Some(pane_ids[1]));
    pane_ids[0].to_string()
  });

  dispatch_app_event(&window, AppEvent::FocusPaneLeft, cx);
  cx.run_until_parked();

  view.read_with(cx, |mw, _| {
    let tab = &mw.ui_tree.tree().windows[0].tabs[0];
    assert_eq!(
      tab.pane_tree.focused_pane_id(),
      Some(expected_left_pane_id.as_str())
    );
  });

  clear_terminal_session_factory_for_testing();
}

#[gpui::test]
fn moving_tab_into_terminal_split_reuses_existing_sessions(cx: &mut TestAppContext) {
  let _guard = test_lock();
  crate::test_support::init_test_app(cx);
  let calls = install_fake_factory();

  let window = cx.add_window(|window, cx| MainWindow::new(window, cx));
  cx.run_until_parked();

  window
    .update(cx, |root: &mut MainWindow, window, cx| {
      root.insert_new_tab(window, cx);
      root.split_pane_horizontal(window, cx);
    })
    .expect("source tab setup should succeed");
  cx.run_until_parked();

  window
    .update(cx, |root: &mut MainWindow, window, cx| {
      root.set_active_tab(0, window, cx);
    })
    .expect("activating the target tab should succeed");
  cx.run_until_parked();

  let call_count_before_merge = calls.lock().unwrap().programs.len();
  let target_pane_id = window
    .update(cx, |root: &mut MainWindow, _window, _cx| {
      root.items[0]
        .split_container
        .all_terminals()
        .first()
        .map(|(pane_id, _)| *pane_id)
        .expect("target tab should have a terminal pane")
    })
    .expect("reading the target pane id should succeed");

  window
    .update(cx, |root: &mut MainWindow, window, cx| {
      root.move_tab_into_split(1, target_pane_id, window, cx);
    })
    .expect("moving the tab into a split should succeed");
  cx.run_until_parked();

  let call_count_after_merge = calls.lock().unwrap().programs.len();
  assert_eq!(
    call_count_after_merge, call_count_before_merge,
    "expected tab-to-split move to reuse the dragged tab terminals instead of creating new ones",
  );

  let view = window.root(cx).unwrap();
  view.read_with(cx, |mw, _| {
    assert_eq!(mw.items.len(), 1);
    assert_eq!(mw.active_tab_ix, Some(0));
    assert_eq!(mw.items[0].split_container.all_terminals().len(), 3);
    assert_eq!(mw.ui_tree.tree().windows[0].tabs.len(), 1);
    assert_eq!(
      mw.ui_tree.tree().windows[0].tabs[0]
        .pane_tree
        .terminal_count(),
      3,
    );
  });

  clear_terminal_session_factory_for_testing();
}

#[gpui::test]
fn pinned_tabs_are_ignored_by_close_tabs_to_right(cx: &mut TestAppContext) {
  let _guard = test_lock();
  crate::test_support::init_test_app(cx);
  install_fake_factory();

  let window = cx.add_window(|window, cx| MainWindow::new(window, cx));
  cx.run_until_parked();

  window
    .update(cx, |root: &mut MainWindow, window, cx| {
      root.insert_new_tab(window, cx);
      root.insert_new_tab(window, cx);
      root.insert_new_tab(window, cx);
    })
    .expect("creating additional tabs should succeed");
  cx.run_until_parked();

  let (keep_tab_index, pinned_tab_index) = window
    .update(cx, |root: &mut MainWindow, _window, _cx| {
      (root.items[0].index, root.items[2].index)
    })
    .expect("reading tab indexes should succeed");

  window
    .update(cx, |root: &mut MainWindow, window, cx| {
      root.set_tab_pinned(pinned_tab_index, true, window, cx);
    })
    .expect("pinning a tab should succeed");
  cx.run_until_parked();

  window
    .update(cx, |root: &mut MainWindow, window, cx| {
      root.close_tabs_to_right(0, window, cx);
      root.sync_ui_tree(cx);
    })
    .expect("closing tabs to the right should succeed");
  cx.run_until_parked();

  let view = window.root(cx).unwrap();
  view.read_with(cx, |mw, _| {
    assert_eq!(mw.items.len(), 2);
    assert_eq!(mw.active_tab_ix, Some(0));
    assert_eq!(mw.items[0].index, keep_tab_index);
    assert_eq!(mw.items[1].index, pinned_tab_index);
    assert!(mw.items[1].pinned);
    assert_eq!(mw.ui_tree.tree().windows[0].tabs.len(), 2);
    assert!(mw.ui_tree.tree().windows[0].tabs[1].pinned);
  });

  clear_terminal_session_factory_for_testing();
}

#[gpui::test]
fn pinned_tabs_are_ignored_by_close_other_tabs(cx: &mut TestAppContext) {
  let _guard = test_lock();
  crate::test_support::init_test_app(cx);
  install_fake_factory();

  let window = cx.add_window(|window, cx| MainWindow::new(window, cx));
  cx.run_until_parked();

  window
    .update(cx, |root: &mut MainWindow, window, cx| {
      root.insert_new_tab(window, cx);
      root.insert_new_tab(window, cx);
      root.insert_new_tab(window, cx);
    })
    .expect("creating additional tabs should succeed");
  cx.run_until_parked();

  let (pinned_tab_index, keep_tab_index) = window
    .update(cx, |root: &mut MainWindow, _window, _cx| {
      (root.items[0].index, root.items[2].index)
    })
    .expect("reading tab indexes should succeed");

  window
    .update(cx, |root: &mut MainWindow, window, cx| {
      root.set_tab_pinned(pinned_tab_index, true, window, cx);
    })
    .expect("pinning a tab should succeed");
  cx.run_until_parked();

  window
    .update(cx, |root: &mut MainWindow, window, cx| {
      root.close_other_tabs(keep_tab_index, window, cx);
      root.sync_ui_tree(cx);
    })
    .expect("closing other tabs should succeed");
  cx.run_until_parked();

  let view = window.root(cx).unwrap();
  view.read_with(cx, |mw, _| {
    assert_eq!(mw.items.len(), 2);
    assert_eq!(mw.active_tab_ix, Some(1));
    assert_eq!(mw.items[0].index, pinned_tab_index);
    assert!(mw.items[0].pinned);
    assert_eq!(mw.items[1].index, keep_tab_index);
    assert!(!mw.items[1].pinned);
    assert_eq!(mw.ui_tree.tree().windows[0].tabs.len(), 2);
    assert!(mw.ui_tree.tree().windows[0].tabs[0].pinned);
  });

  clear_terminal_session_factory_for_testing();
}

#[gpui::test]
fn pinned_tabs_round_trip_through_ui_tree_snapshots(cx: &mut TestAppContext) {
  let _guard = test_lock();
  crate::test_support::init_test_app(cx);
  install_fake_factory();

  let window = cx.add_window(|window, cx| MainWindow::new(window, cx));
  cx.run_until_parked();

  window
    .update(cx, |root: &mut MainWindow, window, cx| {
      root.insert_new_tab(window, cx);
    })
    .expect("creating an additional tab should succeed");
  cx.run_until_parked();

  let pinned_tab_index = window
    .update(cx, |root: &mut MainWindow, _window, _cx| {
      root.items[0].index
    })
    .expect("reading the pinned tab index should succeed");

  window
    .update(cx, |root: &mut MainWindow, window, cx| {
      root.set_tab_pinned(pinned_tab_index, true, window, cx);
    })
    .expect("pinning a tab should succeed");
  cx.run_until_parked();

  let snapshot = window
    .update(cx, |root: &mut MainWindow, _window, cx| {
      root
        .snapshot_ui_tree(cx)
        .expect("snapshot should serialize successfully")
    })
    .expect("snapshot should succeed");

  assert!(
    snapshot.contains("\"pinned\": true"),
    "expected pinned tabs to be serialized into the UI tree snapshot"
  );

  window
    .update(cx, |root: &mut MainWindow, window, cx| {
      root
        .load_ui_tree_from_str(&snapshot, window, cx)
        .expect("snapshot should restore successfully");
    })
    .expect("loading a UI tree snapshot should succeed");
  cx.run_until_parked();

  let view = window.root(cx).unwrap();
  view.read_with(cx, |mw, _| {
    assert_eq!(mw.items.len(), 2);
    assert_eq!(mw.active_tab_ix, Some(1));
    assert!(mw.items[0].pinned);
    assert!(!mw.items[1].pinned);
    assert!(mw.ui_tree.tree().windows[0].tabs[0].pinned);
  });

  clear_terminal_session_factory_for_testing();
}
