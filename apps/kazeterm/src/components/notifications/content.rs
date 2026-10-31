use super::NotificationReason;

#[derive(Debug, PartialEq, Eq)]
pub(super) struct NotificationContent {
  pub(super) title: String,
  pub(super) body: String,
}

impl NotificationContent {
  fn program_status(
    tab: Option<&str>,
    current: &str,
    record: &terminal::program_status::ProgramStatusRecord,
  ) -> Self {
    use terminal::program_status::{BlockedKind, ProgramState, display_text};
    let label = match (record.state, record.kind) {
      (ProgramState::Done, _) => "Task completed",
      (ProgramState::Error, _) => "Task failed",
      (ProgramState::Blocked, Some(BlockedKind::Permission)) => "Permission required",
      (ProgramState::Blocked, Some(BlockedKind::Question)) => "Input required",
      (ProgramState::Blocked, Some(BlockedKind::Auth)) => "Authentication required",
      (ProgramState::Blocked, None) => "Task blocked",
      _ => "Program status",
    };
    let app = record.app.as_deref().map(display_text).unwrap_or_default();
    let task = record
      .title
      .as_deref()
      .map(display_text)
      .filter(|text| !text.trim().is_empty())
      .unwrap_or_else(|| {
        if record.id.is_empty() {
          display_text(current)
        } else {
          record.id.clone()
        }
      });
    let subject = if !app.is_empty() && !task.trim().is_empty() && app != task {
      format!("{app} — {}", task.trim())
    } else if !app.is_empty() {
      app
    } else {
      task.trim().to_string()
    };
    let mut body = if subject.is_empty() {
      label.to_string()
    } else {
      format!("{label}: {subject}")
    };
    if let Some(msg) = record
      .msg
      .as_deref()
      .map(display_text)
      .filter(|msg| !msg.trim().is_empty())
    {
      body.push('\n');
      body.push_str(msg.trim());
    }
    if let Some(tab) = tab
      .map(display_text)
      .filter(|tab| !tab.trim().is_empty() && tab.trim() != subject)
    {
      body = format!("{}\n{body}", tab.trim());
    }
    Self {
      title: format!("Kazeterm — {label}"),
      body,
    }
  }

  #[cfg(any(target_os = "windows", target_os = "linux", test))]
  fn escape_markup(text: &str) -> String {
    text
      .replace('&', "&amp;")
      .replace('<', "&lt;")
      .replace('>', "&gt;")
      .replace('"', "&quot;")
  }

  #[cfg(any(target_os = "linux", test))]
  pub(super) fn linux_body(&self) -> String {
    Self::escape_markup(&self.body)
  }

  #[cfg(any(target_os = "windows", test))]
  pub(super) fn windows_xml(&self) -> String {
    let title = Self::escape_markup(&self.title);
    let body = Self::escape_markup(&self.body);
    format!(
      r#"<toast><visual><binding template="ToastGeneric"><text>{title}</text><text>{body}</text></binding></visual></toast>"#
    )
  }

