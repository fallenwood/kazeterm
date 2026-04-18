use toml::Value;

/// Add background_blur configuration support.
pub(crate) fn migrate_v20260323_2_to_20260327_1(value: &mut Value) {
  if let Value::Table(table) = value {
    if !table.contains_key("background_blur") {
      table.insert("background_blur".to_string(), Value::Boolean(false));
    }
    table.insert(
      "version".to_string(),
      Value::String("20260327.1".to_string()),
    );
  }
}
