use toml::Value;

/// Add a toggle shortcut for hiding or restoring other panes.
pub(crate) fn migrate_v20260421_1_to_20260422_1(value: &mut Value) {
  if let Value::Table(table) = value {
    if let Some(Value::Table(keybindings)) = table.get_mut("keybindings") {
      let defaults = crate::KeybindingConfig::default();
      if let Some(binding) = defaults.toggle_hidden_panes.first()
        && !keybindings.contains_key(binding)
      {
        keybindings.insert(
          binding.to_string(),
          Value::String("toggle_hidden_panes".to_string()),
        );
      }
    }

    table.insert(
      "version".to_string(),
      Value::String("20260422.1".to_string()),
    );
  }
}
