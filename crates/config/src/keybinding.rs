use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeybindingList(Vec<String>);

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum KeybindingListRepr {
  Single(String),
  Multiple(Vec<String>),
}

impl KeybindingList {
  pub fn new(binding: impl Into<String>) -> Self {
    Self::from_vec(vec![binding.into()])
  }

  pub fn from_vec(bindings: Vec<String>) -> Self {
    Self(
      bindings
        .into_iter()
        .map(|binding| binding.trim().to_string())
        .filter(|binding| !binding.is_empty())
        .collect(),
    )
  }

  pub fn first(&self) -> Option<&str> {
    self.0.first().map(String::as_str)
  }

  pub fn iter(&self) -> impl Iterator<Item = &str> {
    self.0.iter().map(String::as_str)
  }

  pub fn matches(&self, control: bool, shift: bool, alt: bool, platform: bool, key: &str) -> bool {
    self
      .iter()
      .any(|binding| ParsedKeybinding::parse(binding).matches(control, shift, alt, platform, key))
  }

  pub fn display_text(&self) -> String {
    self
      .iter()
      .map(|binding| ParsedKeybinding::parse(binding).display_text())
      .collect::<Vec<_>>()
      .join(" / ")
  }
}

impl Default for KeybindingList {
  fn default() -> Self {
    Self(Vec::new())
  }
}

impl Serialize for KeybindingList {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    match self.0.as_slice() {
      [binding] => serializer.serialize_str(binding),
      bindings => bindings.serialize(serializer),
    }
  }
}

impl<'de> Deserialize<'de> for KeybindingList {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    let repr = KeybindingListRepr::deserialize(deserializer)?;
    Ok(match repr {
      KeybindingListRepr::Single(binding) => Self::from_vec(vec![binding]),
      KeybindingListRepr::Multiple(bindings) => Self::from_vec(bindings),
    })
  }
}

impl PartialEq<&str> for KeybindingList {
  fn eq(&self, other: &&str) -> bool {
    matches!(self.0.as_slice(), [binding] if binding == *other)
  }
}

/// Configuration for custom keyboard shortcuts.
///
/// Each field maps an action name to either one keystroke string or an array of
/// keystroke strings using the format:
/// `[modifier-]...[key]` where modifiers can be `ctrl`, `shift`, `alt`.
///
/// Examples: `"ctrl-shift-c"`, `["ctrl-shift-c", "ctrl-insert"]`, `"ctrl-tab"`
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct KeybindingConfig {
  /// Copy selection to clipboard
  pub copy: KeybindingList,
  /// Paste from clipboard
  pub paste: KeybindingList,
  /// Zoom in terminal text
  pub zoom_in: KeybindingList,
  /// Zoom out terminal text
  pub zoom_out: KeybindingList,
  /// Reset zoom level
  pub zoom_reset: KeybindingList,
  /// Switch to next tab
  pub next_tab: KeybindingList,
  /// Switch to previous tab
  pub previous_tab: KeybindingList,
  /// Toggle search bar
  pub toggle_search: KeybindingList,
  /// Split pane horizontally
  pub split_horizontal: KeybindingList,
  /// Split pane vertically
  pub split_vertical: KeybindingList,
  /// Close active pane
  pub close_pane: KeybindingList,
  /// Focus next split pane
  pub focus_next_pane: KeybindingList,
  /// Focus previous split pane
  pub focus_previous_pane: KeybindingList,
  /// Swap the two halves of the current split
  pub swap_split_panes: KeybindingList,
  /// Toggle fullscreen mode
  pub toggle_fullscreen: KeybindingList,
  /// Toggle tab bar visibility
  pub toggle_tab_bar: KeybindingList,
  /// Open a new tab with the default profile
  pub new_tab: KeybindingList,
  /// Open a new tab with profile 1
  pub new_tab_profile_1: KeybindingList,
  /// Open a new tab with profile 2
  pub new_tab_profile_2: KeybindingList,
  /// Open a new tab with profile 3
  pub new_tab_profile_3: KeybindingList,
  /// Open a new tab with profile 4
  pub new_tab_profile_4: KeybindingList,
  /// Open a new tab with profile 5
  pub new_tab_profile_5: KeybindingList,
  /// Open a new tab with profile 6
  pub new_tab_profile_6: KeybindingList,
  /// Open a new tab with profile 7
  pub new_tab_profile_7: KeybindingList,
  /// Open a new tab with profile 8
  pub new_tab_profile_8: KeybindingList,
  /// Open a new tab with profile 9
  pub new_tab_profile_9: KeybindingList,
  /// Open a new window
  pub new_window: KeybindingList,
  /// Quit the application
  pub quit: KeybindingList,
}

