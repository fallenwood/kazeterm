use toml::Value;

/// Add background_opacity configuration support.
pub(crate) fn migrate_v20260303_1_to_20260306_1(value: &mut Value) {
  if let Value::Table(table) = value {
    if !table.contains_key("background_opacity") {
      table.insert("background_opacity".to_string(), Value::Float(1.0));
    }
    table.insert(
      "version".to_string(),
      Value::String("20260306.1".to_string()),
    );
  }
}
