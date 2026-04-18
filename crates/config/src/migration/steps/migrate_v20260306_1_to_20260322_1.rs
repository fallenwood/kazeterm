use toml::Value;

/// Add split pane navigation keybindings (focus_next_pane, focus_previous_pane, swap_split_panes).
pub(crate) fn migrate_v20260306_1_to_20260322_1(value: &mut Value) {
  if let Value::Table(table) = value {
    // New keybinding defaults are handled by serde defaults, no explicit insertion needed.
    table.insert(
      "version".to_string(),
      Value::String("20260322.1".to_string()),
    );
  }
}
