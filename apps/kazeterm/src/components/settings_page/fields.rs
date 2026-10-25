#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Section {
  Startup,
  Appearance,
  Font,
  Tabs,
  Panes,
  Terminal,
  Cursor,
  Notifications,
  Animation,
  Updates,
  Profiles,
  Keybindings,
  Environment,
  Imports,
}

impl Section {
  pub(super) const ALL: [Self; 14] = [
    Self::Startup,
    Self::Appearance,
    Self::Font,
    Self::Tabs,
    Self::Panes,
    Self::Terminal,
    Self::Cursor,
    Self::Notifications,
    Self::Animation,
    Self::Updates,
    Self::Profiles,
    Self::Keybindings,
    Self::Environment,
    Self::Imports,
  ];

  pub(super) fn label(self) -> &'static str {
    match self {
      Self::Startup => "Startup",
      Self::Appearance => "Appearance",
      Self::Font => "Fonts",
      Self::Tabs => "Tabs",
      Self::Panes => "Panes",
      Self::Terminal => "Terminal",
      Self::Cursor => "Cursor",
      Self::Notifications => "Notifications",
      Self::Animation => "Animation",
      Self::Updates => "Updates",
      Self::Profiles => "Profiles",
      Self::Keybindings => "Keybindings",
      Self::Environment => "Environment",
      Self::Imports => "Imports",
    }
  }
}

#[derive(Clone, Copy)]
pub(super) enum FieldKind {
  Bool,
  Text { optional: bool },
  Number { min: f64, max: f64, integer: bool },
  Choice(&'static [&'static str]),
  Theme,
  DefaultProfile,
}

pub(super) struct FieldSpec {
  pub(super) section: Section,
  pub(super) table: &'static str,
  pub(super) key: &'static str,
  pub(super) label: &'static str,
  pub(super) description: &'static str,
  pub(super) kind: FieldKind,
}

#[cfg(target_os = "linux")]
const KERNEL_CHOICES: &[&str] = &["alacritty", "vte"];
#[cfg(not(target_os = "linux"))]
const KERNEL_CHOICES: &[&str] = &["alacritty"];

