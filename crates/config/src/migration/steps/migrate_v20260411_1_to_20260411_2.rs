use toml::Value;

/// Add startup maximized window configuration support.
pub(crate) fn migrate_v20260411_1_to_20260411_2(value: &mut Value) {
  if let Value::Table(table) = value {
    if !table.contains_key("start_maximized") {
      table.insert("start_maximized".to_string(), Value::Boolean(false));
    }
    table.insert(
      "version".to_string(),
      Value::String("20260411.2".to_string()),
    );
  }
}
