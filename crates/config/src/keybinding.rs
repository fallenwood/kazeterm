use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};
use std::collections::{BTreeMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeybindingList(HashSet<String>);

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
    let mut deduped = HashSet::with_capacity(bindings.len());
    for binding in bindings {
      let binding = binding.trim();
      if !binding.is_empty() {
        deduped.insert(binding.to_string());
      }
    }
    Self(deduped)
  }

  pub fn first(&self) -> Option<&str> {
    self.iter().next()
  }

  pub fn iter(&self) -> impl Iterator<Item = &str> + '_ {
    let mut bindings = self.0.iter().map(String::as_str).collect::<Vec<_>>();
    bindings.sort_unstable();
    bindings.into_iter()
  }

  pub fn clear(&mut self) {
    self.0.clear();
  }

  pub fn insert(&mut self, binding: impl Into<String>) {
    let binding = binding.into();
    let binding = binding.trim();
    if !binding.is_empty() {
      self.0.insert(binding.to_string());
    }
  }

  fn from_value(value: toml::Value) -> Result<Self, String> {
    match value {
      toml::Value::String(binding) => Ok(Self::from_vec(vec![binding])),
      toml::Value::Array(bindings) => {
        let mut parsed = Vec::with_capacity(bindings.len());
        for binding in bindings {
          match binding {
            toml::Value::String(binding) => parsed.push(binding),
            _ => return Err("keybinding arrays must contain only strings".to_string()),
          }
        }
        Ok(Self::from_vec(parsed))
      }
      _ => Err("keybinding values must be strings or arrays of strings".to_string()),
    }
  }

  fn to_sorted_bindings(&self) -> Vec<&str> {
    self.iter().collect()
  }

  pub fn matches(&self, control: bool, shift: bool, alt: bool, platform: bool, key: &str) -> bool {
    self
      .iter()
      .any(|binding| ParsedKeybinding::parse(binding).matches(control, shift, alt, platform, key))
  }

  pub fn display_text(&self) -> String {
    let display = self
      .iter()
      .map(|binding| ParsedKeybinding::parse(binding).display_text())
      .collect::<Vec<_>>()
      .join(" / ");

    if display.is_empty() {
      "Unbound".to_string()
    } else {
      display
    }
  }
}

impl Default for KeybindingList {
  fn default() -> Self {
    Self(HashSet::new())
  }
}

