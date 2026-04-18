use toml::Value;

/// Add terminal configuration options: scrollback, cursor, osc52, copy_on_select, env, working_directory.
pub(crate) fn migrate_v20260322_1_to_20260323_1(value: &mut Value) {
  if let Value::Table(table) = value {
    if !table.contains_key("scrollback_lines") {
      table.insert("scrollback_lines".to_string(), Value::Integer(10_000));
    }
    if !table.contains_key("cursor_shape") {
      table.insert(
        "cursor_shape".to_string(),
        Value::String("block".to_string()),
      );
    }
    if !table.contains_key("cursor_blink") {
      table.insert("cursor_blink".to_string(), Value::Boolean(true));
    }
    if !table.contains_key("cursor_blink_interval") {
      table.insert("cursor_blink_interval".to_string(), Value::Integer(750));
    }
    if !table.contains_key("osc52") {
      table.insert("osc52".to_string(), Value::String("copy_only".to_string()));
    }
    if !table.contains_key("copy_on_select") {
      table.insert("copy_on_select".to_string(), Value::Boolean(false));
    }
    table.insert(
      "version".to_string(),
      Value::String("20260323.1".to_string()),
    );
  }
}
