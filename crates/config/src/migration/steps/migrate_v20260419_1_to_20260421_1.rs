use toml::Value;

/// Rename the last-tab keybinding action to `select_last_tab`.
pub(crate) fn migrate_v20260419_1_to_20260421_1(value: &mut Value) {
  if let Value::Table(table) = value {
    if let Some(Value::Table(keybindings)) = table.get_mut("keybindings") {
      if let Some(binding) = keybindings.remove("select_tab_9")
        && !keybindings.contains_key("select_last_tab")
      {
        keybindings.insert("select_last_tab".to_string(), binding);
      }

      for (_, action) in keybindings.iter_mut() {
        if matches!(action.as_str(), Some("select_tab_9")) {
          *action = Value::String("select_last_tab".to_string());
        }
      }
    }

    table.insert(
      "version".to_string(),
      Value::String("20260421.1".to_string()),
    );
  }
}
