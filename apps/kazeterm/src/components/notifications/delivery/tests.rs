use super::*;
use crate::components::main_window_e2e_tests::{install_fake_factory, test_lock};
use crate::components::terminal_window::clear_terminal_session_factory_for_testing;
use gpui::TestAppContext;
use terminal::program_status::ProgramStatusParser;
use terminal_kernel::event::Event;

fn titles(cx: &mut TestAppContext) -> Vec<String> {
  cx.update(|cx| {
    cx.try_global::<NotificationCenter>()
      .and_then(|center| center.captured.as_ref())
      .map(|messages| {
        messages
          .iter()
          .map(|message| message.title.clone())
          .collect()
      })
      .unwrap_or_default()
  })
}

fn settle(cx: &mut TestAppContext) {
  cx.run_until_parked();
  cx.executor()
    .advance_clock(COALESCE_DELAY + Duration::from_millis(1));
  cx.run_until_parked();
}

#[gpui::test]
fn osc_notifications_coalesce_legacy_signals_and_survive_tab_transfer(cx: &mut TestAppContext) {
  let _guard = test_lock();
  crate::test_support::init_test_app(cx);
  let calls = install_fake_factory();
  cx.update(|cx| {
    let config = cx.global_mut::<config::Config>();
    config.notification.enabled = true;
    config.notification.long_running_threshold_secs = 0;
    config.notification.interval_secs = 0;
  });
  cx.update(|cx| cx.global_mut::<config::Config>().animation.enabled = false);
  let window = cx.add_window(|window, cx| MainWindow::new(window, cx));
  let view = window
    .update(cx, |root, window, cx| {
      let view = root.active_terminal().unwrap();
      root.insert_new_tab(window, cx); // Keep the source window alive after transfer.
      view
    })
    .unwrap();
  let terminal = view.read_with(cx, |view, _| view.terminal().clone());
  let tracker = terminal.read_with(cx, |terminal, _| terminal.program_status_tracker.clone());
  let mut parser = ProgramStatusParser::new(tracker);
  let report = |parser: &mut ProgramStatusParser, bytes: &[u8], cx: &mut TestAppContext| {
    parser.advance(bytes, |_| {});
    terminal.update(cx, |terminal, cx| terminal.process_event(Event::Wakeup, cx));
    cx.run_until_parked();
  };

  window
    .update(cx, |root, _, cx| {
      root.maybe_send_notification(&view, NotificationReason::Bell, cx)
    })
    .unwrap();
  report(
    &mut parser,
    b"\x1b]7501;state=blocked:kind=permission:app=deploy\x07",
    cx,
  );
  assert_eq!(titles(cx), ["Kazeterm — Permission required"]);
  report(
    &mut parser,
    b"\x1b]7501;state=blocked:kind=permission:progress=20:app=deploy\x07",
    cx,
  );
  window
    .update(cx, |root, _, cx| {
      root.maybe_send_notification(&view, NotificationReason::CommandFinished, cx);
      root.maybe_send_notification(&view, NotificationReason::Bell, cx);
    })
    .unwrap();
  settle(cx);
  assert_eq!(titles(cx).len(), 1);

  report(
    &mut parser,
    b"\x1b]7501;state=working\x07\x1b]7501;state=done\x07",
    cx,
  );
  assert_eq!(titles(cx).len(), 2);
  window
    .update(cx, |root, _, cx| {
      root.maybe_send_notification(&view, NotificationReason::CommandFinished, cx)
    })
    .unwrap();
  settle(cx);
  assert_eq!(titles(cx).len(), 2);

  // A plain command in a new input cycle is not hidden by a retained old done record.
  terminal.update(cx, |terminal, _| {
    terminal.last_input_time += Duration::from_secs(1)
  });
  window
    .update(cx, |root, _, cx| {
      root.maybe_send_notification(&view, NotificationReason::Bell, cx);
      root.maybe_send_notification(&view, NotificationReason::CommandFinished, cx);
    })
    .unwrap();
  settle(cx);
  assert_eq!(titles(cx).last().unwrap(), "Kazeterm — Command finished");
  assert_eq!(titles(cx).len(), 3);
  report(&mut parser, b"\x1b]7501;state=done:id=late\x07", cx);
  assert_eq!(titles(cx).len(), 3); // Late OSC cannot duplicate a delivered fallback.

  let other = cx.add_window(|window, cx| MainWindow::new(window, cx));
  let other_terminal = other
    .update(cx, |root, _, cx| {
      root.active_terminal().unwrap().read(cx).terminal().clone()
    })
    .unwrap();
  let mut other_parser = ProgramStatusParser::new(
    other_terminal.read_with(cx, |terminal, _| terminal.program_status_tracker.clone()),
  );
  other_parser.advance(b"\x1b]7501;state=done\x07", |_| {});
  other_terminal.update(cx, |terminal, cx| terminal.process_event(Event::Wakeup, cx));
  cx.run_until_parked();
  assert_eq!(titles(cx).len(), 4); // Same task ID in another terminal is independent.

  terminal.update(cx, |terminal, _| {
    terminal.last_input_time += Duration::from_secs(1)
  });
  window
    .update(cx, |root, _, cx| {
      root.maybe_send_notification(&view, NotificationReason::CommandFinished, cx)
    })
    .unwrap();
  let count = calls.lock().unwrap().programs.len();
  let dragged = window
    .update(cx, |root, window, cx| {
      root.dragged_tab(0, window, cx).unwrap()
    })
    .unwrap();
  other
    .update(cx, |root, window, cx| {
      assert!(root.drop_tab_at(&dragged, None, window, cx))
    })
    .unwrap();
  report(&mut parser, b"\x1b]7501;state=done:id=late\x07", cx);
  settle(cx);
  assert_eq!(titles(cx).len(), 5);
  assert_eq!(titles(cx).last().unwrap(), "Kazeterm — Command finished");
  report(&mut parser, b"\x1b]7501;state=done:id=late\x07", cx);
  assert_eq!(titles(cx).len(), 5);
  assert_eq!(calls.lock().unwrap().programs.len(), count);
  clear_terminal_session_factory_for_testing();
}

