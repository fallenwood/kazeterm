use serde::{Deserialize, Serialize};

/// Configuration for custom keyboard shortcuts.
///
/// Each field maps an action name to a keystroke string using the format:
/// `[modifier-]...[key]` where modifiers can be `ctrl`, `shift`, `alt`.
///
/// Examples: `"ctrl-shift-c"`, `"ctrl-tab"`, `"ctrl-="`, `"alt-1"`
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct KeybindingConfig {
  /// Copy selection to clipboard
  pub copy: String,
  /// Paste from clipboard
  pub paste: String,
  /// Zoom in terminal text
  pub zoom_in: String,
  /// Zoom out terminal text
  pub zoom_out: String,
  /// Reset zoom level
  pub zoom_reset: String,
  /// Switch to next tab
  pub next_tab: String,
  /// Switch to previous tab
  pub previous_tab: String,
  /// Toggle search bar
  pub toggle_search: String,
  /// Split pane horizontally
  pub split_horizontal: String,
  /// Split pane vertically
  pub split_vertical: String,
  /// Close active pane
  pub close_pane: String,
}

impl Default for KeybindingConfig {
  fn default() -> Self {
    Self {
      copy: "ctrl-shift-c".to_string(),
      paste: "ctrl-shift-v".to_string(),
      zoom_in: "ctrl-=".to_string(),
      zoom_out: "ctrl--".to_string(),
      zoom_reset: "ctrl-0".to_string(),
      next_tab: "ctrl-tab".to_string(),
      previous_tab: "ctrl-shift-tab".to_string(),
      toggle_search: "ctrl-shift-f".to_string(),
      split_horizontal: "ctrl-shift-d".to_string(),
      split_vertical: "ctrl-shift-e".to_string(),
      close_pane: "ctrl-shift-w".to_string(),
    }
  }
}

/// A parsed keybinding with separate modifier and key components.
///
/// Use [`ParsedKeybinding::parse`] to convert a keybinding string
/// (e.g., `"ctrl-shift-c"`) into this structured form for matching
/// against key events.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedKeybinding {
  pub control: bool,
  pub shift: bool,
  pub alt: bool,
  pub key: String,
}

impl ParsedKeybinding {
  /// Parse a keybinding string like `"ctrl-shift-c"` into components.
  ///
  /// Recognized modifier prefixes: `ctrl-`, `shift-`, `alt-`.
  /// Everything after modifiers is treated as the key name.
  pub fn parse(s: &str) -> Self {
    let mut remaining = s;
    let mut control = false;
    let mut shift = false;
    let mut alt = false;

    loop {
      if let Some(rest) = remaining.strip_prefix("ctrl-") {
        control = true;
        remaining = rest;
      } else if let Some(rest) = remaining.strip_prefix("shift-") {
        shift = true;
        remaining = rest;
      } else if let Some(rest) = remaining.strip_prefix("alt-") {
        alt = true;
        remaining = rest;
      } else {
        break;
      }
    }

    ParsedKeybinding {
      control,
      shift,
      alt,
      key: remaining.to_string(),
    }
  }

  /// Check if this parsed keybinding matches the given key event parameters.
  pub fn matches(&self, control: bool, shift: bool, alt: bool, key: &str) -> bool {
    self.control == control && self.shift == shift && self.alt == alt && self.key == key
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parse_simple_key() {
    let kb = ParsedKeybinding::parse("tab");
    assert_eq!(
      kb,
      ParsedKeybinding {
        control: false,
        shift: false,
        alt: false,
        key: "tab".to_string(),
      }
    );
  }

  #[test]
  fn parse_single_modifier() {
    let kb = ParsedKeybinding::parse("ctrl-c");
    assert_eq!(
      kb,
      ParsedKeybinding {
        control: true,
        shift: false,
        alt: false,
        key: "c".to_string(),
      }
    );
  }

  #[test]
  fn parse_multiple_modifiers() {
    let kb = ParsedKeybinding::parse("ctrl-shift-c");
    assert_eq!(
      kb,
      ParsedKeybinding {
        control: true,
        shift: true,
        alt: false,
        key: "c".to_string(),
      }
    );
  }

  #[test]
  fn parse_all_modifiers() {
    let kb = ParsedKeybinding::parse("ctrl-shift-alt-x");
    assert_eq!(
      kb,
      ParsedKeybinding {
        control: true,
        shift: true,
        alt: true,
        key: "x".to_string(),
      }
    );
  }

  #[test]
  fn parse_minus_key() {
    // "ctrl--" means ctrl + minus
    let kb = ParsedKeybinding::parse("ctrl--");
    assert_eq!(
      kb,
      ParsedKeybinding {
        control: true,
        shift: false,
        alt: false,
        key: "-".to_string(),
      }
    );
  }

  #[test]
  fn parse_equals_key() {
    let kb = ParsedKeybinding::parse("ctrl-=");
    assert_eq!(
      kb,
      ParsedKeybinding {
        control: true,
        shift: false,
        alt: false,
        key: "=".to_string(),
      }
    );
  }

  #[test]
  fn matches_keystroke() {
    let kb = ParsedKeybinding::parse("ctrl-shift-c");
    assert!(kb.matches(true, true, false, "c"));
    assert!(!kb.matches(true, false, false, "c"));
    assert!(!kb.matches(false, true, false, "c"));
    assert!(!kb.matches(true, true, false, "v"));
  }

  #[test]
  fn default_keybindings_parse_correctly() {
    let config = KeybindingConfig::default();
    let copy = ParsedKeybinding::parse(&config.copy);
    assert!(copy.matches(true, true, false, "c"));

    let zoom_in = ParsedKeybinding::parse(&config.zoom_in);
    assert!(zoom_in.matches(true, false, false, "="));

    let zoom_out = ParsedKeybinding::parse(&config.zoom_out);
    assert!(zoom_out.matches(true, false, false, "-"));
  }

  #[test]
  fn keybinding_config_deserialize_defaults() {
    let toml_str = "";
    let config: KeybindingConfig = toml::from_str(toml_str).unwrap();
    assert_eq!(config.copy, "ctrl-shift-c");
    assert_eq!(config.paste, "ctrl-shift-v");
  }

  #[test]
  fn keybinding_config_deserialize_partial_override() {
    let toml_str = r#"copy = "ctrl-c""#;
    let config: KeybindingConfig = toml::from_str(toml_str).unwrap();
    assert_eq!(config.copy, "ctrl-c");
    // Non-specified fields use defaults
    assert_eq!(config.paste, "ctrl-shift-v");
    assert_eq!(config.next_tab, "ctrl-tab");
  }

  #[test]
  fn keybinding_config_roundtrip() {
    let config = KeybindingConfig::default();
    let serialized = toml::to_string_pretty(&config).unwrap();
    let deserialized: KeybindingConfig = toml::from_str(&serialized).unwrap();
    assert_eq!(config.copy, deserialized.copy);
    assert_eq!(config.paste, deserialized.paste);
    assert_eq!(config.zoom_in, deserialized.zoom_in);
  }
}
