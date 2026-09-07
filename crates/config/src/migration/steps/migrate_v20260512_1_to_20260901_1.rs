use toml::Value;

/// Add global transition animation controls and timing parameters.
pub(crate) fn migrate_v20260512_1_to_20260901_1(value: &mut Value) {
  if let Value::Table(table) = value {
    let animation = table
      .entry("animation".to_string())
      .or_insert_with(|| Value::Table(Default::default()));

    if let Value::Table(animation) = animation {
      animation
        .entry("enabled".to_string())
        .or_insert_with(|| Value::Boolean(true));
      animation
        .entry("duration_ms".to_string())
        .or_insert_with(|| Value::Integer(180));
      animation
        .entry("frame_interval_ms".to_string())
        .or_insert_with(|| Value::Integer(15));
      animation
        .entry("easing".to_string())
        .or_insert_with(|| Value::String("ease_in_out".to_string()));
      animation
        .entry("fade_start_opacity".to_string())
        .or_insert_with(|| Value::Float(1.0));
    }

    table.insert(
      "version".to_string(),
      Value::String("20260901.1".to_string()),
    );
  }
}
