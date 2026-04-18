use toml::Value;

/// Add toggle_tab_bar keybinding (serde defaults handle the new field).
pub(crate) fn migrate_v20260323_1_to_20260323_2(value: &mut Value) {
  if let Value::Table(table) = value {
    table.insert(
      "version".to_string(),
      Value::String("20260323.2".to_string()),
    );
  }
}
