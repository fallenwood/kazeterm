use toml::Value;

/// Add configurable tab label min/max widths to [tab].
pub(crate) fn migrate_v20260415_1_to_20260415_2(value: &mut Value) {
  if let Value::Table(table) = value {
    let tab = table
      .entry("tab")
      .or_insert_with(|| Value::Table(toml::map::Map::new()));
    if let Value::Table(tab_table) = tab {
      if !tab_table.contains_key("label_min_width") {
        tab_table.insert("label_min_width".to_string(), Value::Float(60.0));
      }
      if !tab_table.contains_key("label_max_width") {
        tab_table.insert("label_max_width".to_string(), Value::Float(200.0));
      }
    }

    table.insert(
      "version".to_string(),
      Value::String("20260415.2".to_string()),
    );
  }
}
