//! OSC 7501 program status protocol (revision 0.4).
//!
//! The streaming observer leaves PTY bytes untouched. Both backends share a
//! bounded record store with the UI, so output cannot build an unbounded queue.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use base64::Engine as _;
use base64::engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig};

const MAX_SEQUENCE_BYTES: usize = 4096;
const MAX_RECORDS: usize = 256;
pub const FEATURE_REPLY: &[u8] =
  b"\x1b]7501;?:states=idle,working,done,blocked,error:kinds=permission,question,auth\x1b\\";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgramState {
  Idle,
  Working,
  Done,
  Blocked,
  Error,
}

impl ProgramState {
  pub fn label(self) -> &'static str {
    match self {
      Self::Idle => "Idle",
      Self::Working => "Working",
      Self::Done => "Done",
      Self::Blocked => "Blocked",
      Self::Error => "Error",
    }
  }

  /// Prefer records that need user action over ongoing or completed work.
  pub fn priority(self) -> u8 {
    match self {
      Self::Blocked => 4,
      Self::Error => 3,
      Self::Done => 2,
      Self::Working => 1,
      Self::Idle => 0,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockedKind {
  Permission,
  Question,
  Auth,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProgramStatusRecord {
  /// Empty string denotes the root record.
  pub id: String,
  pub state: ProgramState,
  /// Stable across metadata refreshes; changes on state/kind transitions or recreation.
  pub state_revision: u64,
  pub kind: Option<BlockedKind>,
  pub progress: Option<u8>,
  pub app: Option<String>,
  pub title: Option<String>,
  pub msg: Option<String>,
}

enum Report {
  Set(ProgramStatusRecord),
  Clear(String),
}

fn valid_segment(value: &str) -> bool {
  !value.is_empty()
    && value.len() <= 32
    && value
      .bytes()
      .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'+' | b'-'))
}

fn valid_id(value: &str) -> bool {
  value.len() <= 128 && value.split('/').count() <= 8 && value.split('/').all(valid_segment)
}

fn decode_text(value: &str, max_decoded: usize) -> Option<String> {
  let engine = GeneralPurpose::new(
    &base64::alphabet::STANDARD,
    GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent),
  );
  let bytes = engine.decode(value).ok()?;
  if bytes.len() > max_decoded {
    return None;
  }
  let text = String::from_utf8(bytes).ok()?;
  (!text.chars().any(char::is_control)).then_some(text)
}

fn parse_report(body: &[u8]) -> Option<Report> {
  let mut state = None;
  let mut id = None;
  let mut kind = None;
  let mut progress = None;
  let mut app = None;
  let mut title = None;
  let mut msg = None;

  // Validate every occurrence before applying anything, including overwritten
  // values and keys that do not apply to the final state.
  for pair in body.split(|byte| *byte == b':') {
    let Ok(pair) = std::str::from_utf8(pair) else {
      continue;
    };
    let Some((key, value)) = pair.split_once('=') else {
      continue;
    };
    let key = key.trim();
    let value = value.trim();
    if key.len() > 16 {
      return None;
    }
    match key {
      "msg" if value.len() > 2732 => return None,
      "title" if value.len() > 256 => return None,
      "app" if value.len() > 32 => return None,
      "id"
        if value.len() > 128
          || value.split('/').count() > 8
          || value.split('/').any(|segment| segment.len() > 32) =>
      {
        return None;
      }
      _ => {}
    }
    if key.is_empty()
      || !key.bytes().all(|b| b.is_ascii_lowercase())
      || !value.bytes().all(|b| {
        b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b',' | b'+' | b'/' | b'=' | b'-')
      })
    {
      continue;
    }
    match key {
      "state" => state = Some(value),
      "id" => id = Some(value),
      "kind" => kind = Some(value),
      "progress" => progress = Some(value),
      "app" => app = Some(value),
      "title" => title = Some(decode_text(value, 192)?),
      "msg" => msg = Some(decode_text(value, 2048)?),
      _ => {}
    }
  }

  let id = match id {
    Some(id) if valid_id(id) => id,
    Some(_) => return None,
    None => "",
  };
  let state = match state? {
    "idle" => ProgramState::Idle,
    "working" => ProgramState::Working,
    "done" => ProgramState::Done,
    "blocked" => ProgramState::Blocked,
    "error" => ProgramState::Error,
    "clear" => return Some(Report::Clear(id.to_owned())),
    _ => return None,
  };
  let kind = if state == ProgramState::Blocked {
    match kind {
      Some("permission") => Some(BlockedKind::Permission),
      Some("question") => Some(BlockedKind::Question),
      Some("auth") => Some(BlockedKind::Auth),
      _ => None,
    }
  } else {
    None
  };
  let progress = if matches!(state, ProgramState::Working | ProgramState::Blocked) {
    progress
      .filter(|value| !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()))
      .and_then(|value| value.parse::<u8>().ok())
      .filter(|value| *value <= 100)
  } else {
    None
  };
  Some(Report::Set(ProgramStatusRecord {
    id: id.to_owned(),
    state,
    state_revision: 0,
    kind,
    progress,
    app: app.filter(|value| valid_segment(value)).map(str::to_owned),
    title,
    msg,
  }))
}