impl Serialize for KeybindingList {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    let bindings = self.to_sorted_bindings();
    match bindings.as_slice() {
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
    self.0.len() == 1 && self.0.contains(*other)
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum KeybindingAction {
  Copy,
  Paste,
  ZoomIn,
  ZoomOut,
  ZoomReset,
  NextTab,
  PreviousTab,
  SelectTab1,
  SelectTab2,
  SelectTab3,
  SelectTab4,
  SelectTab5,
  SelectTab6,
  SelectTab7,
  SelectTab8,
  SelectLastTab,
  ToggleSearch,
  SplitHorizontal,
  SplitVertical,
  ClosePane,
  FocusNextPane,
  FocusPreviousPane,
  FocusPaneUp,
  FocusPaneDown,
  FocusPaneLeft,
  FocusPaneRight,
  SwapSplitPanes,
  ToggleHiddenPanes,
  ToggleFullscreen,
  ToggleTabBar,
  NewTab,
  NewTabProfile1,
  NewTabProfile2,
  NewTabProfile3,
  NewTabProfile4,
  NewTabProfile5,
  NewTabProfile6,
  NewTabProfile7,
  NewTabProfile8,
  NewTabProfile9,
  NewWindow,
  Quit,
  Noop,
}

impl KeybindingAction {
  const ALL: [Self; 43] = [
    Self::Copy,
    Self::Paste,
    Self::ZoomIn,
    Self::ZoomOut,
    Self::ZoomReset,
    Self::NextTab,
    Self::PreviousTab,
    Self::SelectTab1,
    Self::SelectTab2,
    Self::SelectTab3,
    Self::SelectTab4,
    Self::SelectTab5,
    Self::SelectTab6,
    Self::SelectTab7,
    Self::SelectTab8,
    Self::SelectLastTab,
    Self::ToggleSearch,
    Self::SplitHorizontal,
    Self::SplitVertical,
    Self::ClosePane,
    Self::FocusNextPane,
    Self::FocusPreviousPane,
    Self::FocusPaneUp,
    Self::FocusPaneDown,
    Self::FocusPaneLeft,
    Self::FocusPaneRight,
    Self::SwapSplitPanes,
    Self::ToggleHiddenPanes,
    Self::ToggleFullscreen,
    Self::ToggleTabBar,
    Self::NewTab,
    Self::NewTabProfile1,
    Self::NewTabProfile2,
    Self::NewTabProfile3,
    Self::NewTabProfile4,
    Self::NewTabProfile5,
    Self::NewTabProfile6,
    Self::NewTabProfile7,
    Self::NewTabProfile8,
    Self::NewTabProfile9,
    Self::NewWindow,
    Self::Quit,
    Self::Noop,
  ];

  pub(crate) fn from_str(value: &str) -> Option<Self> {
    match value {
      "copy" => Some(Self::Copy),
      "paste" => Some(Self::Paste),
      "zoom_in" => Some(Self::ZoomIn),
      "zoom_out" => Some(Self::ZoomOut),
      "zoom_reset" => Some(Self::ZoomReset),
      "next_tab" => Some(Self::NextTab),
      "previous_tab" => Some(Self::PreviousTab),
      "select_tab_1" => Some(Self::SelectTab1),
      "select_tab_2" => Some(Self::SelectTab2),
      "select_tab_3" => Some(Self::SelectTab3),
      "select_tab_4" => Some(Self::SelectTab4),
      "select_tab_5" => Some(Self::SelectTab5),
      "select_tab_6" => Some(Self::SelectTab6),
      "select_tab_7" => Some(Self::SelectTab7),
      "select_tab_8" => Some(Self::SelectTab8),
      "select_tab_9" | "select_last_tab" => Some(Self::SelectLastTab),
      "toggle_search" => Some(Self::ToggleSearch),
      "split_horizontal" => Some(Self::SplitHorizontal),
      "split_vertical" => Some(Self::SplitVertical),
      "close_pane" => Some(Self::ClosePane),
      "focus_next_pane" => Some(Self::FocusNextPane),
      "focus_previous_pane" => Some(Self::FocusPreviousPane),
      "focus_pane_up" => Some(Self::FocusPaneUp),
      "focus_pane_down" => Some(Self::FocusPaneDown),
      "focus_pane_left" => Some(Self::FocusPaneLeft),
      "focus_pane_right" => Some(Self::FocusPaneRight),
      "swap_split_panes" => Some(Self::SwapSplitPanes),
      "toggle_hidden_panes" => Some(Self::ToggleHiddenPanes),
      "toggle_fullscreen" => Some(Self::ToggleFullscreen),
      "toggle_tab_bar" => Some(Self::ToggleTabBar),
      "new_tab" => Some(Self::NewTab),
      "new_tab_profile_1" => Some(Self::NewTabProfile1),
      "new_tab_profile_2" => Some(Self::NewTabProfile2),
      "new_tab_profile_3" => Some(Self::NewTabProfile3),
      "new_tab_profile_4" => Some(Self::NewTabProfile4),
      "new_tab_profile_5" => Some(Self::NewTabProfile5),
      "new_tab_profile_6" => Some(Self::NewTabProfile6),
      "new_tab_profile_7" => Some(Self::NewTabProfile7),
      "new_tab_profile_8" => Some(Self::NewTabProfile8),
      "new_tab_profile_9" => Some(Self::NewTabProfile9),
      "new_window" => Some(Self::NewWindow),
      "quit" => Some(Self::Quit),
      "noop" => Some(Self::Noop),
      _ => None,
    }
  }

  pub(crate) fn as_str(self) -> &'static str {
    match self {
      Self::Copy => "copy",
      Self::Paste => "paste",
      Self::ZoomIn => "zoom_in",
      Self::ZoomOut => "zoom_out",
      Self::ZoomReset => "zoom_reset",
      Self::NextTab => "next_tab",
      Self::PreviousTab => "previous_tab",
      Self::SelectTab1 => "select_tab_1",
      Self::SelectTab2 => "select_tab_2",
      Self::SelectTab3 => "select_tab_3",
      Self::SelectTab4 => "select_tab_4",
      Self::SelectTab5 => "select_tab_5",
      Self::SelectTab6 => "select_tab_6",
      Self::SelectTab7 => "select_tab_7",
      Self::SelectTab8 => "select_tab_8",
      Self::SelectLastTab => "select_last_tab",
      Self::ToggleSearch => "toggle_search",
      Self::SplitHorizontal => "split_horizontal",
      Self::SplitVertical => "split_vertical",
      Self::ClosePane => "close_pane",
      Self::FocusNextPane => "focus_next_pane",
      Self::FocusPreviousPane => "focus_previous_pane",
      Self::FocusPaneUp => "focus_pane_up",
      Self::FocusPaneDown => "focus_pane_down",
      Self::FocusPaneLeft => "focus_pane_left",
      Self::FocusPaneRight => "focus_pane_right",
      Self::SwapSplitPanes => "swap_split_panes",
      Self::ToggleHiddenPanes => "toggle_hidden_panes",
      Self::ToggleFullscreen => "toggle_fullscreen",
      Self::ToggleTabBar => "toggle_tab_bar",
      Self::NewTab => "new_tab",
      Self::NewTabProfile1 => "new_tab_profile_1",
      Self::NewTabProfile2 => "new_tab_profile_2",
      Self::NewTabProfile3 => "new_tab_profile_3",
      Self::NewTabProfile4 => "new_tab_profile_4",
      Self::NewTabProfile5 => "new_tab_profile_5",
      Self::NewTabProfile6 => "new_tab_profile_6",
      Self::NewTabProfile7 => "new_tab_profile_7",
      Self::NewTabProfile8 => "new_tab_profile_8",
      Self::NewTabProfile9 => "new_tab_profile_9",
      Self::NewWindow => "new_window",
      Self::Quit => "quit",
      Self::Noop => "noop",
    }
  }
}

pub(crate) fn rewrite_keybinding_table_to_key_first(
  table: &mut toml::map::Map<String, toml::Value>,
) {
  let Ok(overrides) = KeybindingConfig::binding_overrides_from_table(table) else {
    return;
  };

  *table = overrides
    .into_iter()
    .map(|(binding, action)| (binding, toml::Value::String(action.as_str().to_string())))
    .collect();
}

pub(crate) fn merge_keybinding_tables(
  target: &mut toml::map::Map<String, toml::Value>,
  overlay: toml::map::Map<String, toml::Value>,
) {
  let Ok(mut merged) = KeybindingConfig::binding_overrides_from_table(target) else {
    for (key, value) in overlay {
      target.insert(key, value);
    }
    return;
  };
  let Ok(overlay_overrides) = KeybindingConfig::binding_overrides_from_table(&overlay) else {
    for (key, value) in overlay {
      target.insert(key, value);
    }
    return;
  };

  for (binding, action) in overlay_overrides {
    merged.insert(binding, action);
  }

  *target = merged
    .into_iter()
    .map(|(binding, action)| (binding, toml::Value::String(action.as_str().to_string())))
    .collect();
}

/// Configuration for custom keyboard shortcuts.
///
/// TOML uses key-first entries such as `"ctrl-shift-c" = "copy"`.
/// Multiple bindings for the same action are represented by repeating the action
/// value under multiple keys, for example `"ctrl-shift-c" = "copy"` and
/// `"ctrl-insert" = "copy"`.
///
/// Custom bindings layer on top of the built-in defaults. Use `"some-key" = "noop"`
/// (or `noop = "some-key"` in the legacy format) to disable a specific default key.
/// Legacy action-first entries are still accepted when loading configs and add
/// bindings instead of replacing defaults.
#[derive(Debug, Clone)]
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
  /// Select tab 1
  pub select_tab_1: KeybindingList,
  /// Select tab 2
  pub select_tab_2: KeybindingList,
  /// Select tab 3
  pub select_tab_3: KeybindingList,
  /// Select tab 4
  pub select_tab_4: KeybindingList,
  /// Select tab 5
  pub select_tab_5: KeybindingList,
  /// Select tab 6
  pub select_tab_6: KeybindingList,
  /// Select tab 7
  pub select_tab_7: KeybindingList,
  /// Select tab 8
  pub select_tab_8: KeybindingList,
  /// Select the last tab
  pub select_last_tab: KeybindingList,
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
  /// Focus the split pane above the active pane
  pub focus_pane_up: KeybindingList,
  /// Focus the split pane below the active pane
  pub focus_pane_down: KeybindingList,
  /// Focus the split pane to the left of the active pane
  pub focus_pane_left: KeybindingList,
  /// Focus the split pane to the right of the active pane
  pub focus_pane_right: KeybindingList,
  /// Swap the two halves of the current split
  pub swap_split_panes: KeybindingList,
  /// Toggle hiding or restoring other panes in the current split layout
  pub toggle_hidden_panes: KeybindingList,
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
  /// Keys explicitly disabled by the user
  pub noop: KeybindingList,
}

