use std::sync::{Arc, Mutex};

use clap::Parser;
use gpui::{Entity, TestAppContext};
use kazeterm_event_system::remote_control::{RemoteRequest, RemoteResponse};
use serde_json::{Value, json};

use super::MainWindow;
use super::main_window_e2e_tests::test_lock;
use super::terminal_window::{
  clear_terminal_session_factory_for_testing, set_terminal_session_factory_for_testing,
};
use crate::remote_control::{RemoteIds, execute};

type Writes = Arc<Mutex<Vec<Vec<u8>>>>;

fn install_factory() -> Arc<Mutex<Vec<Writes>>> {
  let writes = Arc::new(Mutex::new(Vec::new()));
  let record = writes.clone();
  set_terminal_session_factory_for_testing(Box::new(move |_, _, _, _| {
    let (terminal, events, writes, _) = terminal::test_support::fake_terminal_session(80, 24);
    record.lock().unwrap().push(writes);
    Ok((terminal, events))
  }));
  writes
}

fn command(
  cmd: &str,
  payload: Value,
  ids: &mut RemoteIds,
  cx: &mut TestAppContext,
) -> RemoteResponse {
  let request =
    RemoteRequest::from_value(json!({"cmd":cmd, "payload":payload, "version":[0,49,2]})).unwrap();
  cx.update(|cx| execute(&request, ids, cx))
}

fn listing(ids: &mut RemoteIds, cx: &mut TestAppContext) -> Value {
  let result = command("ls", json!({}), ids, cx);
  assert!(result.ok, "{:?}", result.error);
  serde_json::from_str(result.data.unwrap().as_str().unwrap()).unwrap()
}

#[test]
fn remote_control_cli_requires_explicit_consent_and_keeps_legacy_events() {
  assert!(
    crate::Args::try_parse_from(["kazeterm"])
      .unwrap()
      .listen_on
      .is_none()
  );
  assert!(crate::Args::try_parse_from(["kazeterm", "--listen-on", "unix:/tmp/rc.sock"]).is_err());
  assert!(crate::Args::try_parse_from(["kazeterm", "--allow-remote-control"]).is_err());
  let args = crate::Args::try_parse_from([
    "kazeterm",
    "--allow-remote-control",
    "--listen-on",
    "unix:/tmp/rc.sock",
    "--event-source",
    "stdio",
  ])
  .unwrap();
  assert!(args.allow_remote_control);
  assert!(matches!(
    args.to_event_source_config(),
    kazeterm_event_system::EventSourceConfig::Stdio
  ));
}

