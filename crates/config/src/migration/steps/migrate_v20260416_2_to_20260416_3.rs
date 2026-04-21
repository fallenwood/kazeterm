use toml::Value;

/// Add direct tab selection keybindings to existing keybinding sections.
pub(crate) fn migrate_v20260416_2_to_20260416_3(value: &mut Value) {
  if let Value::Table(table) = value {
    if let Some(Value::Table(kb)) = table.get_mut("keybindings") {
      let defaults = crate::KeybindingConfig::default();
      let select_tab_bindings = [
        ("select_tab_1", &defaults.select_tab_1),
        ("select_tab_2", &defaults.select_tab_2),
        ("select_tab_3", &defaults.select_tab_3),
        ("select_tab_4", &defaults.select_tab_4),
        ("select_tab_5", &defaults.select_tab_5),
        ("select_tab_6", &defaults.select_tab_6),
        ("select_tab_7", &defaults.select_tab_7),
        ("select_tab_8", &defaults.select_tab_8),
        ("select_last_tab", &defaults.select_last_tab),
      ];

      for (key, binding) in select_tab_bindings {
        if !kb.contains_key(key) {
          kb.insert(
            key.to_string(),
            Value::String(binding.first().unwrap().to_string()),
          );
        }
      }
    }

    table.insert(
      "version".to_string(),
      Value::String("20260416.3".to_string()),
    );
  }
}
