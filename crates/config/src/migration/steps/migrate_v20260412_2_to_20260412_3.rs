use toml::Value;

/// Add configurable tab title debounce support.
pub(crate) fn migrate_v20260412_2_to_20260412_3(value: &mut Value) {
  if let Value::Table(table) = value {
    if !table.contains_key("tab_title_change_delay_ms") {
      table.insert("tab_title_change_delay_ms".to_string(), Value::Integer(200));
    }
    table.insert(
      "version".to_string(),
      Value::String("20260412.3".to_string()),
    );
  }
}
