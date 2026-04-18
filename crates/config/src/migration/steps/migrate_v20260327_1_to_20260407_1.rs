use toml::Value;

/// Add right_click_context_menu configuration.
pub(crate) fn migrate_v20260327_1_to_20260407_1(value: &mut Value) {
  if let Value::Table(table) = value {
    if !table.contains_key("right_click_context_menu") {
      table.insert(
        "right_click_context_menu".to_string(),
        Value::Boolean(false),
      );
    }
    // Remove old string-based right_click field if present
    table.remove("right_click");
    table.insert(
      "version".to_string(),
      Value::String("20260407.1".to_string()),
    );
  }
}
