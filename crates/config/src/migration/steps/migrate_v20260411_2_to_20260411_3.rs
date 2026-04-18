use toml::Value;

/// Add split pane divider width configuration support.
pub(crate) fn migrate_v20260411_2_to_20260411_3(value: &mut Value) {
  if let Value::Table(table) = value {
    if !table.contains_key("split_pane_divider_width") {
      table.insert("split_pane_divider_width".to_string(), Value::Float(6.0));
    }
    table.insert(
      "version".to_string(),
      Value::String("20260411.3".to_string()),
    );
  }
}