pub(super) const FIELDS: &[FieldSpec] = &[
  FieldSpec {
    section: Section::Startup,
    table: "window",
    key: "width",
    label: "Window width",
    description: "Initial width in pixels for new windows without restored bounds. Does not resize existing windows.",
    kind: FieldKind::Number {
      min: 1.0,
      max: f32::MAX as f64,
      integer: false,
    },
  },
  FieldSpec {
    section: Section::Startup,
    table: "window",
    key: "height",
    label: "Window height",
    description: "Initial height in pixels for new windows without restored bounds. Does not resize existing windows.",
    kind: FieldKind::Number {
      min: 1.0,
      max: f32::MAX as f64,
      integer: false,
    },
  },
  FieldSpec {
    section: Section::Startup,
    table: "window",
    key: "start_maximized",
    label: "Start maximized",
    description: "Maximize newly opened windows when restored window state does not take priority.",
    kind: FieldKind::Bool,
  },
  FieldSpec {
    section: Section::Startup,
    table: "window",
    key: "restore_workspace",
    label: "Restore workspace",
    description: "Restore saved tabs, splits, and working directories on the next application launch.",
    kind: FieldKind::Bool,
  },
  FieldSpec {
    section: Section::Appearance,
    table: "colors",
    key: "theme",
    label: "Theme",
    description: "Theme name from the available themes or your custom themes directory.",
    kind: FieldKind::Theme,
  },
  FieldSpec {
    section: Section::Appearance,
    table: "colors",
    key: "theme_mode",
    label: "Theme mode",
    description: "Use a dark or light theme variant, or follow the operating system.",
    kind: FieldKind::Choice(&["dark", "light", "system"]),
  },
  FieldSpec {
    section: Section::Appearance,
    table: "colors",
    key: "bold_as_bright",
    label: "Bold text uses bright colors",
    description: "Use bright ANSI colors for bold terminal text in addition to its font weight.",
    kind: FieldKind::Bool,
  },
  FieldSpec {
    section: Section::Appearance,
    table: "colors",
    key: "minimum_contrast",
    label: "Minimum text contrast",
    description: "Minimum absolute APCA contrast for terminal text. Set to 0 to disable contrast adjustment.",
    kind: FieldKind::Number {
      min: 0.0,
      max: f32::MAX as f64,
      integer: false,
    },
  },
  FieldSpec {
    section: Section::Appearance,
    table: "appearance",
    key: "themes_path",
    label: "Custom themes directory",
    description: "Directory containing theme TOML files, preferred over built-in themes. Leave blank to use built-in themes.",
    kind: FieldKind::Text { optional: true },
  },
  FieldSpec {
    section: Section::Appearance,
    table: "appearance",
    key: "background_opacity",
    label: "Background opacity",
    description: "Configured background opacity from 0 (transparent) to 1. Platform-specific transparency may affect the result.",
    kind: FieldKind::Number {
      min: 0.0,
      max: 1.0,
      integer: false,
    },
  },
  FieldSpec {
    section: Section::Appearance,
    table: "appearance",
    key: "background_blur",
    label: "Blur behind window",
    description: "Blur the desktop behind transparent windows when supported by the platform.",
    kind: FieldKind::Bool,
  },
  FieldSpec {
    section: Section::Appearance,
    table: "window",
    key: "key_debug_mode",
    label: "Keybinding debug overlay",
    description: "Show actions matching the currently held modifier keys in a window overlay.",
    kind: FieldKind::Bool,
  },
  FieldSpec {
    section: Section::Font,
    table: "font",
    key: "family",
    label: "Terminal font family",
    description: "Font family used to render terminal text.",
    kind: FieldKind::Text { optional: false },
  },
  FieldSpec {
    section: Section::Font,
    table: "font",
    key: "size",
    label: "Terminal font size",
    description: "Base terminal font size in pixels, before zoom is applied.",
    kind: FieldKind::Number {
      min: 1.0,
      max: f32::MAX as f64,
      integer: false,
    },
  },
  FieldSpec {
    section: Section::Font,
    table: "font",
    key: "ui_family",
    label: "Interface font family",
    description: "Font family used for tabs, menus, dialogs, and other interface text.",
    kind: FieldKind::Text { optional: false },
  },
  FieldSpec {
    section: Section::Font,
    table: "font",
    key: "ui_size",
    label: "Interface font size",
    description: "Base interface font size in pixels.",
    kind: FieldKind::Number {
      min: 1.0,
      max: f32::MAX as f64,
      integer: false,
    },
  },
  FieldSpec {
    section: Section::Tabs,
    table: "tab",
    key: "vertical",
    label: "Vertical tabs",
    description: "Place tabs in a left sidebar instead of across the top.",
    kind: FieldKind::Bool,
  },
  FieldSpec {
    section: Section::Tabs,
    table: "tab",
    key: "close_on_last",
    label: "Close on last tab",
    description: "Close the window when its last tab closes; otherwise create a new tab.",
    kind: FieldKind::Bool,
  },
  FieldSpec {
    section: Section::Tabs,
    table: "tab",
    key: "switcher_popup",
    label: "Tab switcher popup",
    description: "Show a tab switcher while cycling tabs with keyboard shortcuts.",
    kind: FieldKind::Bool,
  },
  FieldSpec {
    section: Section::Tabs,
    table: "tab",
    key: "title_change_delay_ms",
    label: "Title change delay (ms)",
    description: "Delay terminal-driven tab title updates to reduce rapid title changes. Set to 0 for immediate updates.",
    kind: FieldKind::Number {
      min: 0.0,
      max: 5_000.0,
      integer: true,
    },
  },
  FieldSpec {
    section: Section::Tabs,
    table: "tab",
    key: "label_min_width",
    label: "Minimum tab width (px)",
    description: "Minimum tab item width when minimum characters is 0. Reversed minimum and maximum widths are normalized.",
    kind: FieldKind::Number {
      min: 24.0,
      max: 480.0,
      integer: false,
    },
  },
  FieldSpec {
    section: Section::Tabs,
    table: "tab",
    key: "label_max_width",
    label: "Maximum tab width (px)",
    description: "Maximum tab item width when maximum characters is 0. Effective tab widths are limited to 24-480 pixels.",
    kind: FieldKind::Number {
      min: 24.0,
      max: 480.0,
      integer: false,
    },
  },
  FieldSpec {
    section: Section::Tabs,
    table: "tab",
    key: "label_min_chars",
    label: "Minimum tab width (characters)",
    description: "Override minimum pixel width using the interface font size. Set to 0 for pixels; the result is limited to 24-480 pixels.",
    kind: FieldKind::Number {
      min: 0.0,
      max: u32::MAX as f64,
      integer: true,
    },
  },
  FieldSpec {
    section: Section::Tabs,
    table: "tab",
    key: "label_max_chars",
    label: "Maximum tab width (characters)",
    description: "Override maximum pixel width using the interface font size. Set to 0 for pixels; the result is limited to 24-480 pixels.",
    kind: FieldKind::Number {
      min: 0.0,
      max: u32::MAX as f64,
      integer: true,
    },
  },
  FieldSpec {
    section: Section::Panes,
    table: "pane",
    key: "divider_width",
    label: "Divider width",
    description: "Width of split pane divider drag handles in pixels.",
    kind: FieldKind::Number {
      min: 1.0,
      max: 32.0,
      integer: false,
    },
  },
  FieldSpec {
    section: Section::Panes,
    table: "pane",
    key: "inactive_opacity",
    label: "Inactive pane opacity",
    description: "Opacity of unfocused panes, from 0 (transparent) to 1 (no dimming).",
    kind: FieldKind::Number {
      min: 0.0,
      max: 1.0,
      integer: false,
    },
  },
  FieldSpec {
    section: Section::Terminal,
    table: "terminal",
    key: "kernel",
    label: "Terminal backend",
    description: "Backend for new terminals; existing sessions keep their backend. VTE is available only on Linux.",
    kind: FieldKind::Choice(KERNEL_CHOICES),
  },
  FieldSpec {
    section: Section::Terminal,
    table: "terminal",
    key: "scrollback_lines",
    label: "Scrollback lines",
    description: "Maximum scrollback history for new terminals. Higher values use more memory; 0 disables scrollback.",
    kind: FieldKind::Number {
      min: 0.0,
      max: 100_000.0,
      integer: true,
    },
  },
  FieldSpec {
    section: Section::Terminal,
    table: "terminal",
    key: "osc52",
    label: "OSC 52 clipboard access",
    description: "Clipboard access granted to terminal applications through OSC 52. Applies to new terminals; paste modes allow reading the clipboard.",
    kind: FieldKind::Choice(&["disabled", "copy_only", "paste_only", "copy_paste"]),
  },
  FieldSpec {
    section: Section::Terminal,
    table: "terminal",
    key: "copy_on_select",
    label: "Copy on selection",
    description: "Copy selected terminal text to the clipboard when the mouse button is released.",
    kind: FieldKind::Bool,
  },
  FieldSpec {
    section: Section::Terminal,
    table: "terminal",
    key: "hide_mouse_when_typing",
    label: "Hide mouse while typing",
    description: "Hide the mouse pointer during terminal keyboard input until it moves again.",
    kind: FieldKind::Bool,
  },
  FieldSpec {
    section: Section::Terminal,
    table: "terminal",
    key: "focus_terminal_on_hover",
    label: "Focus terminal on hover",
    description: "Focus a terminal when the mouse pointer enters it.",
    kind: FieldKind::Bool,
  },
  FieldSpec {
    section: Section::Terminal,
    table: "terminal",
    key: "right_click_context_menu",
    label: "Right-click context menu",
    description: "Show a context menu on right-click instead of the default copy/paste behavior.",
    kind: FieldKind::Bool,
  },
  FieldSpec {
    section: Section::Terminal,
    table: "terminal",
    key: "ctrl_scroll_zoom",
    label: "Ctrl+Scroll zoom",
    description: "Change the terminal font zoom while scrolling with Ctrl held.",
    kind: FieldKind::Bool,
  },
  FieldSpec {
    section: Section::Terminal,
    table: "terminal",
    key: "minimap_enabled",
    label: "Terminal minimap",
    description: "Show a compact preview of terminal scrollback beside the terminal.",
    kind: FieldKind::Bool,
  },
  FieldSpec {
    section: Section::Startup,
    table: "terminal",
    key: "working_directory",
    label: "Default working directory",
    description: "Fallback working directory for new terminals. Profile-specific or explicitly supplied directories take priority; leave blank for the normal fallback.",
    kind: FieldKind::Text { optional: true },
  },
  FieldSpec {
    section: Section::Startup,
    table: "terminal",
    key: "default_profile",
    label: "Default profile",
    description: "Profile to use for new terminals. Leave blank for automatic profile selection.",
    kind: FieldKind::DefaultProfile,
  },
  FieldSpec {
    section: Section::Cursor,
    table: "cursor",
    key: "shape",
    label: "Cursor shape",
    description: "Default cursor shape for new terminals. Terminal applications may override the shape.",
    kind: FieldKind::Choice(&["block", "underline", "beam"]),
  },
  FieldSpec {
    section: Section::Cursor,
    table: "cursor",
    key: "blink",
    label: "Blink cursor",
    description: "Allow cursor blinking. New terminals receive this default; existing terminal application cursor modes may still disable blinking.",
    kind: FieldKind::Bool,
  },
  FieldSpec {
    section: Section::Cursor,
    table: "cursor",
    key: "blink_interval",
    label: "Blink interval (ms)",
    description: "Time between cursor visibility changes while blinking is enabled.",
    kind: FieldKind::Number {
      min: 10.0,
      max: 10_000.0,
      integer: true,
    },
  },
  FieldSpec {
    section: Section::Notifications,
    table: "notification",
    key: "long_running_threshold_secs",
    label: "Idle threshold (seconds)",
    description: "Minimum time since terminal input before a prompt return or bell can notify. Set to 0 to remove the idle requirement.",
    kind: FieldKind::Number {
      min: 0.0,
      max: i64::MAX as f64,
      integer: true,
    },
  },
  FieldSpec {
    section: Section::Notifications,
    table: "notification",
    key: "interval_secs",
    label: "Notification interval (seconds)",
    description: "Minimum time between desktop notifications. Set to 0 to allow every eligible notification.",
    kind: FieldKind::Number {
      min: 0.0,
      max: i64::MAX as f64,
      integer: true,
    },
  },
  FieldSpec {
    section: Section::Animation,
    table: "animation",
    key: "enabled",
    label: "Enable animations",
    description: "Animate interface transitions instead of applying their visual changes immediately.",
    kind: FieldKind::Bool,
  },
  FieldSpec {
    section: Section::Animation,
    table: "animation",
    key: "duration_ms",
    label: "Transition duration (ms)",
    description: "Total transition duration. Set to 0 to disable animations.",
    kind: FieldKind::Number {
      min: 0.0,
      max: 5_000.0,
      integer: true,
    },
  },
  FieldSpec {
    section: Section::Animation,
    table: "animation",
    key: "frame_interval_ms",
    label: "Frame interval (ms)",
    description: "Requested delay between animation frames; actual timing is adjusted to fit the transition duration.",
    kind: FieldKind::Number {
      min: 4.0,
      max: 1_000.0,
      integer: true,
    },
  },
  FieldSpec {
    section: Section::Animation,
    table: "animation",
    key: "easing",
    label: "Easing",
    description: "Curve used to interpolate transition geometry.",
    kind: FieldKind::Choice(&["linear", "ease_in", "ease_out", "ease_in_out"]),
  },
  FieldSpec {
    section: Section::Updates,
    table: "auto_update",
    key: "check",
    label: "Automatic update policy",
    description: "Check on every launch, never automatically, or on launch at most once a day. Restart to apply a changed automatic-check policy.",
    kind: FieldKind::Choice(&["always", "never", "once_a_day"]),
  },
  FieldSpec {
    section: Section::Updates,
    table: "auto_update",
    key: "proxy",
    label: "Update proxy",
    description: "Optional HTTP(S) proxy URL for future update checks only. Leave blank to use the updater's default networking.",
    kind: FieldKind::Text { optional: true },
  },
];