#[derive(Default)]
struct RecordStore {
  // Oldest update first. Replacing a record moves it to the back.
  records: VecDeque<ProgramStatusRecord>,
  revision: u64,
}

impl RecordStore {
  fn apply(&mut self, report: Report) {
    match report {
      Report::Clear(id) => {
        let prefix = format!("{id}/");
        self.retain(|record| !id.is_empty() && record.id != id && !record.id.starts_with(&prefix));
      }
      Report::Set(mut record) => {
        record.state_revision = self
          .records
          .iter()
          .find(|previous| {
            previous.id == record.id
              && previous.state == record.state
              && previous.kind == record.kind
          })
          .map_or_else(
            || self.revision.wrapping_add(1),
            |previous| previous.state_revision,
          );
        self.records.retain(|existing| existing.id != record.id);
        if self.records.len() == MAX_RECORDS {
          self.records.pop_front();
        }
        self.records.push_back(record);
        self.revision = self.revision.wrapping_add(1);
      }
    }
  }

  fn retain(&mut self, keep: impl FnMut(&ProgramStatusRecord) -> bool) {
    let old_len = self.records.len();
    self.records.retain(keep);
    if self.records.len() != old_len {
      self.revision = self.revision.wrapping_add(1);
    }
  }

  fn snapshot(&self) -> Vec<ProgramStatusRecord> {
    self
      .records
      .iter()
      .map(|record| {
        let mut record = record.clone();
        if record.app.is_none() {
          let mut ancestor = record.id.as_str();
          while !ancestor.is_empty() {
            ancestor = ancestor.rsplit_once('/').map_or("", |(parent, _)| parent);
            if let Some(app) = self
              .records
              .iter()
              .find(|candidate| candidate.id == ancestor)
              .and_then(|candidate| candidate.app.as_ref())
            {
              record.app = Some(app.clone());
              break;
            }
          }
        }
        record
      })
      .collect()
  }
}

#[derive(Clone, Default)]
pub struct ProgramStatusTracker(Arc<Mutex<RecordStore>>);

impl ProgramStatusTracker {
  pub fn snapshot_since(&self, revision: u64) -> Option<(u64, Vec<ProgramStatusRecord>)> {
    let store = self.0.lock().unwrap();
    (store.revision != revision).then(|| (store.revision, store.snapshot()))
  }

  /// Called for OSC 133 A and when the attached process exits. Completed and
  /// failed records survive; idle records are also retained.
  pub fn expire_running(&self) {
    self
      .0
      .lock()
      .unwrap()
      .retain(|record| !matches!(record.state, ProgramState::Working | ProgramState::Blocked));
  }
}

#[derive(Clone, Copy, Default)]
enum ScanState {
  #[default]
  Ground,
  Escape,
  Osc,
  OscEscape,
  String,
  StringEscape,
}

/// An incremental observer for OSC 7501, OSC 133 A, and RIS. Escape sequences
/// inside DCS/APC/PM/SOS strings are not mistaken for terminal commands.
pub struct ProgramStatusParser {
  tracker: ProgramStatusTracker,
  state: ScanState,
  body: Vec<u8>,
  sequence_bytes: usize,
}

impl ProgramStatusParser {
  pub fn new(tracker: ProgramStatusTracker) -> Self {
    Self {
      tracker,
      state: ScanState::Ground,
      body: Vec::new(),
      sequence_bytes: 0,
    }
  }

