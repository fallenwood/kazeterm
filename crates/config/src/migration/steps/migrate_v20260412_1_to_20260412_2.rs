use toml::Value;

/// Add config overlay import support.
pub(crate) fn migrate_v20260412_1_to_20260412_2(value: &mut Value) {
  if let Value::Table(table) = value {
    if !table.contains_key("imports") {
      table.insert("imports".to_string(), Value::Array(Vec::new()));
    }
    table.insert(
      "version".to_string(),
      Value::String("20260412.2".to_string()),
    );
  }
}