impl Default for KeybindingConfig {
  fn default() -> Self {
    if cfg!(target_os = "macos") {
      Self {
        copy: KeybindingList::new("cmd-c"),
        paste: KeybindingList::new("cmd-v"),
        zoom_in: KeybindingList::new("cmd-="),
        zoom_out: KeybindingList::new("cmd--"),
        zoom_reset: KeybindingList::new("cmd-0"),
        next_tab: KeybindingList::new("ctrl-tab"),
        previous_tab: KeybindingList::new("ctrl-shift-tab"),
        toggle_search: KeybindingList::new("ctrl-shift-f"),
        split_horizontal: KeybindingList::new("ctrl-shift-d"),
        split_vertical: KeybindingList::new("ctrl-shift-e"),
        close_pane: KeybindingList::new("ctrl-shift-w"),
        focus_next_pane: KeybindingList::new("ctrl-shift-]"),
        focus_previous_pane: KeybindingList::new("ctrl-shift-["),
        swap_split_panes: KeybindingList::new("ctrl-shift-x"),
        toggle_fullscreen: KeybindingList::new("f12"),
        toggle_tab_bar: KeybindingList::new("ctrl-shift-b"),
        new_tab: KeybindingList::new("cmd-t"),
        new_tab_profile_1: KeybindingList::new("ctrl-shift-1"),
        new_tab_profile_2: KeybindingList::new("ctrl-shift-2"),
        new_tab_profile_3: KeybindingList::new("ctrl-shift-3"),
        new_tab_profile_4: KeybindingList::new("ctrl-shift-4"),
        new_tab_profile_5: KeybindingList::new("ctrl-shift-5"),
        new_tab_profile_6: KeybindingList::new("ctrl-shift-6"),
        new_tab_profile_7: KeybindingList::new("ctrl-shift-7"),
        new_tab_profile_8: KeybindingList::new("ctrl-shift-8"),
        new_tab_profile_9: KeybindingList::new("ctrl-shift-9"),
        new_window: KeybindingList::new("cmd-n"),
        quit: KeybindingList::new("cmd-q"),
      }
    } else {
      Self {
        copy: KeybindingList::new("ctrl-shift-c"),
        paste: KeybindingList::new("ctrl-shift-v"),
        zoom_in: KeybindingList::new("ctrl-="),
        zoom_out: KeybindingList::new("ctrl--"),
        zoom_reset: KeybindingList::new("ctrl-0"),
        next_tab: KeybindingList::new("ctrl-tab"),
        previous_tab: KeybindingList::new("ctrl-shift-tab"),
        toggle_search: KeybindingList::new("ctrl-shift-f"),
        split_horizontal: KeybindingList::new("ctrl-shift-d"),
        split_vertical: KeybindingList::new("ctrl-shift-e"),
        close_pane: KeybindingList::new("ctrl-shift-w"),
        focus_next_pane: KeybindingList::new("ctrl-shift-]"),
        focus_previous_pane: KeybindingList::new("ctrl-shift-["),
        swap_split_panes: KeybindingList::new("ctrl-shift-x"),
        toggle_fullscreen: KeybindingList::new("f11"),
        toggle_tab_bar: KeybindingList::new("ctrl-shift-b"),
        new_tab: KeybindingList::new("ctrl-shift-t"),
        new_tab_profile_1: KeybindingList::new("ctrl-shift-1"),
        new_tab_profile_2: KeybindingList::new("ctrl-shift-2"),
        new_tab_profile_3: KeybindingList::new("ctrl-shift-3"),
        new_tab_profile_4: KeybindingList::new("ctrl-shift-4"),
        new_tab_profile_5: KeybindingList::new("ctrl-shift-5"),
        new_tab_profile_6: KeybindingList::new("ctrl-shift-6"),
        new_tab_profile_7: KeybindingList::new("ctrl-shift-7"),
        new_tab_profile_8: KeybindingList::new("ctrl-shift-8"),
        new_tab_profile_9: KeybindingList::new("ctrl-shift-9"),
        new_window: KeybindingList::new("ctrl-shift-n"),
        quit: KeybindingList::new("alt-f4"),
      }
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
      parts.push(platform_modifier_label().into());
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

fn platform_modifier_label() -> &'static str {
  if cfg!(target_os = "macos") {
    "Cmd"
  } else if cfg!(target_os = "windows") {
    "Win"
  } else {
    "Super"
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

  fn expected_default_copy_binding() -> &'static str {
    if cfg!(target_os = "macos") {
      "cmd-c"
    } else {
      "ctrl-shift-c"
    }
  }

  fn expected_default_paste_binding() -> &'static str {
    if cfg!(target_os = "macos") {
      "cmd-v"
    } else {
      "ctrl-shift-v"
    }
  }

  fn expected_default_new_tab_binding() -> &'static str {
    if cfg!(target_os = "macos") {
      "cmd-t"
    } else {
      "ctrl-shift-t"
    }
  }

