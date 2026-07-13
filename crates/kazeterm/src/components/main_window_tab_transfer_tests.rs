#![cfg(test)]

use std::collections::HashSet;

use gpui::{TestAppContext, point, px};

use super::{
  MainWindow,
  main_window_e2e_tests::{install_fake_factory, test_lock},
  terminal_window::clear_terminal_session_factory_for_testing,
};

#[gpui::test]
fn moving_tab_between_windows_preserves_terminal_entities(cx: &mut TestAppContext) {
  let _guard = test_lock();
  crate::test_support::init_test_app(cx);
  let calls = install_fake_factory();

  let source = cx.add_window(|window, cx| MainWindow::new(window, cx));
  let target = cx.add_window(|window, cx| MainWindow::new(window, cx));
  source
    .update(cx, |root: &mut MainWindow, window, cx| {
      root.insert_new_tab(window, cx);
    })
    .expect("source tab setup should succeed");
  cx.run_until_parked();

  let calls_before_transfer = calls.lock().unwrap().programs.len();
  let (dragged, terminal_view_id, terminal_id) = source
    .update(cx, |root: &mut MainWindow, window, cx| {
      let terminal = root.items[1]
        .split_container
        .get_active_terminal()
        .expect("source tab should have an active terminal");
      (
        root
          .dragged_tab(1, window, cx)
          .expect("source tab should produce a drag payload"),
        terminal.entity_id(),
        terminal.read(cx).terminal().entity_id(),
      )
    })
    .expect("reading source tab state should succeed");

  target
    .update(cx, |root: &mut MainWindow, window, cx| {
      root.drop_tab_at(&dragged, None, window, cx);
    })
    .expect("cross-window tab drop should succeed");
  cx.run_until_parked();

  assert_eq!(
    calls.lock().unwrap().programs.len(),
    calls_before_transfer,
    "moving a tab must not create a replacement terminal session",
  );
  source
    .root(cx)
    .expect("source root should still exist")
    .read_with(cx, |main_window, _| {
      assert_eq!(main_window.items.len(), 1);
    });
  target
    .root(cx)
    .expect("target root should exist")
    .read_with(cx, |main_window, cx| {
      assert_eq!(main_window.items.len(), 2);
      let terminal = main_window.items[1]
        .split_container
        .get_active_terminal()
        .expect("transferred tab should retain its terminal");
      assert_eq!(terminal.entity_id(), terminal_view_id);
      assert_eq!(terminal.read(cx).terminal().entity_id(), terminal_id);
      let terminal_indices = main_window
        .items
        .iter()
        .flat_map(|item| item.split_container.all_terminals())
        .map(|(_, terminal)| terminal.read(cx).index)
        .collect::<HashSet<_>>();
      assert_eq!(terminal_indices.len(), 2);
    });

  clear_terminal_session_factory_for_testing();
}

#[gpui::test]
fn moving_tab_from_another_window_into_split_preserves_terminal_entities(cx: &mut TestAppContext) {
  let _guard = test_lock();
  crate::test_support::init_test_app(cx);
  let calls = install_fake_factory();

  let source = cx.add_window(|window, cx| MainWindow::new(window, cx));
  let target = cx.add_window(|window, cx| MainWindow::new(window, cx));
  source
    .update(cx, |root: &mut MainWindow, window, cx| {
      root.insert_new_tab(window, cx);
    })
    .expect("source tab setup should succeed");
  cx.run_until_parked();

  let calls_before_transfer = calls.lock().unwrap().programs.len();
  let (dragged, terminal_view_id, terminal_id) = source
    .update(cx, |root: &mut MainWindow, window, cx| {
      let terminal = root.items[1]
        .split_container
        .get_active_terminal()
        .expect("source tab should have an active terminal");
      (
        root
          .dragged_tab(1, window, cx)
          .expect("source tab should produce a drag payload"),
        terminal.entity_id(),
        terminal.read(cx).terminal().entity_id(),
      )
    })
    .expect("reading source tab state should succeed");
  let target_pane_id = target
    .update(cx, |root: &mut MainWindow, _window, _cx| {
      root.items[0]
        .split_container
        .all_terminals()
        .first()
        .map(|(pane_id, _)| *pane_id)
        .expect("target tab should have a terminal pane")
    })
    .expect("reading target pane should succeed");

  target
    .update(cx, |root: &mut MainWindow, window, cx| {
      root.drop_tab_into_split(&dragged, target_pane_id, window, cx);
    })
    .expect("cross-window split drop should succeed");
  cx.run_until_parked();

  assert_eq!(
    calls.lock().unwrap().programs.len(),
    calls_before_transfer,
    "moving a tab into a split must not create a replacement terminal session",
  );
  source
    .root(cx)
    .expect("source root should still exist")
    .read_with(cx, |main_window, _| {
      assert_eq!(main_window.items.len(), 1);
    });
  target
    .root(cx)
    .expect("target root should exist")
    .read_with(cx, |main_window, cx| {
      assert_eq!(main_window.items.len(), 1);
      let terminals = main_window.items[0].split_container.all_terminals();
      assert_eq!(terminals.len(), 2);
      let terminal_indices = terminals
        .iter()
        .map(|(_, terminal)| terminal.read(cx).index)
        .collect::<HashSet<_>>();
      assert_eq!(terminal_indices.len(), 2);
      let transferred = terminals
        .iter()
        .map(|(_, terminal)| terminal)
        .find(|terminal| terminal.entity_id() == terminal_view_id)
        .expect("split should contain the transferred terminal view");
      assert_eq!(transferred.read(cx).terminal().entity_id(), terminal_id);
    });

  clear_terminal_session_factory_for_testing();
}