#[gpui::test]
fn osc7501_switch_applies_live_without_replay_or_disabling_legacy_notifications(
  cx: &mut TestAppContext,
) {
  let _guard = test_lock();
  crate::test_support::init_test_app(cx);
  install_fake_factory();
  cx.update(|cx| {
    let notification = &mut cx.global_mut::<config::Config>().notification;
    notification.enabled = true;
    notification.osc7501_enabled = false;
    notification.long_running_threshold_secs = 0;
    notification.interval_secs = 0;
  });
  let window = cx.add_window(|window, cx| MainWindow::new(window, cx));
  let view = window
    .update(cx, |root, _, _| root.active_terminal().unwrap())
    .unwrap();
  let terminal = view.read_with(cx, |view, _| view.terminal().clone());
  let mut parser = ProgramStatusParser::new(
    terminal.read_with(cx, |terminal, _| terminal.program_status_tracker.clone()),
  );
  let mut report = |bytes: &[u8], cx: &mut TestAppContext| {
    parser.advance(bytes, |_| {});
    terminal.update(cx, |terminal, cx| terminal.process_event(Event::Wakeup, cx));
    cx.run_until_parked();
  };
  report(b"\x1b]7501;state=blocked:kind=permission\x07", cx);
  assert_eq!(
    terminal.read_with(cx, |terminal, _| terminal.program_status_records().len()),
    1
  );
  window
    .update(cx, |root, _, cx| {
      root.maybe_send_notification(&view, NotificationReason::Bell, cx);
      root.maybe_send_notification(&view, NotificationReason::CommandFinished, cx);
    })
    .unwrap();
  settle(cx);
  assert!(titles(cx).is_empty()); // Keep status indicators and deduplication, not fallback bypasses.

  cx.update(|cx| {
    cx.global_mut::<config::Config>()
      .notification
      .osc7501_enabled = true
  });
  report(
    b"\x1b]7501;state=blocked:kind=permission:progress=30\x07",
    cx,
  );
  assert!(titles(cx).is_empty()); // Enabling the source does not replay a consumed status.
  report(b"\x1b]7501;state=working\x07\x1b]7501;state=done\x07", cx);
  assert_eq!(titles(cx), ["Kazeterm — Task completed"]);

  cx.update(|cx| {
    cx.global_mut::<config::Config>()
      .notification
      .osc7501_enabled = false
  });
  report(b"\x1b]7501;state=working\x07\x1b]7501;state=error\x07", cx);
  assert_eq!(titles(cx).len(), 1);
  cx.update(|cx| {
    cx.global_mut::<config::Config>()
      .notification
      .osc7501_enabled = true
  });
  report(b"\x1b]7501;state=error:progress=30\x07", cx);
  assert_eq!(titles(cx).len(), 1);

  cx.update(|cx| {
    cx.global_mut::<config::Config>()
      .notification
      .osc7501_enabled = false
  });
  terminal.update(cx, |terminal, _| {
    terminal.last_input_time += Duration::from_secs(1)
  });
  report(b"\x1b]7501;state=clear\x07", cx);
  window
    .update(cx, |root, _, cx| {
      root.maybe_send_notification(&view, NotificationReason::Bell, cx);
      root.maybe_send_notification(&view, NotificationReason::CommandFinished, cx);
    })
    .unwrap();
  settle(cx);
  assert_eq!(titles(cx).len(), 2);
  assert_eq!(titles(cx).last().unwrap(), "Kazeterm — Command finished");
  clear_terminal_session_factory_for_testing();
}