impl FieldSpec {
  pub(super) fn accepts_input(&self, text: &str) -> bool {
    let FieldKind::Number { min, integer, .. } = self.kind else {
      return true;
    };
    let text = if min < 0.0 {
      text.strip_prefix('-').unwrap_or(text)
    } else {
      text
    };
    // Keep empty and partial decimal drafts editable; parse checks completed values and ranges.
    let mut has_decimal_point = false;
    text.bytes().all(|byte| {
      if byte.is_ascii_digit() {
        true
      } else if byte == b'.' && !integer && !has_decimal_point {
        has_decimal_point = true;
        true
      } else {
        false
      }
    })
  }

  pub(super) fn value(&self, config: &toml::Value) -> String {
    match config.get(self.table).and_then(|table| table.get(self.key)) {
      Some(toml::Value::String(value)) => value.clone(),
      Some(toml::Value::Boolean(value)) => value.to_string(),
      Some(toml::Value::Integer(value)) => value.to_string(),
      // Fractional Config fields are f32; avoid exposing f64 serialization noise.
      Some(toml::Value::Float(value)) => {
        let text = (*value as f32).to_string();
        if self.parse(&text).is_ok() {
          text
        } else {
          value.to_string()
        }
      }
      _ => String::new(),
    }
  }

  pub(super) fn parse(&self, text: &str) -> Result<Option<toml::Value>, String> {
    let text = text.trim();
    let invalid = |message: &str| format!("{}: {message}", self.label);
    let value = match self.kind {
      FieldKind::Bool => toml::Value::Boolean(
        text
          .parse::<bool>()
          .map_err(|_| invalid("choose true or false."))?,
      ),
      FieldKind::Text { optional: true } | FieldKind::DefaultProfile if text.is_empty() => {
        return Ok(None);
      }
      FieldKind::Text { .. } | FieldKind::Theme | FieldKind::DefaultProfile => {
        if text.is_empty() {
          return Err(invalid("enter a value."));
        }
        toml::Value::String(text.to_string())
      }
      FieldKind::Choice(choices) => {
        if !choices.contains(&text) {
          return Err(invalid(&format!("choose one of {}.", choices.join(", "))));
        }
        toml::Value::String(text.to_string())
      }
      FieldKind::Number { min, max, integer } => {
        let (number, value) = if integer {
          // Parse integers directly so values above 2^53 are not rounded through f64.
          let number = text
            .parse::<i64>()
            .map_err(|_| invalid("enter a whole number within the TOML integer range."))?;
          (number as f64, toml::Value::Integer(number))
        } else {
          let number = text
            .parse::<f64>()
            .map_err(|_| invalid("enter a finite number."))?;
          (number, toml::Value::Float(number))
        };
        if !number.is_finite() {
          return Err(invalid("enter a finite number."));
        }
        if !(min..=max).contains(&number) {
          return Err(invalid(&format!("enter a value from {min} to {max}.")));
        }
        value
      }
    };
    Ok(Some(value))
  }

