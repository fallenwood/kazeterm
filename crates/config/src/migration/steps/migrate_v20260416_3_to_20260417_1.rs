use toml::Value;

/// Add directional split-pane focus keybindings to existing keybinding sections.
pub(crate) fn migrate_v20260416_3_to_20260417_1(value: &mut Value) {
  if let Value::Table(table) = value {
    if let Some(Value::Table(kb)) = table.get_mut("keybindings") {
      let defaults = crate::KeybindingConfig::default();
      let directional_bindings = [
        ("focus_pane_up", &defaults.focus_pane_up),
        ("focus_pane_down", &defaults.focus_pane_down),
        ("focus_pane_left", &defaults.focus_pane_left),
        ("focus_pane_right", &defaults.focus_pane_right),
      ];

      for (key, binding) in directional_bindings {
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
      Value::String("20260417.1".to_string()),
    );
  }
}
