#![cfg(test)]

use gpui::TestAppContext;

use super::{
  MainWindow,
  close_confirm_dialog::CloseConfirmEvent,
  main_window_e2e_tests::{install_fake_factory, test_lock},
  terminal_window::clear_terminal_session_factory_for_testing,
};

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
