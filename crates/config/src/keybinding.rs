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
  /// Focus next split pane
  pub focus_next_pane: String,
  /// Focus previous split pane
  pub focus_previous_pane: String,
  /// Swap the two halves of the current split
  pub swap_split_panes: String,
  /// Toggle fullscreen mode
  pub toggle_fullscreen: String,
  /// Toggle tab bar visibility
  pub toggle_tab_bar: String,
  /// Open a new tab with the default profile
  pub new_tab: String,
  /// Open a new tab with profile 1
  pub new_tab_profile_1: String,
  /// Open a new tab with profile 2
  pub new_tab_profile_2: String,
  /// Open a new tab with profile 3
  pub new_tab_profile_3: String,
  /// Open a new tab with profile 4
  pub new_tab_profile_4: String,
  /// Open a new tab with profile 5
  pub new_tab_profile_5: String,
  /// Open a new tab with profile 6
  pub new_tab_profile_6: String,
  /// Open a new tab with profile 7
  pub new_tab_profile_7: String,
  /// Open a new tab with profile 8
  pub new_tab_profile_8: String,
  /// Open a new tab with profile 9
  pub new_tab_profile_9: String,
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
      focus_next_pane: "ctrl-shift-]".to_string(),
      focus_previous_pane: "ctrl-shift-[".to_string(),
      swap_split_panes: "ctrl-shift-x".to_string(),
      toggle_fullscreen: if cfg!(target_os = "macos") {
        "f12".to_string()
      } else {
        "f11".to_string()
      },
      toggle_tab_bar: "ctrl-shift-b".to_string(),
      new_tab: "ctrl-shift-t".to_string(),
      new_tab_profile_1: "ctrl-shift-1".to_string(),
      new_tab_profile_2: "ctrl-shift-2".to_string(),
      new_tab_profile_3: "ctrl-shift-3".to_string(),
      new_tab_profile_4: "ctrl-shift-4".to_string(),
      new_tab_profile_5: "ctrl-shift-5".to_string(),
      new_tab_profile_6: "ctrl-shift-6".to_string(),
      new_tab_profile_7: "ctrl-shift-7".to_string(),
      new_tab_profile_8: "ctrl-shift-8".to_string(),
      new_tab_profile_9: "ctrl-shift-9".to_string(),
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
  pub platform: bool,
  pub key: String,
}

impl ParsedKeybinding {
  /// Parse a keybinding string like `"ctrl-shift-c"` into components.
  ///
  /// Recognized modifier prefixes: `ctrl-`, `shift-`, `alt-`, `win-`/`cmd-`/`super-`.
  /// Everything after modifiers is treated as the key name.
  pub fn parse(s: &str) -> Self {
    let mut remaining = s;
    let mut control = false;
    let mut shift = false;
    let mut alt = false;
    let mut platform = false;

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
      } else if let Some(rest) = remaining
        .strip_prefix("win-")
        .or_else(|| remaining.strip_prefix("cmd-"))
        .or_else(|| remaining.strip_prefix("super-"))
      {
        platform = true;
        remaining = rest;
      } else {
        break;
      }
    }

    ParsedKeybinding {
      control,
      shift,
      alt,
      platform,
      key: normalize_key_name(remaining).to_string(),
    }
  }

  /// Check if this parsed keybinding matches the given key event parameters.
  ///
  /// On Windows, GPUI converts Shift+digit into the shifted symbol (e.g. `!` for
  /// Shift+1) and clears the shift modifier flag. To handle this, when the binding
  /// specifies shift and a key that has a shifted equivalent, we also accept the
  /// shifted symbol with shift=false from the event.
  pub fn matches(&self, control: bool, shift: bool, alt: bool, platform: bool, key: &str) -> bool {
    if self.control == control
      && self.shift == shift
      && self.alt == alt
      && self.platform == platform
      && self.key == key
    {
      return true;
    }

    // Handle GPUI's shifted-key normalization on Windows:
    // When shift is in the binding but the event has shift=false and a shifted symbol,
    // match if the shifted symbol corresponds to the binding's key.
    if self.shift
      && !shift
      && self.control == control
      && self.alt == alt
      && self.platform == platform
    {
      if let Some(shifted) = shift_key(&self.key) {
        return shifted == key;
      }
    }

    false
  }

  /// Format the keybinding for display in menus, e.g. "ctrl-shift-c" → "Ctrl+Shift+C"
  pub fn display_text(&self) -> String {
    let mut parts: Vec<String> = Vec::new();
    if self.platform {
      parts.push("Win".into());
    }
    if self.control {
      parts.push("Ctrl".into());
    }
    if self.shift {
      parts.push("Shift".into());
    }
    if self.alt {
      parts.push("Alt".into());
    }
    parts.push(display_key(&self.key));
    parts.join("+")
  }
}

/// Map an unshifted key to its shifted symbol on a US keyboard layout.
///
/// On Windows, GPUI converts Shift+digit into the shifted symbol and clears the
/// shift modifier. This mapping lets us compare the binding's key (e.g. `"1"`)
/// against the event's key (e.g. `"!"`).
fn shift_key(key: &str) -> Option<&str> {
  match key {
    "1" => Some("!"),
    "2" => Some("@"),
    "3" => Some("#"),
    "4" => Some("$"),
    "5" => Some("%"),
    "6" => Some("^"),
    "7" => Some("&"),
    "8" => Some("*"),
    "9" => Some("("),
    "0" => Some(")"),
    "`" => Some("~"),
    "-" => Some("_"),
    "=" => Some("+"),
    "[" => Some("{"),
    "]" => Some("}"),
    "\\" => Some("|"),
    ";" => Some(":"),
    "'" => Some("\""),
    "," => Some("<"),
    "." => Some(">"),
    "/" => Some("?"),
    _ => None,
  }
}