  pub fn advance(&mut self, bytes: &[u8], mut reply: impl FnMut(&[u8])) {
    self.advance_inner(bytes, &mut reply);
  }

  fn advance_inner(&mut self, bytes: &[u8], reply: &mut dyn FnMut(&[u8])) {
    for &byte in bytes {
      if matches!(self.state, ScanState::Osc | ScanState::OscEscape) {
        self.sequence_bytes = self.sequence_bytes.saturating_add(1);
      }
      // CAN and SUB abort an in-flight control sequence.
      if matches!(byte, 0x18 | 0x1a) {
        self.state = ScanState::Ground;
        self.body.clear();
        continue;
      }
      match self.state {
        ScanState::Ground => {
          if byte == 0x1b {
            self.state = ScanState::Escape;
          }
        }
        ScanState::Escape => {
          self.state = match byte {
            b']' => {
              self.body.clear();
              self.sequence_bytes = 2;
              ScanState::Osc
            }
            b'P' | b'_' | b'^' | b'X' => ScanState::String,
            b'c' => {
              self.tracker.0.lock().unwrap().retain(|_| false);
              ScanState::Ground
            }
            0x1b => ScanState::Escape,
            _ => ScanState::Ground,
          };
        }
        ScanState::Osc => match byte {
          0x07 => {
            self.finish_osc(reply);
            self.state = ScanState::Ground;
          }
          0x1b => self.state = ScanState::OscEscape,
          _ => {
            if self.sequence_bytes <= MAX_SEQUENCE_BYTES {
              self.body.push(byte);
            }
          }
        },
        ScanState::OscEscape => {
          if byte == b'\\' {
            self.finish_osc(reply);
            self.state = ScanState::Ground;
          } else {
            // ESC followed by anything other than ST aborts OSC and starts
            // a new escape sequence, just as the terminal parser does.
            self.body.clear();
            self.state = ScanState::Escape;
            self.advance_inner(&[byte], reply);
          }
        }
        ScanState::String => {
          if byte == 0x1b {
            self.state = ScanState::StringEscape;
          }
        }
        ScanState::StringEscape => {
          self.state = if byte == b'\\' {
            ScanState::Ground
          } else if byte == 0x1b {
            ScanState::StringEscape
          } else {
            ScanState::String
          };
        }
      }
    }
  }

  fn finish_osc(&mut self, reply: &mut dyn FnMut(&[u8])) {
    if self.sequence_bytes <= MAX_SEQUENCE_BYTES {
      if let Some(body) = self.body.strip_prefix(b"7501;") {
        if body == b"?" {
          reply(FEATURE_REPLY);
        } else if let Some(report) = parse_report(body) {
          self.tracker.0.lock().unwrap().apply(report);
        }
      } else if self.body == b"133;A" || self.body.starts_with(b"133;A;") {
        self.tracker.expire_running();
      }
    }
    self.body.clear();
  }
}

/// Free text stays literal, and invisible formatting is removed before it is
/// displayed outside the grid (notably bidi overrides and zero-width text).
pub fn display_text(text: &str) -> String {
  text
    .chars()
    .filter(|ch| {
      !matches!(*ch,
      '\u{00ad}' | '\u{034f}' | '\u{061c}' | '\u{180e}'
      | '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}'
      | '\u{2060}'..='\u{206f}' | '\u{feff}' | '\u{fff9}'..='\u{fffb}'
      | '\u{e0000}'..='\u{e007f}')
    })
    .collect()
}

#[cfg(test)]
mod tests {
  use base64::engine::general_purpose::STANDARD;

  use super::*;

  fn report(parser: &mut ProgramStatusParser, body: &str) {
    parser.advance(format!("\x1b]7501;{body}\x1b\\").as_bytes(), |_| {
      panic!("reports must never write a reply");
    });
  }

  fn records(tracker: &ProgramStatusTracker) -> Vec<ProgramStatusRecord> {
    tracker.0.lock().unwrap().snapshot()
  }

