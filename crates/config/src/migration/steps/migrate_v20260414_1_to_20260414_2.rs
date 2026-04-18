use toml::Value;

/// Move theme/theme_mode from [appearance] to [colors], and bold_as_bright/minimum_contrast from [terminal] to [colors].
pub(crate) fn migrate_v20260414_1_to_20260414_2(value: &mut Value) {
  if let Value::Table(table) = value {
    // Move theme and theme_mode from appearance to colors
    if let Some(Value::Table(appearance)) = table.get_mut("appearance") {
      let theme = appearance.remove("theme");
      let theme_mode = appearance.remove("theme_mode");

      let colors = table
        .entry("colors")
        .or_insert_with(|| Value::Table(toml::map::Map::new()));
      if let Value::Table(colors_table) = colors {
        if let Some(v) = theme {
          colors_table.insert("theme".to_string(), v);
        }
        if let Some(v) = theme_mode {
          colors_table.insert("theme_mode".to_string(), v);
        }
      }
    }

    // Move bold_as_bright and minimum_contrast from terminal to colors
    if let Some(Value::Table(terminal)) = table.get_mut("terminal") {
      let bold = terminal.remove("bold_as_bright");
      let contrast = terminal.remove("minimum_contrast");

      if bold.is_some() || contrast.is_some() {
        let colors = table
          .entry("colors")
          .or_insert_with(|| Value::Table(toml::map::Map::new()));
        if let Value::Table(colors_table) = colors {
          if let Some(v) = bold {
            colors_table.insert("bold_as_bright".to_string(), v);
          }
          if let Some(v) = contrast {
            colors_table.insert("minimum_contrast".to_string(), v);
          }
        }
      }
    }

    table.insert(
      "version".to_string(),
      Value::String("20260414.2".to_string()),
    );
  }
}