#[gpui::test]
fn remote_control_lists_routes_input_and_rejects_unsupported_payloads(cx: &mut TestAppContext) {
  let _guard = test_lock();
  crate::test_support::init_test_app(cx);
  let writes = install_factory();
  let first = cx.add_window(|window, cx| MainWindow::new(window, cx));
  let second = cx.add_window(|window, cx| MainWindow::new(window, cx));
  for handle in [first, second] {
    handle
      .update(cx, |root, window, cx| {
        let owner: Entity<MainWindow> = cx.entity();
        crate::window_manager::register_window(window.window_handle(), &owner, cx);
        let model = root.active_terminal().unwrap().read(cx).terminal().clone();
        model.update(cx, |model, _| {
          model.title_text = format!("terminal-{}", model.term.columns())
        });
      })
      .unwrap();
  }
  cx.run_until_parked();
  cx.update(|cx| crate::window_manager::mark_window_active(first.into(), cx));
  let mut ids = RemoteIds::default();
  let before = listing(&mut ids, cx);
  assert_eq!(before.as_array().unwrap().len(), 2);
  let first_id = before[0]["tabs"][0]["windows"][0]["id"].as_u64().unwrap();
  let second_id = before[1]["tabs"][0]["windows"][0]["id"].as_u64().unwrap();
  let second_tab_id = before[1]["tabs"][0]["id"].as_u64().unwrap();
  assert_ne!(first_id, second_id);
  let selection = format!("id:{second_id}");
  let response = command(
    "send-text",
    json!({"match":selection, "data":"base64:AEFC/w=="}),
    &mut ids,
    cx,
  );
  assert!(response.ok);
  let records = writes.lock().unwrap();
  assert!(records[0].lock().unwrap().is_empty());
  assert_eq!(*records[1].lock().unwrap(), [vec![0, 65, 66, 255]]);
  drop(records);
  for invalid in [
    json!({"match":"title:.*", "data":"text:must not send"}),
    json!({"match":format!("id:{first_id}"), "data":"base64:***"}),
    json!({"data":"text:must not send", "unknown":true}),
    json!({"data":"text:must not send", "bracketed_paste":"enable"}),
  ] {
    assert!(!command("send-text", invalid, &mut ids, cx).ok);
  }
  assert!(writes.lock().unwrap()[0].lock().unwrap().is_empty());
  assert!(!command("launch", json!({}), &mut ids, cx).ok);
  assert!(!command("ls", json!({"all_env_vars":true}), &mut ids, cx).ok);
  assert!(!command("get-text", json!({"ansi":true}), &mut ids, cx).ok);
  assert!(!command("get-text", json!({"self":true}), &mut ids, cx).ok);
  assert!(
    command(
      "get-text",
      json!({"match":selection, "extent":"screen"}),
      &mut ids,
      cx
    )
    .data
    .unwrap()
    .is_string()
  );
  assert!(!command("get-text", json!({"match":"id:999999999"}), &mut ids, cx).ok);

  assert!(
    command(
      "set-tab-title",
      json!({"match":format!("id:{second_tab_id}"), "title":"Remote title"}),
      &mut ids,
      cx
    )
    .ok
  );
  second
    .update(cx, |root, _, _| {
      assert_eq!(root.items[0].display_title(), "Remote title")
    })
    .unwrap();
  assert_eq!(
    listing(&mut ids, cx)[1]["tabs"][0]["windows"][0]["id"],
    second_id
  );
  assert!(command("focus-window", json!({"match":selection}), &mut ids, cx).ok);
  cx.run_until_parked();
  let after = listing(&mut ids, cx);
  assert!(
    after
      .as_array()
      .unwrap()
      .iter()
      .flat_map(|os| os["tabs"].as_array().unwrap())
      .flat_map(|tab| tab["windows"].as_array().unwrap())
      .any(|window| window["id"] == second_id && window["is_focused"] == true)
  );
  clear_terminal_session_factory_for_testing();
}

#[gpui::test]
fn remote_control_pane_matching_and_focus_reuse_ui_actions_and_existing_sessions(
  cx: &mut TestAppContext,
) {
  let _guard = test_lock();
  crate::test_support::init_test_app(cx);
  let writes = install_factory();
  let window = cx.add_window(|window, cx| MainWindow::new(window, cx));
  window
    .update(cx, |root, window, cx| {
      let owner = cx.entity();
      crate::window_manager::register_window(window.window_handle(), &owner, cx);
      root.split_pane_horizontal(window, cx);
    })
    .unwrap();
  cx.run_until_parked();
  let mut ids = RemoteIds::default();
  let snapshot = listing(&mut ids, cx);
  let panes = snapshot[0]["tabs"][0]["windows"].as_array().unwrap();
  assert_eq!(panes.len(), 2);
  let id = panes[0]["id"].as_u64().unwrap();
  assert!(
    command(
      "focus-window",
      json!({"match":format!("id:{id}")}),
      &mut ids,
      cx
    )
    .ok
  );
  cx.run_until_parked();
  assert!(
    command(
      "send-text",
      json!({"data":"text:active only"}),
      &mut ids,
      cx
    )
    .ok
  );
  let records = writes.lock().unwrap();
  assert_eq!(*records[0].lock().unwrap(), [b"active only".to_vec()]);
  assert!(records[1].lock().unwrap().is_empty());
  drop(records);
  assert!(
    command(
      "send-text",
      json!({"data":"text:background only", "all":true, "exclude_active":true}),
      &mut ids,
      cx
    )
    .ok
  );
  assert_eq!(
    *writes.lock().unwrap()[1].lock().unwrap(),
    [b"background only".to_vec()]
  );
  assert_eq!(writes.lock().unwrap().len(), 2); // No PTY/session recreation.
  clear_terminal_session_factory_for_testing();
}
