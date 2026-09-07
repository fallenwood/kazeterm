use toml::Value;

/// Remove the root fade setting now that transitions only animate geometry.
pub(crate) fn migrate_v20260901_1_to_20260905_1(value: &mut Value) {
  if let Value::Table(table) = value {
    if let Some(Value::Table(animation)) = table.get_mut("animation") {
      animation.remove("fade_start_opacity");
    }

    table.insert(
      "version".to_string(),
      Value::String("20260905.1".to_string()),
    );
  }
}
