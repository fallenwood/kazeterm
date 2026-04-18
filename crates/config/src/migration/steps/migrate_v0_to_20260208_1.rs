use toml::Value;

/// Migrate config with no version field to the first versioned format.
pub(crate) fn migrate_v0_to_20260208_1(value: &mut Value) {
  if let Value::Table(table) = value {
    table.insert(
      "version".to_string(),
      Value::String("20260208.1".to_string()),
    );
  }
}