#[gpui::test]
fn notification_queue_consumes_disabled_events_and_cancels_stale_or_closed_terminals(
  cx: &mut TestAppContext,
) {
  let _guard = test_lock();
  crate::test_support::init_test_app(cx);
  install_fake_factory();
  cx.update(|cx| cx.global_mut::<config::Config>().animation.enabled = false);
  let window = cx.add_window(|window, cx| MainWindow::new(window, cx));
  let view = window
    .update(cx, |root, window, cx| {
      let view = root.active_terminal().unwrap();
      root.insert_new_tab(window, cx);
      view
    })
    .unwrap();
  let terminal = view.read_with(cx, |view, _| view.terminal().clone());
  let mut parser = ProgramStatusParser::new(
    terminal.read_with(cx, |terminal, _| terminal.program_status_tracker.clone()),
  );
  parser.advance(b"\x1b]7501;state=blocked\x07", |_| {});
  terminal.update(cx, |terminal, cx| terminal.process_event(Event::Wakeup, cx));
  cx.run_until_parked();
  assert!(titles(cx).is_empty());
  cx.update(|cx| {
    let config = cx.global_mut::<config::Config>();
    config.notification.enabled = true;
    config.notification.interval_secs = 0;
    config.notification.long_running_threshold_secs = 3600;
  });
  parser.advance(b"\x1b]7501;state=blocked:progress=30\x07", |_| {});
  terminal.update(cx, |terminal, cx| terminal.process_event(Event::Wakeup, cx));
  cx.run_until_parked();
  assert!(titles(cx).is_empty());
  parser.advance(b"\x1b]7501;state=error\x07", |_| {});
  terminal.update(cx, |terminal, cx| terminal.process_event(Event::Wakeup, cx));
  cx.run_until_parked();
  assert_eq!(titles(cx), ["Kazeterm — Task failed"]); // Explicit OSC ignores heuristic idle threshold.

  cx.update(|cx| {
    cx.global_mut::<config::Config>()
      .notification
      .long_running_threshold_secs = 0
  });
  terminal.update(cx, |terminal, _| {
    terminal.last_input_time += Duration::from_secs(1)
  });
  window
    .update(cx, |root, _, cx| {
      root.maybe_send_notification(&view, NotificationReason::Bell, cx)
    })
    .unwrap();
  terminal.update(cx, |terminal, _| {
    terminal.last_input_time += Duration::from_secs(1)
  });
  settle(cx);
  assert_eq!(titles(cx).len(), 1);
  window
    .update(cx, |root, _, cx| {
      root.maybe_send_notification(&view, NotificationReason::CommandFinished, cx)
    })
    .unwrap();
  let id = view.entity_id();
  window
    .update(cx, |root, window, cx| {
      let index = root
        .items
        .iter()
        .find(|item| {
          item
            .split_container
            .all_terminals()
            .iter()
            .any(|(_, terminal)| terminal.entity_id() == id)
        })
        .unwrap()
        .index;
      root.remove_tab_by(index, window, cx);
    })
    .unwrap();
  cx.run_until_parked();
  window
    .update(cx, |root, _, _| {
      assert!(
        !root.items.iter().any(|item| item
          .split_container
          .all_terminals()
          .iter()
          .any(|(_, terminal)| terminal.entity_id() == id)),
        "terminal must actually be closed before testing cancellation"
      )
    })
    .unwrap();
  drop(view);
  settle(cx);
  assert_eq!(titles(cx).len(), 1);
  cx.update(|cx| {
    assert!(
      !cx
        .global::<NotificationCenter>()
        .terminals
        .contains_key(&id)
    )
  });
  clear_terminal_session_factory_for_testing();
}
