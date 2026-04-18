use toml::Value;

/// Fix macOS legacy tab shortcuts that were inserted with non-macOS defaults.
/// Add window.key_debug_mode configuration support.
pub(crate) fn migrate_v20260416_1_to_20260416_2(value: &mut Value) {
  if let Value::Table(table) = value {
    if cfg!(target_os = "macos") {
      if let Some(Value::Table(kb)) = table.get_mut("keybindings") {
        let defaults = crate::KeybindingConfig::default();
        let default_profile_bindings = [
          &defaults.new_tab_profile_1,
          &defaults.new_tab_profile_2,
          &defaults.new_tab_profile_3,
          &defaults.new_tab_profile_4,
          &defaults.new_tab_profile_5,
          &defaults.new_tab_profile_6,
          &defaults.new_tab_profile_7,
          &defaults.new_tab_profile_8,
          &defaults.new_tab_profile_9,
        ];

        if matches!(kb.get("new_tab"), Some(Value::String(binding)) if binding == "ctrl-shift-t") {
          kb.insert(
            "new_tab".to_string(),
            Value::String(defaults.new_tab.first().unwrap().to_string()),
          );
        }

        for (i, default_binding) in default_profile_bindings.iter().enumerate() {
          let key = format!("new_tab_profile_{}", i + 1);
          let legacy_binding = format!("ctrl-shift-{}", i + 1);
          if matches!(kb.get(&key), Some(Value::String(binding)) if binding == &legacy_binding) {
            kb.insert(
              key,
              Value::String(default_binding.first().unwrap().to_string()),
            );
          }
        }
      }
    }

    let window = table
      .entry("window")
      .or_insert_with(|| Value::Table(toml::map::Map::new()));
    if let Value::Table(window_table) = window
      && !window_table.contains_key("key_debug_mode")
    {
      window_table.insert("key_debug_mode".to_string(), Value::Boolean(false));
    }

    table.insert(
      "version".to_string(),
      Value::String("20260416.2".to_string()),
    );
  }
}
