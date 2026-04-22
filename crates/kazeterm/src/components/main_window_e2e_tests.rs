//! End-to-end tests that boot a real `MainWindow` with a fake terminal
//! session factory. These exercise the full tab-management / event flow
//! without spawning any child processes.
//!
//! NOTE: these tests share a process-global factory override, so they must
//! run serially. A dedicated `Mutex` enforces that.
#![cfg(test)]

use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use gpui::TestAppContext;

use crate::components::MainWindow;
use crate::components::terminal_window::{
  clear_terminal_session_factory_for_testing, set_terminal_session_factory_for_testing,
};
use terminal::test_support::fake_terminal_session;

/// Global serializer: e2e tests install a process-global factory, so only
/// one may run at a time.
fn test_lock() -> MutexGuard<'static, ()> {
  static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
  LOCK
    .get_or_init(|| Mutex::new(()))
    .lock()
    .unwrap_or_else(|p| p.into_inner())
}

/// Records every call the MainWindow makes into the terminal-session factory.
#[derive(Default)]
struct FactoryCalls {
  programs: Vec<String>,
  args: Vec<Vec<String>>,
}

fn install_fake_factory() -> Arc<Mutex<FactoryCalls>> {
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
  });

  clear_terminal_session_factory_for_testing();
}
