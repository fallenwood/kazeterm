use toml::Value;

/// Add hide_mouse_when_typing configuration to [terminal].
pub(crate) fn migrate_v20260415_2_to_20260415_3(value: &mut Value) {
  if let Value::Table(table) = value {
    let terminal = table
      .entry("terminal")
      .or_insert_with(|| Value::Table(toml::map::Map::new()));
    if let Value::Table(terminal_table) = terminal {
      if !terminal_table.contains_key("hide_mouse_when_typing") {
        terminal_table.insert("hide_mouse_when_typing".to_string(), Value::Boolean(false));
      }
    }

    table.insert(
      "version".to_string(),
      Value::String("20260415.3".to_string()),
    );
  }
}