/// Normalize human-friendly key names to the symbols GPUI uses in key events.
fn normalize_key_name(key: &str) -> &str {
  match key {
    "minus" => "-",
    "plus" => "+",
    "equal" | "equals" => "=",
    "comma" => ",",
    "period" | "dot" => ".",
    "slash" => "/",
    "backslash" => "\\",
    "semicolon" => ";",
    "quote" | "apostrophe" => "'",
    "backtick" | "grave" => "`",
    "space" => " ",
    "lbracket" | "leftbracket" => "[",
    "rbracket" | "rightbracket" => "]",
    _ => key,
  }
}

/// Capitalize a key name for display: single chars uppercase, multi-char title-case, F-keys uppercase.
fn display_key(key: &str) -> String {
  if key.len() == 1 {
    return key.to_uppercase();
  }
  let mut chars = key.chars();
  match chars.next() {
    Some(c) => {
      let first = c.to_uppercase().to_string();
      first + chars.as_str()
    }
    None => String::new(),
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
        platform: false,
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
        platform: false,
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
        platform: false,
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
        platform: false,
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
        platform: false,
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
        platform: false,
        key: "=".to_string(),
      }
    );
  }

  #[test]
  fn parse_platform_modifier() {
    let kb = ParsedKeybinding::parse("win-ctrl-shift-c");
    assert_eq!(
      kb,
      ParsedKeybinding {
        control: true,
        shift: true,
        alt: false,
        platform: true,
        key: "c".to_string(),
      }
    );
    // cmd- and super- are aliases
    assert_eq!(kb, ParsedKeybinding::parse("cmd-ctrl-shift-c"));
    assert_eq!(kb, ParsedKeybinding::parse("super-ctrl-shift-c"));
  }

  #[test]
  fn matches_keystroke() {
    let kb = ParsedKeybinding::parse("ctrl-shift-c");
    assert!(kb.matches(true, true, false, false, "c"));
    assert!(!kb.matches(true, false, false, false, "c"));
    assert!(!kb.matches(false, true, false, false, "c"));
    assert!(!kb.matches(true, true, false, false, "v"));
  }

  #[test]
  fn matches_shifted_number_keys() {
    // GPUI on Windows converts Shift+1 to key="!" with shift=false.
    // ctrl-shift-1 should match when event has ctrl=true, shift=false, key="!"
    let kb = ParsedKeybinding::parse("ctrl-shift-1");
    assert!(kb.matches(true, true, false, false, "1")); // direct match
    assert!(kb.matches(true, false, false, false, "!")); // GPUI shifted key

    let kb5 = ParsedKeybinding::parse("ctrl-shift-5");
    assert!(kb5.matches(true, true, false, false, "5"));
    assert!(kb5.matches(true, false, false, false, "%"));

    let kb9 = ParsedKeybinding::parse("ctrl-shift-9");
    assert!(kb9.matches(true, true, false, false, "9"));
    assert!(kb9.matches(true, false, false, false, "("));

    // Without shift in the binding, shifted symbols should not match
    let kb_no_shift = ParsedKeybinding::parse("ctrl-1");
    assert!(kb_no_shift.matches(true, false, false, false, "1"));
    assert!(!kb_no_shift.matches(true, false, false, false, "!"));
  }

  #[test]
  fn default_keybindings_parse_correctly() {
    let config = KeybindingConfig::default();
    let copy = ParsedKeybinding::parse(&config.copy);
    assert!(copy.matches(true, true, false, false, "c"));

    let zoom_in = ParsedKeybinding::parse(&config.zoom_in);
    assert!(zoom_in.matches(true, false, false, false, "="));

    let zoom_out = ParsedKeybinding::parse(&config.zoom_out);
    assert!(zoom_out.matches(true, false, false, false, "-"));
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

  #[test]
  fn parse_key_name_aliases() {
    // "alt-shift-minus" should normalize "minus" to "-"
    let kb = ParsedKeybinding::parse("alt-shift-minus");
    assert_eq!(kb.key, "-");
    assert!(kb.alt);
    assert!(kb.shift);

    // "alt-shift-plus" should normalize "plus" to "+"
    let kb = ParsedKeybinding::parse("alt-shift-plus");
    assert_eq!(kb.key, "+");
    assert!(kb.alt);
    assert!(kb.shift);

    // "ctrl-equal" should normalize to "="
    let kb = ParsedKeybinding::parse("ctrl-equal");
    assert_eq!(kb.key, "=");
    assert!(kb.control);

    // "ctrl-space" should normalize to " "
    let kb = ParsedKeybinding::parse("ctrl-space");
    assert_eq!(kb.key, " ");
    assert!(kb.control);
  }

  #[test]
  fn matches_alt_shift_minus() {
    // User config: "alt-shift-minus" should match Alt+Shift+- key press
    let kb = ParsedKeybinding::parse("alt-shift-minus");
    assert!(kb.matches(false, true, true, false, "-")); // direct match
    assert!(kb.matches(false, false, true, false, "_")); // GPUI shifted key (shift+- = _)
  }
}
