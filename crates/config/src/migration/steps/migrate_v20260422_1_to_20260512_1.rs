use toml::Value;

/// Add auto-update configuration.
pub(crate) fn migrate_v20260422_1_to_20260512_1(value: &mut Value) {
  if let Value::Table(table) = value {
    let auto_update = table
      .entry("auto_update".to_string())
      .or_insert_with(|| Value::Table(Default::default()));

    if let Value::Table(auto_update) = auto_update {
      auto_update
        .entry("check".to_string())
        .or_insert_with(|| Value::String("never".to_string()));
      auto_update
        .entry("restore_workspace_once".to_string())
        .or_insert_with(|| Value::Boolean(false));
    }

    table.insert(
      "version".to_string(),
      Value::String("20260512.1".to_string()),
    );
  }
}