  fn expected_default_new_tab_profile_binding(index: usize) -> String {
    format!("ctrl-shift-{}", index)
  }

  fn expected_platform_modifier_label() -> &'static str {
    if cfg!(target_os = "macos") {
      "Cmd"
    } else if cfg!(target_os = "windows") {
      "Win"
    } else {
      "Super"
    }
  }

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
    let copy = ParsedKeybinding::parse(config.copy.first().unwrap());
    if cfg!(target_os = "macos") {
      assert!(copy.matches(false, false, false, true, "c"));
    } else {
      assert!(copy.matches(true, true, false, false, "c"));
    }

    let zoom_in = ParsedKeybinding::parse(config.zoom_in.first().unwrap());
    if cfg!(target_os = "macos") {
      assert!(zoom_in.matches(false, false, false, true, "="));
    } else {
      assert!(zoom_in.matches(true, false, false, false, "="));
    }

    let zoom_out = ParsedKeybinding::parse(config.zoom_out.first().unwrap());
    if cfg!(target_os = "macos") {
      assert!(zoom_out.matches(false, false, false, true, "-"));
    } else {
      assert!(zoom_out.matches(true, false, false, false, "-"));
    }

    let new_tab = ParsedKeybinding::parse(config.new_tab.first().unwrap());
    if cfg!(target_os = "macos") {
      assert!(new_tab.matches(false, false, false, true, "t"));
    } else {
      assert!(new_tab.matches(true, true, false, false, "t"));
    }
  }

  #[test]
  fn keybinding_config_deserialize_defaults() {
    let toml_str = "";
    let config: KeybindingConfig = toml::from_str(toml_str).unwrap();
    assert_eq!(config.copy, expected_default_copy_binding());
    assert_eq!(config.paste, expected_default_paste_binding());
    assert_eq!(
      config.new_tab.first().unwrap(),
      expected_default_new_tab_binding()
    );
    assert_eq!(
      config.new_tab_profile_1.first().unwrap(),
      expected_default_new_tab_profile_binding(1)
    );
  }

  #[test]
  fn keybinding_config_deserialize_partial_override() {
    let toml_str = r#"copy = "ctrl-c""#;
    let config: KeybindingConfig = toml::from_str(toml_str).unwrap();
    assert_eq!(config.copy, "ctrl-c");
    // Non-specified fields use defaults
    assert_eq!(config.paste, expected_default_paste_binding());
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
  fn keybinding_config_deserialize_multiple_bindings() {
    let toml_str = r#"copy = ["ctrl-shift-c", "ctrl-insert"]"#;
    let config: KeybindingConfig = toml::from_str(toml_str).unwrap();

    assert_eq!(
      config.copy.iter().collect::<Vec<_>>(),
      vec!["ctrl-shift-c", "ctrl-insert"]
    );
    assert!(config.copy.matches(true, true, false, false, "c"));
    assert!(config.copy.matches(true, false, false, false, "insert"));
  }

  #[test]
  fn keybinding_list_displays_multiple_bindings() {
    let bindings = KeybindingList::from_vec(vec!["ctrl-shift-c".into(), "ctrl-insert".into()]);
    assert_eq!(bindings.display_text(), "Ctrl+Shift+C / Ctrl+Insert");
  }

  #[test]
  fn keybinding_list_serializes_single_binding_as_string() {
    #[derive(Serialize)]
    struct Wrapper {
      copy: KeybindingList,
    }

    let bindings = KeybindingList::new("ctrl-shift-c");
    let serialized = toml::to_string(&Wrapper { copy: bindings }).unwrap();
    assert_eq!(serialized.trim(), "copy = \"ctrl-shift-c\"");
  }

  #[test]
  fn display_text_formats_platform_modifier_for_current_os() {
    let kb = ParsedKeybinding::parse("cmd-c");
    assert_eq!(
      kb.display_text(),
      format!("{}+C", expected_platform_modifier_label())
    );
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
