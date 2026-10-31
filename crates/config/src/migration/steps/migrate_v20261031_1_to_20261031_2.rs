use toml::Value;

/// Preserve OSC 7501 notification behavior without overwriting explicit choices.
pub(crate) fn migrate_v20261031_1_to_20261031_2(value: &mut Value) {
  if let Value::Table(table) = value {
    let notification = table
      .entry("notification")
      .or_insert_with(|| Value::Table(Default::default()));
    if let Value::Table(notification) = notification {
      notification
        .entry("osc7501_enabled")
        .or_insert(Value::Boolean(true));
    }
    table.insert(
      "version".to_string(),
      Value::String("20261031.2".to_string()),
    );
  }
}
