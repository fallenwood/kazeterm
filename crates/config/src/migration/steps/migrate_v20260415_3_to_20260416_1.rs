use toml::Value;

/// Add character-based tab label min/max widths to [tab].
pub(crate) fn migrate_v20260415_3_to_20260416_1(value: &mut Value) {
  if let Value::Table(table) = value {
    // No new keys to insert: label_min_chars and label_max_chars default to None (absent).
    // Existing label_min_width / label_max_width are preserved as the pixel fallback.
    table.insert(
      "version".to_string(),
      Value::String("20260416.1".to_string()),
    );
  }
}