  #[test]
  fn program_status_state_revision_tracks_lifecycle_not_repeated_reports() {
    let tracker = ProgramStatusTracker::default();
    let mut parser = ProgramStatusParser::new(tracker.clone());
    report(&mut parser, "state=done:id=task:app=build");
    let first = records(&tracker)[0].state_revision;
    report(&mut parser, "state=done:id=task:app=build:msg=bmV3");
    assert_eq!(records(&tracker)[0].state_revision, first);
    // Intermediate working state can be coalesced out of the UI snapshot.
    report(&mut parser, "state=working:id=task");
    report(&mut parser, "state=done:id=task");
    let second = records(&tracker)[0].state_revision;
    assert_ne!(second, first);
    report(&mut parser, "state=clear:id=task");
    report(&mut parser, "state=done:id=task");
    assert_ne!(records(&tracker)[0].state_revision, second);
    report(
      &mut parser,
      "state=blocked:id=task:kind=permission:progress=10",
    );
    let permission = records(&tracker)[0].state_revision;
    report(
      &mut parser,
      "state=blocked:id=task:kind=permission:progress=20",
    );
    assert_eq!(records(&tracker)[0].state_revision, permission);
    report(&mut parser, "state=blocked:id=task:kind=auth");
    assert_ne!(records(&tracker)[0].state_revision, permission);
  }

  #[test]
  fn program_status_parses_states_and_optional_fields() {
    let tracker = ProgramStatusTracker::default();
    let mut parser = ProgramStatusParser::new(tracker.clone());
    for (state, expected) in [
      ("idle", ProgramState::Idle),
      ("working", ProgramState::Working),
      ("done", ProgramState::Done),
      ("blocked", ProgramState::Blocked),
      ("error", ProgramState::Error),
    ] {
      report(
        &mut parser,
        &format!(
          "state={state}:kind=permission:progress=40:app=terraform:title=VGVzdA:msg=5L2g5aW9"
        ),
      );
      let record = records(&tracker).pop().unwrap();
      assert_eq!(record.state, expected);
      assert_eq!(record.id, "");
      assert_eq!(record.app.as_deref(), Some("terraform"));
      assert_eq!(record.title.as_deref(), Some("Test"));
      assert_eq!(record.msg.as_deref(), Some("你好"));
      assert_eq!(
        record.kind,
        (state == "blocked").then_some(BlockedKind::Permission)
      );
      assert_eq!(
        record.progress,
        matches!(state, "working" | "blocked").then_some(40)
      );
    }
    for (kind, expected) in [
      ("question", Some(BlockedKind::Question)),
      ("auth", Some(BlockedKind::Auth)),
      ("future", None),
    ] {
      report(&mut parser, &format!("state=blocked:kind={kind}"));
      assert_eq!(records(&tracker)[0].kind, expected);
    }
    for progress in ["", "-1", "+1", "1.5", "101", "999999999999999"] {
      report(
        &mut parser,
        &format!("state=working:progress={progress}:app=bad/name"),
      );
      let record = records(&tracker).pop().unwrap();
      assert_eq!(record.progress, None);
      assert_eq!(record.app, None);
    }
    for progress in ["0", "100"] {
      report(&mut parser, &format!("state=working:progress={progress}"));
      assert_eq!(records(&tracker)[0].progress, progress.parse().ok());
    }
  }

  #[test]
  fn program_status_skips_bad_pairs_and_replaces_complete_records() {
    let tracker = ProgramStatusTracker::default();
    let mut parser = ProgramStatusParser::new(tracker.clone());
    report(&mut parser, "state=working:app=cargo:msg=dGVzdA==");
    parser.advance(
      b"\x1b]7501; state = idle :unknown=x:missing:=bad:bad@key=x:msg=bad@:bad=\xff:state=done\x07",
      |_| panic!("unexpected reply"),
    );
    let record = records(&tracker).pop().unwrap();
    assert_eq!(record.state, ProgramState::Done);
    assert_eq!(record.app, None);
    assert_eq!(record.msg, None);
    for body in ["app=cargo", "state=unknown", "state="] {
      report(&mut parser, body);
      assert_eq!(records(&tracker), vec![record.clone()]);
    }
  }

  #[test]
  fn program_status_inherits_apps_and_clears_only_addressed_subtrees() {
    let tracker = ProgramStatusTracker::default();
    let mut parser = ProgramStatusParser::new(tracker.clone());
    for body in [
      "state=idle:app=root",
      "state=working:id=build:app=cargo",
      "state=blocked:id=build/test/unit",
      "state=working:id=builder",
    ] {
      report(&mut parser, body);
    }
    assert_eq!(records(&tracker)[2].app.as_deref(), Some("cargo"));
    assert_eq!(records(&tracker)[3].app.as_deref(), Some("root"));
    report(&mut parser, "state=idle:id=build");
    assert_eq!(records(&tracker)[1].app.as_deref(), Some("root"));
    report(&mut parser, "state=clear:id=build");
    assert_eq!(
      records(&tracker)
        .iter()
        .map(|r| r.id.as_str())
        .collect::<Vec<_>>(),
      ["", "builder"]
    );
    report(&mut parser, "state=clear");
    assert!(records(&tracker).is_empty());
  }

