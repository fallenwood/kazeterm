#![cfg(test)]

use std::sync::atomic::{AtomicU64, Ordering};

use gpui::TestAppContext;

use super::{
  MainWindow,
  close_confirm_dialog::CloseConfirmEvent,
  main_window_e2e_tests::{install_fake_factory, test_lock},
  terminal_window::clear_terminal_session_factory_for_testing,
  workspace_state::override_workspace_directory_for_testing,
};
use crate::reconciler::UITreeStore;

static NEXT_WORKSPACE_TEST_ID: AtomicU64 = AtomicU64::new(0);

#[gpui::test]
fn closing_detached_window_keeps_sibling_window_running(cx: &mut TestAppContext) {
  let _guard = test_lock();
  crate::test_support::init_test_app(cx);
  install_fake_factory();

  let sibling = cx.add_window(|window, cx| MainWindow::new(window, cx));
  let detached = cx.add_window(|window, cx| MainWindow::new(window, cx));
  let sibling_terminal_id = sibling
    .update(cx, |main_window, _window, cx| {
      main_window.items[0]
        .split_container
        .get_active_terminal()
        .expect("sibling window should have an active terminal")
        .read(cx)
        .terminal()
        .entity_id()
    })
    .expect("reading sibling terminal should succeed");

  let dialog = detached
    .update(cx, |main_window, window, cx| {
      main_window.show_close_confirm_dialog(window, cx);
      main_window
        .close_confirm_dialog
        .clone()
        .expect("close dialog should be visible")
    })
    .expect("showing close dialog should succeed");
  detached
    .update(cx, |main_window, window, cx| {
      main_window.on_close_confirm_event(&dialog, &CloseConfirmEvent::Close, window, cx);
    })
    .expect("closing detached window should succeed");
  cx.run_until_parked();

  assert!(
    detached.root(cx).is_err(),
    "selected detached window should close"
  );
  sibling
    .root(cx)
    .expect("sibling window should remain open")
    .read_with(cx, |main_window, cx| {
      let terminal = main_window.items[0]
        .split_container
        .get_active_terminal()
        .expect("sibling terminal should remain available");
      assert_eq!(
        terminal.read(cx).terminal().entity_id(),
        sibling_terminal_id
      );
    });

  clear_terminal_session_factory_for_testing();
}

#[gpui::test]
fn closing_last_tab_closes_only_its_window(cx: &mut TestAppContext) {
  let _guard = test_lock();
  crate::test_support::init_test_app(cx);
  install_fake_factory();

  let sibling = cx.add_window(|window, cx| MainWindow::new(window, cx));
  let closing = cx.add_window(|window, cx| MainWindow::new(window, cx));
  let tab_index = closing
    .update(cx, |main_window, _window, _cx| main_window.items[0].index)
    .expect("reading closing tab should succeed");

  closing
    .update(cx, |main_window, window, cx| {
      main_window.remove_tab_by(tab_index, window, cx);
    })
    .expect("closing the last tab should succeed");
  cx.run_until_parked();

  assert!(
    closing.root(cx).is_err(),
    "window with no remaining tabs should close"
  );
  sibling
    .root(cx)
    .expect("sibling window should remain open")
    .read_with(cx, |main_window, _cx| {
      assert_eq!(main_window.items.len(), 1);
    });

  clear_terminal_session_factory_for_testing();
}

#[gpui::test]
fn active_window_routing_survives_the_original_window_closing(cx: &mut TestAppContext) {
  let _guard = test_lock();
  crate::test_support::init_test_app(cx);
  install_fake_factory();

  let first = cx.add_window(|window, cx| MainWindow::new(window, cx));
  let second = cx.add_window(|window, cx| MainWindow::new(window, cx));
  let first_view = first.root(cx).expect("first window root should exist");
  let second_view = second.root(cx).expect("second window root should exist");

  first
    .update(cx, |_root, window, cx| {
      crate::window_manager::register_window(window.window_handle(), &first_view, cx);
    })
    .expect("first window registration should succeed");
  second
    .update(cx, |_root, window, cx| {
      crate::window_manager::register_window(window.window_handle(), &second_view, cx);
      window.activate_window();
    })
    .expect("second window registration should succeed");
  cx.run_until_parked();

  let routed_entity = cx
    .update(|cx| {
      crate::window_manager::update_active_window(cx, |_main_window, _window, cx| cx.entity_id())
    })
    .expect("an active window should be available");
  assert_eq!(routed_entity, second_view.entity_id());

  second
    .update(cx, |_root, window, cx| {
      crate::window_manager::close_window(window, cx);
    })
    .expect("closing the active window should succeed");
  cx.run_until_parked();

  let routed_entity = cx
    .update(|cx| {
      crate::window_manager::update_active_window(cx, |_main_window, _window, cx| cx.entity_id())
    })
    .expect("the surviving window should be available");
  assert_eq!(routed_entity, first_view.entity_id());

  clear_terminal_session_factory_for_testing();
}

#[gpui::test]
fn ordinary_new_window_does_not_consume_saved_workspace(cx: &mut TestAppContext) {
  let _guard = test_lock();
  crate::test_support::init_test_app(cx);
  install_fake_factory();

  let workspace_directory = std::env::temp_dir().join(format!(
    "kazeterm-workspace-test-{}-{}",
    std::process::id(),
    NEXT_WORKSPACE_TEST_ID.fetch_add(1, Ordering::Relaxed),
  ));
  let _workspace_override = override_workspace_directory_for_testing(workspace_directory.clone());

  let saved = cx.add_window(|window, cx| MainWindow::new(window, cx));
  saved
    .update(cx, |main_window, window, cx| {
      main_window.insert_new_tab(window, cx);
      main_window.sync_ui_tree(cx);
      main_window.ui_tree.save_workspace();
    })
    .expect("saving a two-tab workspace should succeed");
  let workspace_path = UITreeStore::workspace_file_path();
  assert!(workspace_path.exists(), "saved workspace should exist");

  cx.update(|cx| {
    let mut config = cx.global::<::config::Config>().clone();
    config.window.restore_workspace = true;
    cx.set_global(config);
  });

  let fresh = cx.add_window(|window, cx| MainWindow::new_fresh(window, cx));
  fresh
    .root(cx)
    .expect("fresh window root should exist")
    .read_with(cx, |main_window, _cx| {
      assert_eq!(main_window.items.len(), 1);
    });
  assert!(
    workspace_path.exists(),
    "ordinary new windows must leave the saved workspace for the next launch"
  );

  let restored = cx.add_window(|window, cx| MainWindow::new(window, cx));
  restored
    .root(cx)
    .expect("restored window root should exist")
    .read_with(cx, |main_window, _cx| {
      assert_eq!(main_window.items.len(), 2);
    });
  assert!(
    !workspace_path.exists(),
    "the initial restoring window should consume the saved workspace"
  );

  clear_terminal_session_factory_for_testing();
}
