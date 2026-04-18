use toml::Value;

/// Add vertical tab configuration support.
pub(crate) fn migrate_v20260208_1_to_20260220_1(value: &mut Value) {
  if let Value::Table(table) = value {
    if !table.contains_key("vertical_tabs") {
      table.insert("vertical_tabs".to_string(), Value::Boolean(false));
    }
    table.insert(
      "version".to_string(),
      Value::String("20260220.1".to_string()),
    );
  }
}