impl KeybindingConfig {
  const MAIN_WINDOW_SHORTCUTS: [KeybindingAction; 19] = [
    KeybindingAction::NextTab,
    KeybindingAction::PreviousTab,
    KeybindingAction::SelectTab1,
    KeybindingAction::SelectTab2,
    KeybindingAction::SelectTab3,
    KeybindingAction::SelectTab4,
    KeybindingAction::SelectTab5,
    KeybindingAction::SelectTab6,
    KeybindingAction::SelectTab7,
    KeybindingAction::SelectTab8,
    KeybindingAction::SelectLastTab,
    KeybindingAction::ToggleSearch,
    KeybindingAction::SplitHorizontal,
    KeybindingAction::SplitVertical,
    KeybindingAction::ClosePane,
    KeybindingAction::FocusNextPane,
    KeybindingAction::FocusPreviousPane,
    KeybindingAction::FocusPaneUp,
    KeybindingAction::FocusPaneDown,
  ];

  const MAIN_WINDOW_SHORTCUTS_CONTINUED: [KeybindingAction; 8] = [
    KeybindingAction::FocusPaneLeft,
    KeybindingAction::FocusPaneRight,
    KeybindingAction::SwapSplitPanes,
    KeybindingAction::ToggleHiddenPanes,
    KeybindingAction::ToggleFullscreen,
    KeybindingAction::ToggleTabBar,
    KeybindingAction::NewTab,
    KeybindingAction::Quit,
  ];

