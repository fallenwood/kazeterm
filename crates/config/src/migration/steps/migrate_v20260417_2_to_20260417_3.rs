use toml::Value;

/// Add terminal kernel selection.
pub(crate) fn migrate_v20260417_2_to_20260417_3(value: &mut Value) {
  if let Value::Table(table) = value {
    let terminal = table
      .entry("terminal")
      .or_insert_with(|| Value::Table(toml::map::Map::new()));
    if let Value::Table(terminal_table) = terminal
      && !terminal_table.contains_key("kernel")
    {
      terminal_table.insert("kernel".to_string(), Value::String("alacritty".to_string()));
    }

    table.insert(
      "version".to_string(),
      Value::String("20260417.3".to_string()),
    );
  }
}