#[gpui::test]
fn moving_tab_from_single_tab_window_closes_empty_source(cx: &mut TestAppContext) {
  let _guard = test_lock();
  crate::test_support::init_test_app(cx);
  let calls = install_fake_factory();

  let source = cx.add_window(|window, cx| MainWindow::new(window, cx));
  let target = cx.add_window(|window, cx| MainWindow::new(window, cx));
  cx.run_until_parked();

  let calls_before_transfer = calls.lock().unwrap().programs.len();
  let (dragged, terminal_view_id, terminal_id) = source
    .update(cx, |root: &mut MainWindow, window, cx| {
      let terminal = root.items[0]
        .split_container
        .get_active_terminal()
        .expect("source tab should have an active terminal");
      (
        root
          .dragged_tab(0, window, cx)
          .expect("source tab should produce a drag payload"),
        terminal.entity_id(),
        terminal.read(cx).terminal().entity_id(),
      )
    })
    .expect("reading source tab state should succeed");

  target
    .update(cx, |root: &mut MainWindow, window, cx| {
      root.drop_tab_at(&dragged, None, window, cx);
    })
    .expect("cross-window tab drop should succeed");
  cx.run_until_parked();

  assert_eq!(
    calls.lock().unwrap().programs.len(),
    calls_before_transfer,
    "merging a single-tab window must not create a terminal session",
  );
  assert!(source.root(cx).is_err(), "empty source window should close");
  target
    .root(cx)
    .expect("target root should exist")
    .read_with(cx, |main_window, cx| {
      assert_eq!(main_window.items.len(), 2);
      let terminal = main_window.items[1]
        .split_container
        .get_active_terminal()
        .expect("transferred tab should retain its terminal");
      assert_eq!(terminal.entity_id(), terminal_view_id);
      assert_eq!(terminal.read(cx).terminal().entity_id(), terminal_id);
    });

  clear_terminal_session_factory_for_testing();
}

#[gpui::test]
fn cross_window_drop_prefers_most_recently_active_over_newest_window(cx: &mut TestAppContext) {
  let _guard = test_lock();
  crate::test_support::init_test_app(cx);
  install_fake_factory();

  let older_target = cx.add_window(|window, cx| MainWindow::new(window, cx));
  let newer_target = cx.add_window(|window, cx| MainWindow::new(window, cx));
  let source = cx.add_window(|window, cx| MainWindow::new(window, cx));
  let older_view = older_target
    .root(cx)
    .expect("older target root should exist");
  let newer_view = newer_target
    .root(cx)
    .expect("newer target root should exist");
  let source_view = source.root(cx).expect("source root should exist");

  older_target
    .update(cx, |_root, window, cx| {
      crate::window_manager::register_window(window.window_handle(), &older_view, cx);
    })
    .expect("older target registration should succeed");
  newer_target
    .update(cx, |_root, window, cx| {
      crate::window_manager::register_window(window.window_handle(), &newer_view, cx);
    })
    .expect("newer target registration should succeed");
  source
    .update(cx, |_root, window, cx| {
      crate::window_manager::register_window(window.window_handle(), &source_view, cx);
    })
    .expect("source registration should succeed");

  older_target
    .update(cx, |_root, window, _cx| window.activate_window())
    .expect("older target activation should succeed");
  cx.run_until_parked();
  source
    .update(cx, |_root, window, _cx| window.activate_window())
    .expect("source activation should succeed");
  cx.run_until_parked();

  let dragged = source
    .update(cx, |root: &mut MainWindow, window, cx| {
      root
        .dragged_tab(0, window, cx)
        .expect("source tab should produce a drag payload")
    })
    .expect("reading source tab state should succeed");
  let drop_position = older_target
    .update(cx, |_root, window, _cx| {
      let bounds = window.bounds();
      point(bounds.origin.x + px(1.0), bounds.origin.y + px(1.0))
    })
    .expect("reading target bounds should succeed");

  let accepted = newer_target
    .update(cx, |_root, _window, cx| {
      crate::window_manager::drop_tab_on_existing_window(&dragged, drop_position, cx)
    })
    .expect("cross-window routing should succeed");
  assert!(accepted, "drop should be accepted by an overlapping target");
  cx.run_until_parked();

  older_target
    .root(cx)
    .expect("older target root should exist")
    .read_with(cx, |main_window, _| {
      assert_eq!(main_window.items.len(), 2);
    });
  newer_target
    .root(cx)
    .expect("newer target root should exist")
    .read_with(cx, |main_window, _| {
      assert_eq!(main_window.items.len(), 1);
    });

  clear_terminal_session_factory_for_testing();
}
