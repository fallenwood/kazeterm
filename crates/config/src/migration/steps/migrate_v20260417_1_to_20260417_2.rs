use toml::Value;

/// Add terminal hover-to-focus configuration.
pub(crate) fn migrate_v20260417_1_to_20260417_2(value: &mut Value) {
  if let Value::Table(table) = value {
    let terminal = table
      .entry("terminal")
      .or_insert_with(|| Value::Table(toml::map::Map::new()));
    if let Value::Table(terminal_table) = terminal
      && !terminal_table.contains_key("focus_terminal_on_hover")
    {
      terminal_table.insert("focus_terminal_on_hover".to_string(), Value::Boolean(true));
    }

    table.insert(
      "version".to_string(),
      Value::String("20260417.2".to_string()),
    );
  }
}