  pub fn matches_main_window_shortcut(
    &self,
    control: bool,
    shift: bool,
    alt: bool,
    platform: bool,
    key: &str,
  ) -> bool {
    Self::MAIN_WINDOW_SHORTCUTS
      .into_iter()
      .chain(Self::MAIN_WINDOW_SHORTCUTS_CONTINUED)
      .chain([
        KeybindingAction::NewTabProfile1,
        KeybindingAction::NewTabProfile2,
        KeybindingAction::NewTabProfile3,
        KeybindingAction::NewTabProfile4,
        KeybindingAction::NewTabProfile5,
        KeybindingAction::NewTabProfile6,
        KeybindingAction::NewTabProfile7,
        KeybindingAction::NewTabProfile8,
        KeybindingAction::NewTabProfile9,
      ])
      .any(|action| {
        self
          .binding(action)
          .matches(control, shift, alt, platform, key)
      })
  }

  fn empty() -> Self {
    let mut config = Self::default();
    for action in KeybindingAction::ALL {
      config.binding_mut(action).clear();
    }
    config
  }

  fn from_binding_map(bindings: BTreeMap<String, KeybindingAction>) -> Self {
    let mut config = Self::empty();
    for (binding, action) in bindings {
      config.binding_mut(action).insert(binding);
    }
    config
  }

  fn default_binding_map() -> BTreeMap<String, KeybindingAction> {
    Self::default().to_binding_map()
  }

  fn binding_overrides_from_table(
    table: &toml::map::Map<String, toml::Value>,
  ) -> Result<BTreeMap<String, KeybindingAction>, String> {
    let mut bindings = BTreeMap::new();

    for (key, value) in table {
      if let Some(action) = KeybindingAction::from_str(key) {
        let parsed = KeybindingList::from_value(value.clone())
          .map_err(|error| format!("keybindings.{key}: {error}"))?;
        for binding in parsed.iter() {
          bindings.insert(binding.to_string(), action);
        }
        continue;
      }

      let action_name = value
        .as_str()
        .ok_or_else(|| format!("keybinding '{key}' must map to an action name string"))?;
      let action = KeybindingAction::from_str(action_name)
        .ok_or_else(|| format!("unknown keybinding action '{action_name}' for binding '{key}'"))?;
      let binding = key.trim();
      if binding.is_empty() {
        return Err("keybinding names must not be empty".to_string());
      }
      bindings.insert(binding.to_string(), action);
    }

    Ok(bindings)
  }

  fn binding(&self, action: KeybindingAction) -> &KeybindingList {
    match action {
      KeybindingAction::Copy => &self.copy,
      KeybindingAction::Paste => &self.paste,
      KeybindingAction::ZoomIn => &self.zoom_in,
      KeybindingAction::ZoomOut => &self.zoom_out,
      KeybindingAction::ZoomReset => &self.zoom_reset,
      KeybindingAction::NextTab => &self.next_tab,
      KeybindingAction::PreviousTab => &self.previous_tab,
      KeybindingAction::SelectTab1 => &self.select_tab_1,
      KeybindingAction::SelectTab2 => &self.select_tab_2,
      KeybindingAction::SelectTab3 => &self.select_tab_3,
      KeybindingAction::SelectTab4 => &self.select_tab_4,
      KeybindingAction::SelectTab5 => &self.select_tab_5,
      KeybindingAction::SelectTab6 => &self.select_tab_6,
      KeybindingAction::SelectTab7 => &self.select_tab_7,
      KeybindingAction::SelectTab8 => &self.select_tab_8,
      KeybindingAction::SelectLastTab => &self.select_last_tab,
      KeybindingAction::ToggleSearch => &self.toggle_search,
      KeybindingAction::SplitHorizontal => &self.split_horizontal,
      KeybindingAction::SplitVertical => &self.split_vertical,
      KeybindingAction::ClosePane => &self.close_pane,
      KeybindingAction::FocusNextPane => &self.focus_next_pane,
      KeybindingAction::FocusPreviousPane => &self.focus_previous_pane,
      KeybindingAction::FocusPaneUp => &self.focus_pane_up,
      KeybindingAction::FocusPaneDown => &self.focus_pane_down,
      KeybindingAction::FocusPaneLeft => &self.focus_pane_left,
      KeybindingAction::FocusPaneRight => &self.focus_pane_right,
      KeybindingAction::SwapSplitPanes => &self.swap_split_panes,
      KeybindingAction::ToggleHiddenPanes => &self.toggle_hidden_panes,
      KeybindingAction::ToggleFullscreen => &self.toggle_fullscreen,
      KeybindingAction::ToggleTabBar => &self.toggle_tab_bar,
      KeybindingAction::NewTab => &self.new_tab,
      KeybindingAction::NewTabProfile1 => &self.new_tab_profile_1,
      KeybindingAction::NewTabProfile2 => &self.new_tab_profile_2,
      KeybindingAction::NewTabProfile3 => &self.new_tab_profile_3,
      KeybindingAction::NewTabProfile4 => &self.new_tab_profile_4,
      KeybindingAction::NewTabProfile5 => &self.new_tab_profile_5,
      KeybindingAction::NewTabProfile6 => &self.new_tab_profile_6,
      KeybindingAction::NewTabProfile7 => &self.new_tab_profile_7,
      KeybindingAction::NewTabProfile8 => &self.new_tab_profile_8,
      KeybindingAction::NewTabProfile9 => &self.new_tab_profile_9,
      KeybindingAction::NewWindow => &self.new_window,
      KeybindingAction::Quit => &self.quit,
      KeybindingAction::Noop => &self.noop,
    }
  }

