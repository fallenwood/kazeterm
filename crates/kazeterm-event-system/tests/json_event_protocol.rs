//! Integration tests for JSON → `AppEvent` conversion.
//!
//! Ensures the CLI/socket event protocol documented in `kazeterm-event-system`
//! stays wire-compatible across changes.

use kazeterm_event_system::{AppEvent, JsonEvent};

fn parse(line: &str) -> JsonEvent {
  serde_json::from_str(line).unwrap_or_else(|e| panic!("parse failure on {line:?}: {e}"))
}

#[test]
fn simple_events_parse_and_convert() {
  assert_eq!(
    AppEvent::from(parse(r#"{"event":"NextTab"}"#)),
    AppEvent::NextTab
  );
  assert_eq!(
    AppEvent::from(parse(r#"{"event":"PreviousTab"}"#)),
    AppEvent::PreviousTab
  );
  assert_eq!(
    AppEvent::from(parse(r#"{"event":"ToggleSearch"}"#)),
    AppEvent::ToggleSearch
  );
  assert_eq!(
    AppEvent::from(parse(r#"{"event":"Quit"}"#)),
    AppEvent::Quit
  );
}

#[test]
fn new_terminal_with_profile_round_trips_fields() {
  let ev = parse(
    r#"{"event":"NewTerminalWithProfile","profile_name":"bash","working_directory":"/home/x"}"#,
  );
  let got = AppEvent::from(ev);
  match got {
    AppEvent::NewTerminalWithProfile {
      profile_name,
      working_directory,
    } => {
      assert_eq!(profile_name, "bash");
      assert_eq!(working_directory.as_deref(), Some("/home/x"));
    }
    other => panic!("unexpected variant: {other:?}"),
  }
}

#[test]
fn switch_to_tab_parses_position() {
  let ev = parse(r#"{"event":"SwitchToTab","position":7}"#);
  assert_eq!(AppEvent::from(ev), AppEvent::SwitchToTab { position: 7 });
}

#[test]
fn send_text_accepts_unicode() {
  let ev = parse(r#"{"event":"SendTextToTerminal","text":"echo 你好\n"}"#);
  assert_eq!(
    AppEvent::from(ev),
    AppEvent::SendTextToTerminal {
      text: String::from("echo 你好\n"),
    }
  );
}

#[test]
fn unknown_events_fail_cleanly() {
  let result: Result<JsonEvent, _> = serde_json::from_str(r#"{"event":"NotAnEvent"}"#);
  assert!(result.is_err(), "expected parse failure for unknown event");
}

#[test]
fn custom_event_preserves_payload() {
  let ev = parse(r#"{"event":"Custom","name":"ping","data":"{\"v\":1}"}"#);
  assert_eq!(
    AppEvent::from(ev),
    AppEvent::Custom {
      name: "ping".into(),
      data: "{\"v\":1}".into(),
    }
  );
}
