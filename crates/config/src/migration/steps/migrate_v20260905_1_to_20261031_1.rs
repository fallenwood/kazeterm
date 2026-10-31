use toml::Value;

/// Default desktop notifications to off while preserving explicit choices.
pub(crate) fn migrate_v20260905_1_to_20261031_1(value: &mut Value) {
  if let Value::Table(table) = value {
    let notification = table
      .entry("notification")
      .or_insert_with(|| Value::Table(Default::default()));
    if let Value::Table(notification) = notification {
      notification
        .entry("enabled")
        .or_insert(Value::Boolean(false));
    }
    table.insert(
      "version".to_string(),
      Value::String("20261031.1".to_string()),
    );
  }
}