  fn binding_mut(&mut self, action: KeybindingAction) -> &mut KeybindingList {
    match action {
      KeybindingAction::Copy => &mut self.copy,
      KeybindingAction::Paste => &mut self.paste,
      KeybindingAction::ZoomIn => &mut self.zoom_in,
      KeybindingAction::ZoomOut => &mut self.zoom_out,
      KeybindingAction::ZoomReset => &mut self.zoom_reset,
      KeybindingAction::NextTab => &mut self.next_tab,
      KeybindingAction::PreviousTab => &mut self.previous_tab,
      KeybindingAction::SelectTab1 => &mut self.select_tab_1,
      KeybindingAction::SelectTab2 => &mut self.select_tab_2,
      KeybindingAction::SelectTab3 => &mut self.select_tab_3,
      KeybindingAction::SelectTab4 => &mut self.select_tab_4,
      KeybindingAction::SelectTab5 => &mut self.select_tab_5,
      KeybindingAction::SelectTab6 => &mut self.select_tab_6,
      KeybindingAction::SelectTab7 => &mut self.select_tab_7,
      KeybindingAction::SelectTab8 => &mut self.select_tab_8,
      KeybindingAction::SelectLastTab => &mut self.select_last_tab,
      KeybindingAction::ToggleSearch => &mut self.toggle_search,
      KeybindingAction::SplitHorizontal => &mut self.split_horizontal,
      KeybindingAction::SplitVertical => &mut self.split_vertical,
      KeybindingAction::ClosePane => &mut self.close_pane,
      KeybindingAction::FocusNextPane => &mut self.focus_next_pane,
      KeybindingAction::FocusPreviousPane => &mut self.focus_previous_pane,
      KeybindingAction::FocusPaneUp => &mut self.focus_pane_up,
      KeybindingAction::FocusPaneDown => &mut self.focus_pane_down,
      KeybindingAction::FocusPaneLeft => &mut self.focus_pane_left,
      KeybindingAction::FocusPaneRight => &mut self.focus_pane_right,
      KeybindingAction::SwapSplitPanes => &mut self.swap_split_panes,
      KeybindingAction::ToggleHiddenPanes => &mut self.toggle_hidden_panes,
      KeybindingAction::ToggleFullscreen => &mut self.toggle_fullscreen,
      KeybindingAction::ToggleTabBar => &mut self.toggle_tab_bar,
      KeybindingAction::NewTab => &mut self.new_tab,
      KeybindingAction::NewTabProfile1 => &mut self.new_tab_profile_1,
      KeybindingAction::NewTabProfile2 => &mut self.new_tab_profile_2,
      KeybindingAction::NewTabProfile3 => &mut self.new_tab_profile_3,
      KeybindingAction::NewTabProfile4 => &mut self.new_tab_profile_4,
      KeybindingAction::NewTabProfile5 => &mut self.new_tab_profile_5,
      KeybindingAction::NewTabProfile6 => &mut self.new_tab_profile_6,
      KeybindingAction::NewTabProfile7 => &mut self.new_tab_profile_7,
      KeybindingAction::NewTabProfile8 => &mut self.new_tab_profile_8,
      KeybindingAction::NewTabProfile9 => &mut self.new_tab_profile_9,
      KeybindingAction::NewWindow => &mut self.new_window,
      KeybindingAction::Quit => &mut self.quit,
      KeybindingAction::Noop => &mut self.noop,
    }
  }

