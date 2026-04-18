use toml::Value;

/// Add inactive_pane_opacity configuration for dimming unfocused split panes.
pub(crate) fn migrate_v20260411_3_to_20260412_1(value: &mut Value) {
  if let Value::Table(table) = value {
    if !table.contains_key("inactive_pane_opacity") {
      table.insert("inactive_pane_opacity".to_string(), Value::Float(0.6));
    }
    table.insert(
      "version".to_string(),
      Value::String("20260412.1".to_string()),
    );
  }
}