  #[cfg(any(target_os = "macos", test))]
  pub(super) fn macos_script(&self) -> String {
    let escape = |text: &str| {
      text
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
    };
    let title = escape(&self.title);
    let body = escape(&self.body);
    format!(r#"display notification "{body}" with title "{title}""#)
  }
}

impl NotificationReason {
  pub(super) fn content(
    self,
    tab_title: Option<&str>,
    current_title: &str,
    previous_title: &str,
  ) -> NotificationContent {
    let (label, subject, fallback) = match self {
      Self::CommandFinished => (
        "Command finished",
        if previous_title.trim().is_empty() {
          current_title
        } else {
          previous_title
        },
        "The shell prompt returned.",
      ),
      Self::ProgramStatus(record) => {
        return NotificationContent::program_status(tab_title, current_title, &record);
      }
      Self::Bell => (
        "Terminal bell",
        current_title,
        "The terminal rang its bell.",
      ),
    };
    let subject = subject.trim();
    let detail = if subject.is_empty() {
      fallback.to_string()
    } else {
      format!("{label}: {subject}")
    };
    let body = match tab_title
      .map(str::trim)
      .filter(|tab| !tab.is_empty() && *tab != subject)
    {
      Some(tab) => format!("{tab}\n{detail}"),
      None => detail,
    };
    NotificationContent {
      title: format!("Kazeterm — {label}"),
      body,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn notification_content_distinguishes_command_completion_from_bell() {
    let finished = NotificationReason::CommandFinished.content(Some("Build"), "pwsh", "cargo test");
    assert_eq!(finished.title, "Kazeterm — Command finished");
    assert_eq!(finished.body, "Build\nCommand finished: cargo test");
    let bell = NotificationReason::Bell.content(Some("Assistant"), "copilot", "cargo test");
    assert_eq!(bell.title, "Kazeterm — Terminal bell");
    assert_eq!(bell.body, "Assistant\nTerminal bell: copilot");
    assert_ne!(finished.title, bell.title);
    assert!(!bell.body.contains("cargo test"));
  }

  #[test]
  fn osc_content_identifies_status_application_task_and_message() {
    use terminal::program_status::{BlockedKind, ProgramState, ProgramStatusRecord};
    let mut record = ProgramStatusRecord {
      id: "build/test".into(),
      state: ProgramState::Done,
      state_revision: 1,
      kind: None,
      progress: None,
      app: Some("cargo".into()),
      title: Some("Tests\u{202e}".into()),
      msg: Some("<b>All passed</b>\u{200b}".into()),
    };
    for (state, kind, label) in [
      (ProgramState::Done, None, "Task completed"),
      (ProgramState::Error, None, "Task failed"),
      (
        ProgramState::Blocked,
        Some(BlockedKind::Permission),
        "Permission required",
      ),
      (
        ProgramState::Blocked,
        Some(BlockedKind::Question),
        "Input required",
      ),
      (
        ProgramState::Blocked,
        Some(BlockedKind::Auth),
        "Authentication required",
      ),
      (ProgramState::Blocked, None, "Task blocked"),
    ] {
      record.state = state;
      record.kind = kind;
      let content = NotificationReason::ProgramStatus(record.clone()).content(
        Some("Build"),
        "pwsh",
        "old command",
      );
      assert_eq!(content.title, format!("Kazeterm — {label}"));
      assert_eq!(
        content.body,
        format!("Build\n{label}: cargo — Tests\n<b>All passed</b>")
      );
    }
  }

  #[test]
  fn native_payloads_preserve_titles_and_escape_notification_text() {
    let content = NotificationContent {
      title: "Kazeterm — \"Bell\"".into(),
      body: "build & <test> \"quoted\" C:\\work\nnext\r\tend".into(),
    };
    assert!(
      content
        .linux_body()
        .contains("build &amp; &lt;test&gt; &quot;quoted&quot;")
    );
    let xml = content.windows_xml();
    assert!(xml.contains("<text>Kazeterm — &quot;Bell&quot;</text>"));
    assert!(xml.contains("build &amp; &lt;test&gt; &quot;quoted&quot; C:\\work"));
    let script = content.macos_script();
    assert!(script.contains(r#"with title "Kazeterm — \"Bell\"""#));
    assert!(script.contains("\\\"quoted\\\""));
    assert!(script.contains("C:\\\\work"));
    assert!(script.contains("\\nnext\\r\\tend"));
    assert!(!script.contains(['\n', '\r', '\t']));
  }

  #[test]
  fn notification_content_uses_source_specific_fallbacks_and_avoids_duplicate_context() {
    for (reason, tab, current, previous, body) in [
      (
        NotificationReason::CommandFinished,
        None,
        "pwsh",
        "  ",
        "Command finished: pwsh",
      ),
      (
        NotificationReason::CommandFinished,
        None,
        "",
        "",
        "The shell prompt returned.",
      ),
      (
        NotificationReason::Bell,
        None,
        "",
        "old command",
        "The terminal rang its bell.",
      ),
      (
        NotificationReason::Bell,
        Some("  copilot  "),
        " copilot ",
        "old",
        "Terminal bell: copilot",
      ),
      (
        NotificationReason::CommandFinished,
        Some(" cargo test "),
        "pwsh",
        " cargo test ",
        "Command finished: cargo test",
      ),
      (
        NotificationReason::Bell,
        Some("  "),
        "copilot",
        "",
        "Terminal bell: copilot",
      ),
      (
        NotificationReason::Bell,
        Some("Assistant"),
        "",
        "old",
        "Assistant\nThe terminal rang its bell.",
      ),
    ] {
      assert_eq!(reason.content(tab, current, previous).body, body);
    }
  }
}