  fn from_toml_value(value: toml::Value) -> Result<Self, String> {
    let toml::Value::Table(table) = value else {
      return Err("keybindings must be a table".to_string());
    };

    let mut bindings = Self::default_binding_map();
    for (binding, action) in Self::binding_overrides_from_table(&table)? {
      bindings.insert(binding, action);
    }

    Ok(Self::from_binding_map(bindings))
  }

  fn to_binding_map(&self) -> BTreeMap<String, KeybindingAction> {
    let mut bindings = BTreeMap::new();

    for action in KeybindingAction::ALL {
      for binding in self.binding(action).iter() {
        if let Some(previous) = bindings.insert(binding.to_string(), action)
          && previous != action
        {
          tracing::warn!(
            "Keybinding '{}' is assigned to multiple actions; keeping '{}'",
            binding,
            action.as_str()
          );
        }
      }
    }

    bindings
  }

  fn to_key_first_map(&self) -> BTreeMap<String, String> {
    self
      .to_binding_map()
      .into_iter()
      .map(|(binding, action)| (binding, action.as_str().to_string()))
      .collect()
  }
}

impl Serialize for KeybindingConfig {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    self.to_key_first_map().serialize(serializer)
  }
}

impl<'de> Deserialize<'de> for KeybindingConfig {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    let value = toml::Value::deserialize(deserializer)?;
    Self::from_toml_value(value).map_err(D::Error::custom)
  }
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
        next_tab: KeybindingList::from_vec(vec!["ctrl-tab".into(), "cmd-shift-]".into()]),
        previous_tab: KeybindingList::from_vec(vec!["ctrl-shift-tab".into(), "cmd-shift-[".into()]),
        select_tab_1: KeybindingList::new("cmd-1"),
        select_tab_2: KeybindingList::new("cmd-2"),
        select_tab_3: KeybindingList::new("cmd-3"),
        select_tab_4: KeybindingList::new("cmd-4"),
        select_tab_5: KeybindingList::new("cmd-5"),
        select_tab_6: KeybindingList::new("cmd-6"),
        select_tab_7: KeybindingList::new("cmd-7"),
        select_tab_8: KeybindingList::new("cmd-8"),
        select_last_tab: KeybindingList::new("cmd-9"),
        toggle_search: KeybindingList::new("cmd-f"),
        split_horizontal: KeybindingList::new("alt-shift-minus"),
        split_vertical: KeybindingList::new("alt-shift-equal"),
        close_pane: KeybindingList::new("cmd-w"),
        focus_next_pane: KeybindingList::new("cmd-]"),
        focus_previous_pane: KeybindingList::new("cmd-["),
        focus_pane_up: KeybindingList::new("alt-up"),
        focus_pane_down: KeybindingList::new("alt-down"),
        focus_pane_left: KeybindingList::new("alt-left"),
        focus_pane_right: KeybindingList::new("alt-right"),
        swap_split_panes: KeybindingList::new("ctrl-shift-x"),
        toggle_hidden_panes: KeybindingList::new("cmd-shift-h"),
        toggle_fullscreen: KeybindingList::new("cmd-ctrl-f"),
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
        noop: KeybindingList::default(),
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
        select_tab_1: KeybindingList::new("ctrl-alt-1"),
        select_tab_2: KeybindingList::new("ctrl-alt-2"),
        select_tab_3: KeybindingList::new("ctrl-alt-3"),
        select_tab_4: KeybindingList::new("ctrl-alt-4"),
        select_tab_5: KeybindingList::new("ctrl-alt-5"),
        select_tab_6: KeybindingList::new("ctrl-alt-6"),
        select_tab_7: KeybindingList::new("ctrl-alt-7"),
        select_tab_8: KeybindingList::new("ctrl-alt-8"),
        select_last_tab: KeybindingList::new("ctrl-alt-9"),
        toggle_search: KeybindingList::new("ctrl-shift-f"),
        split_horizontal: KeybindingList::new("alt-shift-minus"),
        split_vertical: KeybindingList::new("alt-shift-equal"),
        close_pane: KeybindingList::new("ctrl-shift-w"),
        focus_next_pane: KeybindingList::new("ctrl-shift-]"),
        focus_previous_pane: KeybindingList::new("ctrl-shift-["),
        focus_pane_up: KeybindingList::new("alt-up"),
        focus_pane_down: KeybindingList::new("alt-down"),
        focus_pane_left: KeybindingList::new("alt-left"),
        focus_pane_right: KeybindingList::new("alt-right"),
        swap_split_panes: KeybindingList::new("ctrl-shift-x"),
        toggle_hidden_panes: KeybindingList::new("ctrl-shift-h"),
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
        noop: KeybindingList::default(),
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

  fn expected_default_select_tab_binding(index: usize) -> String {
    if cfg!(target_os = "macos") {
      format!("cmd-{}", index)
    } else {
      format!("ctrl-alt-{}", index)
    }
  }

  fn expected_default_directional_pane_binding(direction: &str) -> String {
    format!("alt-{}", direction)
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

  fn assert_binding_strings(bindings: &KeybindingList, mut expected: Vec<String>) {
    expected.sort();
    expected.dedup();
    assert_eq!(
      bindings.iter().map(ToOwned::to_owned).collect::<Vec<_>>(),
      expected
    );
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

    let select_tab_1 = ParsedKeybinding::parse(config.select_tab_1.first().unwrap());
    if cfg!(target_os = "macos") {
      assert!(select_tab_1.matches(false, false, false, true, "1"));
    } else {
      assert!(select_tab_1.matches(true, false, true, false, "1"));
    }

    let select_last_tab = ParsedKeybinding::parse(config.select_last_tab.first().unwrap());
    if cfg!(target_os = "macos") {
      assert!(select_last_tab.matches(false, false, false, true, "9"));
    } else {
      assert!(select_last_tab.matches(true, false, true, false, "9"));
    }

    let focus_pane_up = ParsedKeybinding::parse(config.focus_pane_up.first().unwrap());
    assert!(focus_pane_up.matches(false, false, true, false, "up"));

    let focus_pane_right = ParsedKeybinding::parse(config.focus_pane_right.first().unwrap());
    assert!(focus_pane_right.matches(false, false, true, false, "right"));
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
    assert_eq!(
      config.select_tab_1.first().unwrap(),
      expected_default_select_tab_binding(1)
    );
    assert_eq!(
      config.select_last_tab.first().unwrap(),
      expected_default_select_tab_binding(9)
    );
    assert_eq!(
      config.focus_pane_up.first().unwrap(),
      expected_default_directional_pane_binding("up")
    );
    assert_eq!(
      config.focus_pane_right.first().unwrap(),
      expected_default_directional_pane_binding("right")
    );
    assert_binding_strings(&config.noop, vec![]);
  }

  #[test]
  fn keybinding_config_deserialize_legacy_partial_override() {
    let toml_str = r#"copy = "ctrl-c""#;
    let config: KeybindingConfig = toml::from_str(toml_str).unwrap();
    assert_binding_strings(
      &config.copy,
      vec![
        expected_default_copy_binding().to_string(),
        "ctrl-c".to_string(),
      ],
    );
    // Non-specified fields use defaults
    assert_eq!(config.paste, expected_default_paste_binding());
    if cfg!(target_os = "macos") {
      assert_eq!(
        config.next_tab,
        KeybindingList::from_vec(vec!["ctrl-tab".into(), "cmd-shift-]".into()])
      );
    } else {
      assert_eq!(config.next_tab, "ctrl-tab");
    }
  }

  #[test]
  fn keybinding_config_deserialize_key_first_partial_override() {
    let toml_str = r##"
"ctrl-c" = "copy"
"ctrl-alt-v" = "paste"
"##;
    let config: KeybindingConfig = toml::from_str(toml_str).unwrap();

    assert_binding_strings(
      &config.copy,
      vec![
        expected_default_copy_binding().to_string(),
        "ctrl-c".to_string(),
      ],
    );
    assert_binding_strings(
      &config.paste,
      vec![
        expected_default_paste_binding().to_string(),
        "ctrl-alt-v".to_string(),
      ],
    );
    assert_eq!(
      config.new_tab.first().unwrap(),
      expected_default_new_tab_binding()
    );
  }

  #[test]
  fn keybinding_config_deserialize_select_tab_9_aliases() {
    let legacy_action_first: KeybindingConfig =
      toml::from_str(r#"select_tab_9 = "alt-shift-9""#).unwrap();
    assert_binding_strings(
      &legacy_action_first.select_last_tab,
      vec![
        expected_default_select_tab_binding(9),
        "alt-shift-9".to_string(),
      ],
    );

    let key_first_alias: KeybindingConfig =
      toml::from_str(r##""alt-shift-9" = "select_tab_9""##).unwrap();
    assert_binding_strings(
      &key_first_alias.select_last_tab,
      vec![
        expected_default_select_tab_binding(9),
        "alt-shift-9".to_string(),
      ],
    );
  }

  #[test]
  fn keybinding_config_roundtrip() {
    let config = KeybindingConfig::default();
    let serialized = toml::to_string_pretty(&config).unwrap();
    let raw: toml::Value = toml::from_str(&serialized).unwrap();
    let table = raw.as_table().unwrap();

    assert_eq!(
      table
        .get(expected_default_copy_binding())
        .unwrap()
        .as_str()
        .unwrap(),
      "copy"
    );
    assert_eq!(
      table
        .get(expected_default_paste_binding())
        .unwrap()
        .as_str()
        .unwrap(),
      "paste"
    );
    assert_eq!(
      table
        .get(&expected_default_select_tab_binding(9))
        .unwrap()
        .as_str()
        .unwrap(),
      "select_last_tab"
    );
    assert!(!table.contains_key("copy"));
    assert!(!table.contains_key("paste"));

    let deserialized: KeybindingConfig = toml::from_str(&serialized).unwrap();
    assert_eq!(config.copy, deserialized.copy);
    assert_eq!(config.paste, deserialized.paste);
    assert_eq!(config.zoom_in, deserialized.zoom_in);
    assert_eq!(config.noop, deserialized.noop);
  }

  #[test]
  fn keybinding_config_deserialize_key_first_multiple_bindings() {
    let toml_str = r##"
"ctrl-shift-c" = "copy"
"ctrl-insert" = "copy"
"##;
    let config: KeybindingConfig = toml::from_str(toml_str).unwrap();

    assert_binding_strings(
      &config.copy,
      vec![
        expected_default_copy_binding().to_string(),
        "ctrl-shift-c".to_string(),
        "ctrl-insert".to_string(),
      ],
    );
    assert!(config.copy.matches(true, true, false, false, "c"));
    assert!(config.copy.matches(true, false, false, false, "insert"));
  }

  #[test]
  fn keybinding_config_deserialize_legacy_multiple_bindings() {
    let toml_str = r#"copy = ["ctrl-shift-c", "ctrl-insert"]"#;
    let config: KeybindingConfig = toml::from_str(toml_str).unwrap();

    assert_binding_strings(
      &config.copy,
      vec![
        expected_default_copy_binding().to_string(),
        "ctrl-shift-c".to_string(),
        "ctrl-insert".to_string(),
      ],
    );
    assert!(config.copy.matches(true, true, false, false, "c"));
    assert!(config.copy.matches(true, false, false, false, "insert"));
  }

  #[test]
  fn keybinding_config_noop_disables_default_binding() {
    let default_copy = expected_default_copy_binding();
    let toml_str = format!(
      r##"
"{}" = "noop"
"ctrl-c" = "copy"
"##,
      default_copy,
    );
    let config: KeybindingConfig = toml::from_str(&toml_str).unwrap();

    assert_binding_strings(&config.copy, vec!["ctrl-c".to_string()]);
    assert_binding_strings(&config.noop, vec![default_copy.to_string()]);
  }

  #[test]
  fn keybinding_config_legacy_noop_disables_default_binding() {
    let default_copy = expected_default_copy_binding();
    let toml_str = format!(
      r#"
copy = "ctrl-c"
noop = "{}"
"#,
      default_copy,
    );
    let config: KeybindingConfig = toml::from_str(&toml_str).unwrap();

    assert_binding_strings(&config.copy, vec!["ctrl-c".to_string()]);
    assert_binding_strings(&config.noop, vec![default_copy.to_string()]);
  }

  #[test]
  fn keybinding_config_reassigns_default_binding_by_key() {
    let default_copy = expected_default_copy_binding();
    let toml_str = format!(r##""{}" = "paste""##, default_copy);
    let config: KeybindingConfig = toml::from_str(&toml_str).unwrap();

    assert_binding_strings(&config.copy, vec![]);
    assert_binding_strings(
      &config.paste,
      vec![
        expected_default_paste_binding().to_string(),
        default_copy.to_string(),
      ],
    );
  }

  #[test]
  fn keybinding_config_matches_main_window_shortcut_for_manual_window_actions() {
    let config = KeybindingConfig::default();

    if cfg!(target_os = "macos") {
      assert!(config.matches_main_window_shortcut(false, false, false, true, "t"));
    } else {
      assert!(config.matches_main_window_shortcut(true, true, false, false, "t"));
    }
  }

  #[test]
  fn keybinding_config_does_not_treat_terminal_actions_as_main_window_shortcuts() {
    let config = KeybindingConfig::default();

    if cfg!(target_os = "macos") {
      assert!(!config.matches_main_window_shortcut(false, false, false, true, "c"));
    } else {
      assert!(!config.matches_main_window_shortcut(true, true, false, false, "c"));
    }
  }

  #[test]
  fn keybinding_list_displays_multiple_bindings() {
    let bindings = KeybindingList::from_vec(vec!["ctrl-shift-c".into(), "ctrl-insert".into()]);
    assert_eq!(bindings.display_text(), "Ctrl+Insert / Ctrl+Shift+C");
  }

  #[test]
  fn keybinding_list_displays_unbound_when_empty() {
    assert_eq!(KeybindingList::default().display_text(), "Unbound");
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