  pub(super) fn write(&self, config: &mut toml::Value, text: &str) -> Result<(), String> {
    let value = self.parse(text)?;
    let table = config
      .get_mut(self.table)
      .and_then(toml::Value::as_table_mut)
      .ok_or_else(|| format!("{}: missing or invalid [{}] table.", self.label, self.table))?;
    match value {
      Some(value) => {
        table.insert(self.key.to_string(), value);
      }
      None => {
        table.remove(self.key);
      }
    }
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use std::collections::BTreeSet;

  use super::*;

  fn field(table: &str, key: &str) -> &'static FieldSpec {
    FIELDS
      .iter()
      .find(|field| field.table == table && field.key == key)
      .unwrap()
  }

  fn config_with_optional_values() -> ::config::Config {
    // Construct sections independently to avoid Config::default's process discovery.
    let mut config = ::config::Config {
      version: ::config::CURRENT_CONFIG_VERSION.to_string(),
      imports: vec!["extra.toml".to_string()],
      colors: toml::from_str("").unwrap(),
      appearance: toml::from_str("").unwrap(),
      animation: toml::from_str("").unwrap(),
      font: toml::from_str("").unwrap(),
      window: toml::from_str("").unwrap(),
      tab: toml::from_str("").unwrap(),
      pane: toml::from_str("").unwrap(),
      terminal: toml::from_str("").unwrap(),
      cursor: toml::from_str("").unwrap(),
      notification: toml::from_str("").unwrap(),
      auto_update: toml::from_str("").unwrap(),
      profiles: Vec::new(),
      keybindings: toml::from_str("").unwrap(),
      container_profiles: Vec::new(),
    };
    config.appearance.themes_path = Some("custom themes".to_string());
    config.terminal.working_directory = Some("workspace".to_string());
    config.terminal.default_profile = Some("Example shell".to_string());
    config.terminal.env.insert("EXAMPLE".into(), "value".into());
    config.auto_update.proxy = Some("http://localhost:8080".to_string());
    config.auto_update.last_check_unix_secs = Some(123);
    config.auto_update.restore_workspace_once = true;
    config
  }