  #[test]
  fn program_status_invalid_ids_never_overwrite_root() {
    let tracker = ProgramStatusTracker::default();
    let mut parser = ProgramStatusParser::new(tracker.clone());
    report(&mut parser, "state=done:app=root");
    let original = records(&tracker);
    for id in ["", "/child", "child/", "a//b", "a,b", "a=b"] {
      report(&mut parser, &format!("state=error:id={id}"));
      assert_eq!(records(&tracker), original);
    }
    for id in ["A_1.+-", "a/b/c/d/e/f/g/h"] {
      report(&mut parser, &format!("state=working:id={id}"));
      assert!(records(&tracker).iter().any(|record| record.id == id));
    }
  }

  #[test]
  fn program_status_rejects_invalid_text_atomically_even_if_overwritten() {
    let tracker = ProgramStatusTracker::default();
    let mut parser = ProgramStatusParser::new(tracker.clone());
    report(&mut parser, "state=done:app=original");
    let original = records(&tracker);
    for value in [
      "_", "a", "====", "____", "/w==", "Cg==", "CQ==", "AA==", "fw==", "woA=",
    ] {
      for key in ["msg", "title"] {
        report(
          &mut parser,
          &format!("state=clear:{key}={value}:{key}=b2s="),
        );
        assert_eq!(records(&tracker), original, "{key}={value}");
      }
    }
    for ch in ['\u{001f}', '\u{0085}', '\u{009f}'] {
      report(
        &mut parser,
        &format!("state=error:msg={}", STANDARD.encode(ch.to_string())),
      );
      assert_eq!(records(&tracker), original);
    }
  }

  #[test]
  fn program_status_enforces_all_field_limits_before_applying() {
    let tracker = ProgramStatusTracker::default();
    let mut parser = ProgramStatusParser::new(tracker.clone());
    report(&mut parser, "state=done");
    let original = records(&tracker);
    for field in [
      format!("{}=x", "k".repeat(17)),
      format!("msg={}", "A".repeat(2733)),
      format!("msg={}", STANDARD.encode("a".repeat(2049))),
      format!("title={}", "A".repeat(257)),
      format!("app={}", "a".repeat(33)),
      format!("id={}", "a".repeat(33)),
      format!("id={}", ["a"; 9].join("/")),
      format!("id={}", vec!["a".repeat(32); 4].join("/") + "/a"),
    ] {
      report(&mut parser, &format!("state=clear:{field}"));
      assert_eq!(records(&tracker), original, "{field}");
    }
    report(
      &mut parser,
      &format!(
        "state=working:app={}:id={}:title={}:msg={}",
        "a".repeat(32),
        vec!["a".repeat(31); 4].join("/"),
        STANDARD.encode("a".repeat(192)),
        STANDARD.encode("a".repeat(2048)),
      ),
    );
    assert_eq!(records(&tracker).len(), 2);
  }

  #[test]
  fn program_status_streams_across_every_chunk_boundary_and_terminator() {
    for terminator in ["\x07", "\x1b\\"] {
      let sequence = format!("text\x1b]7501;state=working:msg=dGVzdA=={terminator}tail");
      for split in 0..=sequence.len() {
        let tracker = ProgramStatusTracker::default();
        let mut parser = ProgramStatusParser::new(tracker.clone());
        parser.advance(&sequence.as_bytes()[..split], |_| {
          panic!("unexpected reply")
        });
        parser.advance(&sequence.as_bytes()[split..], |_| {
          panic!("unexpected reply")
        });
        assert_eq!(records(&tracker)[0].msg.as_deref(), Some("test"));
      }
    }
  }

