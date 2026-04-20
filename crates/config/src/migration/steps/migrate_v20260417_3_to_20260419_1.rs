use toml::Value;

/// Rewrite keybindings to the key-first TOML format (`key = action`).
pub(crate) fn migrate_v20260417_3_to_20260419_1(value: &mut Value) {
  if let Value::Table(table) = value {
    if let Some(Value::Table(keybindings)) = table.get_mut("keybindings") {
      crate::keybinding::rewrite_keybinding_table_to_key_first(keybindings);
    }

    table.insert(
      "version".to_string(),
      Value::String("20260419.1".to_string()),
    );
  }
}