  #[test]
  fn numeric_input_accepts_only_editable_number_syntax() {
    for spec in FIELDS {
      let FieldKind::Number { integer, .. } = spec.kind else {
        assert!(spec.accepts_input("Text with spaces and symbols: #"));
        continue;
      };
      for text in ["", "0", "12", "0012", "999999999999999999999999"] {
        assert!(spec.accepts_input(text), "{} rejected {text:?}", spec.key);
      }
      for text in [
        "a",
        "12px",
        "1e3",
        "NaN",
        "inf",
        " 12",
        "12 ",
        "+1",
        "-1",
        "1,5",
        "1.2.3",
        "\u{ff11}\u{ff12}",
        "\u{4e2d}\u{6587}",
      ] {
        assert!(!spec.accepts_input(text), "{} accepted {text:?}", spec.key);
      }
      for text in [".", ".5", "12.", "12.5"] {
        assert_eq!(spec.accepts_input(text), !integer, "{}: {text:?}", spec.key);
      }
    }
  }

  #[test]
  fn numeric_fields_reject_nonfinite_and_invalid_numbers() {
    for field in FIELDS {
      if matches!(field.kind, FieldKind::Number { .. }) {
        for text in [
          "NaN", "nan", "inf", "-inf", "+inf", "1e999", "", "text", "-1",
        ] {
          assert!(field.parse(text).is_err(), "{} accepted {text}", field.key);
        }
      }
    }
    for (table, key) in [
      ("window", "width"),
      ("window", "height"),
      ("font", "size"),
      ("font", "ui_size"),
    ] {
      let field = field(table, key);
      assert!(field.parse("0").is_err());
      assert!(field.parse("3.5e38").is_err());
      assert_eq!(
        field.parse(" 18.5 ").unwrap(),
        Some(toml::Value::Float(18.5)),
      );
    }
    let mut config = config_with_optional_values();
    config.font.size = f32::MAX;
    let value = toml::Value::try_from(config).unwrap();
    let size = field("font", "size");
    assert_eq!(
      size.parse(&size.value(&value)).unwrap(),
      Some(toml::Value::Float(f32::MAX as f64)),
    );
  }