  #[test]
  fn program_status_feature_detection_replies_in_order_without_reflecting_reports() {
    let tracker = ProgramStatusTracker::default();
    let mut parser = ProgramStatusParser::new(tracker.clone());
    let mut replies = Vec::new();
    for byte in b"\x1b]7501;?\x07\x1b]7501;state=done:msg=dGVzdA==\x1b\\\x1b]7501;?\x1b\\" {
      parser.advance(&[*byte], |reply| replies.push(reply.to_vec()));
    }
    assert_eq!(replies, [FEATURE_REPLY, FEATURE_REPLY]);
    assert_eq!(records(&tracker)[0].state, ProgramState::Done);
  }

  #[test]
  fn program_status_sequence_limit_counts_framing_and_recovers() {
    for terminator in ["\x07", "\x1b\\"] {
      for length in [4096, 4097] {
        let tracker = ProgramStatusTracker::default();
        let mut parser = ProgramStatusParser::new(tracker.clone());
        let prefix = "\x1b]7501;state=done:unknown=";
        let sequence = format!(
          "{prefix}{}{terminator}",
          "a".repeat(length - prefix.len() - terminator.len())
        );
        for chunk in sequence.as_bytes().chunks(17) {
          parser.advance(chunk, |_| panic!("unexpected reply"));
        }
        assert_eq!(records(&tracker).len(), usize::from(length == 4096));
        assert!(parser.body.is_empty());
        report(&mut parser, "state=idle");
        assert_eq!(records(&tracker)[0].state, ProgramState::Idle);
      }
    }
  }

  #[test]
  fn program_status_lifetimes_follow_prompt_exit_and_reset() {
    let tracker = ProgramStatusTracker::default();
    let mut parser = ProgramStatusParser::new(tracker.clone());
    for id in ["idle", "working", "done", "blocked", "error"] {
      report(&mut parser, &format!("state={id}:id={id}"));
    }
    parser.advance(b"\x1b[?1049h\x1b[!p\x1b[?1049l", |_| {});
    assert_eq!(records(&tracker).len(), 5);
    parser.advance(b"\x1b]133;A;cl=m\x1b\\", |_| {});
    assert_eq!(
      records(&tracker)
        .iter()
        .map(|r| r.id.as_str())
        .collect::<Vec<_>>(),
      ["idle", "done", "error"]
    );
    report(&mut parser, "state=working:id=worker");
    tracker.expire_running();
    assert_eq!(records(&tracker).len(), 3);
    parser.advance(b"\x1b", |_| {});
    parser.advance(b"c", |_| {});
    assert!(records(&tracker).is_empty());
  }

  #[test]
  fn program_status_ignores_other_control_strings_and_canceled_reports() {
    let tracker = ProgramStatusTracker::default();
    let mut parser = ProgramStatusParser::new(tracker.clone());
    for opener in ["\x1bP", "\x1b_", "\x1b^", "\x1bX"] {
      parser.advance(
        format!("{opener}\x1b]7501;state=error\x07\x1b\\").as_bytes(),
        |_| {},
      );
      assert!(records(&tracker).is_empty());
    }
    parser.advance(b"\x1b]2;7501;state=error\x07", |_| {});
    assert!(records(&tracker).is_empty());
    for cancel in [0x18, 0x1a] {
      parser.advance(b"\x1b]7501;state=error", |_| {});
      parser.advance(&[cancel, 0x07], |_| {});
      assert!(records(&tracker).is_empty());
    }
    report(&mut parser, "state=done");
    assert_eq!(records(&tracker).len(), 1);
  }

  #[test]
  fn program_status_evicts_least_recent_update_and_retains_recent_replacement() {
    let tracker = ProgramStatusTracker::default();
    let mut parser = ProgramStatusParser::new(tracker.clone());
    for id in 0..MAX_RECORDS {
      report(&mut parser, &format!("state=idle:id=task{id}"));
    }
    report(&mut parser, "state=done:id=task0");
    report(&mut parser, "state=working:id=new");
    let snapshot = records(&tracker);
    assert_eq!(snapshot.len(), MAX_RECORDS);
    assert!(!snapshot.iter().any(|r| r.id == "task1"));
    assert!(
      snapshot
        .iter()
        .any(|r| r.id == "task0" && r.state == ProgramState::Done)
    );
  }

  #[test]
  fn program_status_display_disarms_invisible_formatting_and_preserves_literal_text() {
    assert_eq!(
      display_text("<b>你好</b>\u{202e}test\u{200b}\u{2066}"),
      "<b>你好</b>test"
    );
  }
}
