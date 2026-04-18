use toml::Value;

pub(crate) fn migrate_v20260407_1_to_20260411_1(value: &mut Value) {
  if let Value::Table(table) = value {
    // Add new_tab and new_tab_profile_N keybindings to existing keybindings section
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

      if !kb.contains_key("new_tab") {
        kb.insert(
          "new_tab".to_string(),
          Value::String(defaults.new_tab.first().unwrap().to_string()),
        );
      }
      for (i, binding) in default_profile_bindings.iter().enumerate() {
        let key = format!("new_tab_profile_{}", i + 1);
        if !kb.contains_key(&key) {
          kb.insert(key, Value::String(binding.first().unwrap().to_string()));
        }
      }
    }
    table.insert(
      "version".to_string(),
      Value::String("20260411.1".to_string()),
    );
  }
}