  #[test]
  fn numeric_bounds_match_config_limits_and_integer_types() {
    for (table, key, min, max) in [
      ("appearance", "background_opacity", 0.0, 1.0),
      ("colors", "minimum_contrast", 0.0, f32::MAX as f64),
      ("animation", "duration_ms", 0.0, 5_000.0),
      ("animation", "frame_interval_ms", 4.0, 1_000.0),
      ("tab", "title_change_delay_ms", 0.0, 5_000.0),
      ("tab", "label_min_width", 24.0, 480.0),
      ("tab", "label_max_width", 24.0, 480.0),
      ("tab", "label_min_chars", 0.0, u32::MAX as f64),
      ("tab", "label_max_chars", 0.0, u32::MAX as f64),
      ("pane", "divider_width", 1.0, 32.0),
      ("pane", "inactive_opacity", 0.0, 1.0),
      ("terminal", "scrollback_lines", 0.0, 100_000.0),
      ("cursor", "blink_interval", 10.0, 10_000.0),
    ] {
      let field = field(table, key);
      assert!(field.parse(&min.to_string()).is_ok(), "{table}.{key}");
      assert!(field.parse(&max.to_string()).is_ok(), "{table}.{key}");
      assert!(field.parse(&(min - 1.0).to_string()).is_err());
      assert!(field.parse(&(max * 2.0 + 1.0).to_string()).is_err());
      if matches!(field.kind, FieldKind::Number { integer: true, .. }) {
        assert!(field.parse(&format!("{min}.5")).is_err());
      }
    }
    let seconds = field("notification", "interval_secs");
    for number in [0, 9_007_199_254_740_993, i64::MAX] {
      assert_eq!(
        seconds.parse(&number.to_string()).unwrap(),
        Some(toml::Value::Integer(number)),
      );
    }
    assert!(seconds.parse("9223372036854775808").is_err());
    assert!(seconds.parse("1.5").is_err());
  }

