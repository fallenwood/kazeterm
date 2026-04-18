use toml::Value;

/// Add ctrl_scroll_zoom configuration to [terminal].
pub(crate) fn migrate_v20260414_2_to_20260415_1(value: &mut Value) {
  if let Value::Table(table) = value {
    let terminal = table
      .entry("terminal")
      .or_insert_with(|| Value::Table(toml::map::Map::new()));
    if let Value::Table(terminal_table) = terminal {
      if !terminal_table.contains_key("ctrl_scroll_zoom") {
        terminal_table.insert("ctrl_scroll_zoom".to_string(), Value::Boolean(true));
      }
    }

    table.insert(
      "version".to_string(),
      Value::String("20260415.1".to_string()),
    );
  }
}
