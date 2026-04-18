use toml::Value;

/// Add custom keybindings configuration support.
pub(crate) fn migrate_v20260220_1_to_20260303_1(value: &mut Value) {
  if let Value::Table(table) = value {
    table.insert(
      "version".to_string(),
      Value::String("20260303.1".to_string()),
    );
  }
}