  #[test]
  fn text_fields_trim_edges_and_allow_only_optional_blanks() {
    for field in FIELDS {
      match field.kind {
        FieldKind::Text { optional: true } | FieldKind::DefaultProfile => {
          assert_eq!(field.parse(" \t\n ").unwrap(), None);
        }
        FieldKind::Text { optional: false } | FieldKind::Theme => {
          assert!(field.parse(" \t\n ").is_err());
        }
        _ => continue,
      }
      assert_eq!(
        field.parse("  Custom  name  ").unwrap(),
        Some(toml::Value::String("Custom  name".to_string())),
      );
    }
    assert_eq!(
      field("terminal", "working_directory")
        .parse(r"  C:\Program Files\Shell  ")
        .unwrap(),
      Some(toml::Value::String(r"C:\Program Files\Shell".to_string())),
    );
  }

  #[test]
  fn booleans_and_choices_validate_without_restricting_dynamic_names() {
    for field in FIELDS {
      match field.kind {
        FieldKind::Bool => {
          for (text, value) in [(" true ", true), ("false", false)] {
            assert_eq!(
              field.parse(text).unwrap(),
              Some(toml::Value::Boolean(value))
            );
          }
          for text in ["", "yes", "1", "TRUE"] {
            assert!(field.parse(text).is_err());
          }
        }
        FieldKind::Choice(choices) => {
          for choice in choices {
            assert_eq!(
              field.parse(&format!(" {choice} ")).unwrap(),
              Some(toml::Value::String((*choice).to_string())),
            );
          }
          assert!(field.parse("").is_err());
          assert!(field.parse("unknown").is_err());
        }
        _ => {}
      }
    }
    let kernel = field("terminal", "kernel");
    assert!(kernel.parse("alacritty").is_ok());
    assert_eq!(kernel.parse("vte").is_ok(), cfg!(target_os = "linux"));
    assert!(field("colors", "theme").parse("My custom theme").is_ok());
    assert!(
      field("terminal", "default_profile")
        .parse("My shell")
        .is_ok()
    );
  }

  #[test]
  fn writes_preserve_other_values_and_reject_invalid_tables_without_mutation() {
    let mut value = toml::Value::try_from(config_with_optional_values()).unwrap();
    let original = value.clone();
    let themes_path = field("appearance", "themes_path");
    themes_path.write(&mut value, " ").unwrap();
    assert_eq!(themes_path.value(&value), "");
    assert!(value["appearance"].get("themes_path").is_none());
    themes_path.write(&mut value, "  replacement  ").unwrap();
    assert_eq!(themes_path.value(&value), "replacement");
    value["appearance"]["themes_path"] = original["appearance"]["themes_path"].clone();
    assert_eq!(value, original);

    for mut invalid in [
      toml::Value::Table(toml::Table::new()),
      toml::from_str("appearance = 'not a table'").unwrap(),
      toml::Value::String("not a config".to_string()),
    ] {
      let before = invalid.clone();
      assert!(themes_path.write(&mut invalid, "new themes").is_err());
      assert!(themes_path.write(&mut invalid, "").is_err());
      assert_eq!(invalid, before);
    }
    assert!(
      field("pane", "inactive_opacity")
        .write(&mut value, "NaN")
        .is_err(),
    );
    assert_eq!(value, original);
  }

  #[test]
  fn scalar_metadata_covers_config_without_duplicates_and_round_trips_defaults() {
    fn scalar_paths(value: &toml::Value, prefix: &str, paths: &mut BTreeSet<String>) {
      if matches!(
        prefix,
        "version"
          | "imports"
          | "profiles"
          | "keybindings"
          | "terminal.env"
          | "auto_update.last_check_unix_secs"
          | "auto_update.restore_workspace_once"
      ) {
        return;
      }
      match value {
        toml::Value::Table(table) => {
          for (key, value) in table {
            let path = if prefix.is_empty() {
              key.clone()
            } else {
              format!("{prefix}.{key}")
            };
            scalar_paths(value, &path, paths);
          }
        }
        toml::Value::Array(_) => {}
        _ => {
          paths.insert(prefix.to_string());
        }
      }
    }

    let original = toml::Value::try_from(config_with_optional_values()).unwrap();
    let mut expected = BTreeSet::new();
    scalar_paths(&original, "", &mut expected);
    let actual: BTreeSet<_> = FIELDS
      .iter()
      .map(|field| format!("{}.{}", field.table, field.key))
      .collect();
    assert_eq!(actual.len(), FIELDS.len(), "duplicate field metadata");
    assert_eq!(actual, expected, "missing or unexpected scalar metadata");

    let mut edited = original.clone();
    for field in FIELDS {
      assert!(Section::ALL.contains(&field.section));
      assert!(!field.label.is_empty());
      assert!(!field.description.is_empty());
      let text = field.value(&original);
      field.write(&mut edited, &text).unwrap();
      assert_eq!(field.value(&edited), text, "{}.{}", field.table, field.key);
      if let Some(toml::Value::Float(before)) = original[field.table].get(field.key) {
        assert_eq!(
          *before as f32,
          edited[field.table][field.key].as_float().unwrap() as f32,
        );
      } else {
        assert_eq!(
          edited[field.table][field.key],
          original[field.table][field.key]
        );
      }
    }
    assert_eq!(field("pane", "inactive_opacity").value(&original), "0.6");
    for (index, section) in Section::ALL.iter().enumerate() {
      assert!(!Section::ALL[..index].contains(section));
      assert!(!section.label().is_empty());
    }
  }
}
