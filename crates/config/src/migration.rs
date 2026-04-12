use toml::Value;

/// Current config version in YYYYMMDD.Rev format.
pub const CURRENT_CONFIG_VERSION: &str = "20260412.3";

/// A migration that transforms raw TOML config from one version to the next.
struct Migration {
  from_version: &'static str,
  to_version: &'static str,
  migrate: fn(&mut Value),
}

/// Registry of all migrations, ordered from oldest to newest.
/// Each migration transforms the config from `from_version` to `to_version`.
/// To add a new migration:
/// 1. Add a new entry at the end of this list
/// 2. Set `from_version` to the previous `CURRENT_CONFIG_VERSION`
/// 3. Set `to_version` to the new version
/// 4. Update `CURRENT_CONFIG_VERSION` to the new version
/// 5. Implement the migration function that modifies the raw TOML `Value`
fn migrations() -> &'static [Migration] {
  &[
    Migration {
      from_version: "0",
      to_version: "20260208.1",
      migrate: migrate_v0_to_20260208_1,
    },
    Migration {
      from_version: "20260208.1",
      to_version: "20260220.1",
      migrate: migrate_v20260208_1_to_20260220_1,
    },
    Migration {
      from_version: "20260220.1",
      to_version: "20260303.1",
      migrate: migrate_v20260220_1_to_20260303_1,
    },
    Migration {
      from_version: "20260303.1",
      to_version: "20260306.1",
      migrate: migrate_v20260303_1_to_20260306_1,
    },
    Migration {
      from_version: "20260306.1",
      to_version: "20260322.1",
      migrate: migrate_v20260306_1_to_20260322_1,
    },
    Migration {
      from_version: "20260322.1",
      to_version: "20260323.1",
      migrate: migrate_v20260322_1_to_20260323_1,
    },
    Migration {
      from_version: "20260323.1",
      to_version: "20260323.2",
      migrate: migrate_v20260323_1_to_20260323_2,
    },
    Migration {
      from_version: "20260323.2",
      to_version: "20260327.1",
      migrate: migrate_v20260323_2_to_20260327_1,
    },
    Migration {
      from_version: "20260327.1",
      to_version: "20260407.1",
      migrate: migrate_v20260327_1_to_20260407_1,
    },
    Migration {
      from_version: "20260407.1",
      to_version: "20260411.1",
      migrate: migrate_v20260407_1_to_20260411_1,
    },
    Migration {
      from_version: "20260411.1",
      to_version: "20260411.2",
      migrate: migrate_v20260411_1_to_20260411_2,
    },
    Migration {
      from_version: "20260411.2",
      to_version: "20260411.3",
      migrate: migrate_v20260411_2_to_20260411_3,
    },
    Migration {
      from_version: "20260411.3",
      to_version: "20260412.1",
      migrate: migrate_v20260411_3_to_20260412_1,
    },
    Migration {
      from_version: "20260412.1",
      to_version: "20260412.2",
      migrate: migrate_v20260412_1_to_20260412_2,
    },
    Migration {
      from_version: "20260412.2",
      to_version: "20260412.3",
      migrate: migrate_v20260412_2_to_20260412_3,
    },
  ]
}

/// Migrate config with no version field to the first versioned format.
fn migrate_v0_to_20260208_1(value: &mut Value) {
  if let Value::Table(table) = value {
    table.insert(
      "version".to_string(),
      Value::String("20260208.1".to_string()),
    );
  }
}

/// Add vertical tab configuration support.
fn migrate_v20260208_1_to_20260220_1(value: &mut Value) {
  if let Value::Table(table) = value {
    if !table.contains_key("vertical_tabs") {
      table.insert("vertical_tabs".to_string(), Value::Boolean(false));
    }
    table.insert(
      "version".to_string(),
      Value::String("20260220.1".to_string()),
    );
  }
}

/// Add custom keybindings configuration support.
fn migrate_v20260220_1_to_20260303_1(value: &mut Value) {
  if let Value::Table(table) = value {
    table.insert(
      "version".to_string(),
      Value::String("20260303.1".to_string()),
    );
  }
}

/// Add background_opacity configuration support.
fn migrate_v20260303_1_to_20260306_1(value: &mut Value) {
  if let Value::Table(table) = value {
    if !table.contains_key("background_opacity") {
      table.insert("background_opacity".to_string(), Value::Float(1.0));
    }
    table.insert(
      "version".to_string(),
      Value::String("20260306.1".to_string()),
    );
  }
}

/// Add split pane navigation keybindings (focus_next_pane, focus_previous_pane, swap_split_panes).
fn migrate_v20260306_1_to_20260322_1(value: &mut Value) {
  if let Value::Table(table) = value {
    // New keybinding defaults are handled by serde defaults, no explicit insertion needed.
    table.insert(
      "version".to_string(),
      Value::String("20260322.1".to_string()),
    );
  }
}

/// Add terminal configuration options: scrollback, cursor, osc52, copy_on_select, env, working_directory.
fn migrate_v20260322_1_to_20260323_1(value: &mut Value) {
  if let Value::Table(table) = value {
    if !table.contains_key("scrollback_lines") {
      table.insert("scrollback_lines".to_string(), Value::Integer(10_000));
    }
    if !table.contains_key("cursor_shape") {
      table.insert(
        "cursor_shape".to_string(),
        Value::String("block".to_string()),
      );
    }
    if !table.contains_key("cursor_blink") {
      table.insert("cursor_blink".to_string(), Value::Boolean(true));
    }
    if !table.contains_key("cursor_blink_interval") {
      table.insert("cursor_blink_interval".to_string(), Value::Integer(750));
    }
    if !table.contains_key("osc52") {
      table.insert("osc52".to_string(), Value::String("copy_only".to_string()));
    }
    if !table.contains_key("copy_on_select") {
      table.insert("copy_on_select".to_string(), Value::Boolean(false));
    }
    table.insert(
      "version".to_string(),
      Value::String("20260323.1".to_string()),
    );
  }
}

/// Add toggle_tab_bar keybinding (serde defaults handle the new field).
fn migrate_v20260323_1_to_20260323_2(value: &mut Value) {
  if let Value::Table(table) = value {
    table.insert(
      "version".to_string(),
      Value::String("20260323.2".to_string()),
    );
  }
}

/// Add background_blur configuration support.
fn migrate_v20260323_2_to_20260327_1(value: &mut Value) {
  if let Value::Table(table) = value {
    if !table.contains_key("background_blur") {
      table.insert("background_blur".to_string(), Value::Boolean(false));
    }
    table.insert(
      "version".to_string(),
      Value::String("20260327.1".to_string()),
    );
  }
}

/// Add right_click_context_menu configuration.
fn migrate_v20260327_1_to_20260407_1(value: &mut Value) {
  if let Value::Table(table) = value {
    if !table.contains_key("right_click_context_menu") {
      table.insert(
        "right_click_context_menu".to_string(),
        Value::Boolean(false),
      );
    }
    // Remove old string-based right_click field if present
    table.remove("right_click");
    table.insert(
      "version".to_string(),
      Value::String("20260407.1".to_string()),
    );
  }
}

fn migrate_v20260407_1_to_20260411_1(value: &mut Value) {
  if let Value::Table(table) = value {
    // Add new_tab and new_tab_profile_N keybindings to existing keybindings section
    if let Some(Value::Table(kb)) = table.get_mut("keybindings") {
      if !kb.contains_key("new_tab") {
        kb.insert("new_tab".to_string(), Value::String("ctrl-shift-t".to_string()));
      }
      for i in 1..=9 {
        let key = format!("new_tab_profile_{}", i);
        if !kb.contains_key(&key) {
          kb.insert(key, Value::String(format!("ctrl-shift-{}", i)));
        }
      }
    }
    table.insert(
      "version".to_string(),
      Value::String("20260411.1".to_string()),
    );
  }
}

/// Add startup maximized window configuration support.
fn migrate_v20260411_1_to_20260411_2(value: &mut Value) {
  if let Value::Table(table) = value {
    if !table.contains_key("start_maximized") {
      table.insert("start_maximized".to_string(), Value::Boolean(false));
    }
    table.insert(
      "version".to_string(),
      Value::String("20260411.2".to_string()),
    );
  }
}

/// Add split pane divider width configuration support.
fn migrate_v20260411_2_to_20260411_3(value: &mut Value) {
  if let Value::Table(table) = value {
    if !table.contains_key("split_pane_divider_width") {
      table.insert("split_pane_divider_width".to_string(), Value::Float(6.0));
    }
    table.insert(
      "version".to_string(),
      Value::String("20260411.3".to_string()),
    );
  }
}

/// Add inactive_pane_opacity configuration for dimming unfocused split panes.
fn migrate_v20260411_3_to_20260412_1(value: &mut Value) {
  if let Value::Table(table) = value {
    if !table.contains_key("inactive_pane_opacity") {
      table.insert("inactive_pane_opacity".to_string(), Value::Float(0.6));
    }
    table.insert(
      "version".to_string(),
      Value::String("20260412.1".to_string()),
    );
  }
}

/// Add config overlay import support.
fn migrate_v20260412_1_to_20260412_2(value: &mut Value) {
  if let Value::Table(table) = value {
    if !table.contains_key("imports") {
      table.insert("imports".to_string(), Value::Array(Vec::new()));
    }
    table.insert(
      "version".to_string(),
      Value::String("20260412.2".to_string()),
    );
  }
}

/// Add configurable tab title debounce support.
fn migrate_v20260412_2_to_20260412_3(value: &mut Value) {
  if let Value::Table(table) = value {
    if !table.contains_key("tab_title_change_delay_ms") {
      table.insert(
        "tab_title_change_delay_ms".to_string(),
        Value::Integer(200),
      );
    }
    table.insert(
      "version".to_string(),
      Value::String("20260412.3".to_string()),
    );
  }
}

/// Apply all necessary migrations to bring the config up to `CURRENT_CONFIG_VERSION`.
/// Returns `true` if any migrations were applied, `false` if the config was already current.
pub fn apply_migrations(value: &mut Value) -> bool {
  let current_version = value
    .get("version")
    .and_then(|v| v.as_str())
    .unwrap_or("0")
    .to_string();

  if current_version == CURRENT_CONFIG_VERSION {
    return false;
  }

  let all_migrations = migrations();

  // Find the starting migration index
  let start_idx = match all_migrations
    .iter()
    .position(|m| m.from_version == current_version)
  {
    Some(idx) => idx,
    None => {
      tracing::warn!(
        "Unknown config version '{}', attempting to use as-is",
        current_version
      );
      return false;
    }
  };

  // Apply migrations in sequence
  for migration in &all_migrations[start_idx..] {
    tracing::info!(
      "Migrating config from {} to {}",
      migration.from_version,
      migration.to_version
    );
    (migration.migrate)(value);
  }

  true
}

#[cfg(test)]
mod tests {
  use super::*;

  fn make_v0_config() -> Value {
    toml::from_str(
      r#"
theme = "one"
font_size = 18.0
font_family = "Cascadia Code NF"
"#,
    )
    .unwrap()
  }

  fn make_current_config() -> Value {
    toml::from_str(&format!(
      r#"
version = "{}"
theme = "one"
font_size = 18.0
"#,
      CURRENT_CONFIG_VERSION
    ))
    .unwrap()
  }

  fn make_20260208_config() -> Value {
    toml::from_str(
      r#"
version = "20260208.1"
theme = "one"
font_size = 18.0
"#,
    )
    .unwrap()
  }

  fn make_20260220_config() -> Value {
    toml::from_str(
      r#"
version = "20260220.1"
theme = "one"
font_size = 18.0
vertical_tabs = false
"#,
    )
    .unwrap()
  }

  #[test]
  fn no_migration_needed_for_current_version() {
    let mut config = make_current_config();
    let migrated = apply_migrations(&mut config);
    assert!(!migrated);
    assert_eq!(
      config.get("version").unwrap().as_str().unwrap(),
      CURRENT_CONFIG_VERSION
    );
  }

  #[test]
  fn migrate_from_v0_adds_version() {
    let mut config = make_v0_config();
    assert!(config.get("version").is_none());

    let migrated = apply_migrations(&mut config);
    assert!(migrated);
    assert_eq!(
      config.get("version").unwrap().as_str().unwrap(),
      CURRENT_CONFIG_VERSION
    );
    // Original fields are preserved
    assert_eq!(config.get("theme").unwrap().as_str().unwrap(), "one");
    assert_eq!(config.get("font_size").unwrap().as_float().unwrap(), 18.0);
  }

  #[test]
  fn migrate_20260208_adds_vertical_tabs() {
    let mut config = make_20260208_config();
    let migrated = apply_migrations(&mut config);
    assert!(migrated);
    assert_eq!(
      config.get("version").unwrap().as_str().unwrap(),
      CURRENT_CONFIG_VERSION
    );
    assert_eq!(
      config.get("vertical_tabs").unwrap().as_bool().unwrap(),
      false
    );
  }

  #[test]
  fn migrate_20260220_bumps_version_for_keybindings() {
    let mut config = make_20260220_config();
    let migrated = apply_migrations(&mut config);
    assert!(migrated);
    assert_eq!(
      config.get("version").unwrap().as_str().unwrap(),
      CURRENT_CONFIG_VERSION
    );
  }

  #[test]
  fn migrate_20260303_adds_background_opacity() {
    let mut config: Value = toml::from_str(
      r#"
version = "20260303.1"
theme = "one"
font_size = 18.0
"#,
    )
    .unwrap();
    let migrated = apply_migrations(&mut config);
    assert!(migrated);
    assert_eq!(
      config.get("version").unwrap().as_str().unwrap(),
      CURRENT_CONFIG_VERSION
    );
    assert_eq!(
      config
        .get("background_opacity")
        .unwrap()
        .as_float()
        .unwrap(),
      1.0
    );
  }

  #[test]
  fn migrate_20260306_adds_terminal_options() {
    let mut config: Value = toml::from_str(
      r#"
version = "20260306.1"
theme = "one"
font_size = 18.0
background_opacity = 0.9
"#,
    )
    .unwrap();
    let migrated = apply_migrations(&mut config);
    assert!(migrated);
    assert_eq!(
      config.get("version").unwrap().as_str().unwrap(),
      CURRENT_CONFIG_VERSION
    );
    assert_eq!(
      config
        .get("scrollback_lines")
        .unwrap()
        .as_integer()
        .unwrap(),
      10_000
    );
    assert_eq!(
      config.get("cursor_shape").unwrap().as_str().unwrap(),
      "block"
    );
    assert_eq!(config.get("cursor_blink").unwrap().as_bool().unwrap(), true);
    assert_eq!(
      config
        .get("cursor_blink_interval")
        .unwrap()
        .as_integer()
        .unwrap(),
      750
    );
    assert_eq!(config.get("osc52").unwrap().as_str().unwrap(), "copy_only");
    assert_eq!(
      config.get("copy_on_select").unwrap().as_bool().unwrap(),
      false
    );
  }

  #[test]
  fn unknown_version_is_not_migrated() {
    let mut config: Value = toml::from_str(
      r#"
version = "99999999.1"
theme = "one"
"#,
    )
    .unwrap();
    let migrated = apply_migrations(&mut config);
    assert!(!migrated);
    assert_eq!(
      config.get("version").unwrap().as_str().unwrap(),
      "99999999.1"
    );
  }

  #[test]
  fn migrate_20260323_2_adds_background_blur() {
    let mut config: Value = toml::from_str(
      r##"
version = "20260323.2"
theme = "one"
font_size = 18.0
background_opacity = 0.8
"##,
    )
    .unwrap();
    let migrated = apply_migrations(&mut config);
    assert!(migrated);
    assert_eq!(
      config.get("version").unwrap().as_str().unwrap(),
      CURRENT_CONFIG_VERSION
    );
    assert_eq!(
      config.get("background_blur").unwrap().as_bool().unwrap(),
      false
    );
  }

  #[test]
  fn migrate_20260327_1_adds_right_click() {
    let mut config: Value = toml::from_str(
      r##"
version = "20260327.1"
theme = "one"
font_size = 18.0
background_blur = false
"##,
    )
    .unwrap();
    let migrated = apply_migrations(&mut config);
    assert!(migrated);
    assert_eq!(
      config.get("version").unwrap().as_str().unwrap(),
      CURRENT_CONFIG_VERSION
    );
    assert_eq!(
      config
        .get("right_click_context_menu")
        .unwrap()
        .as_bool()
        .unwrap(),
      false
    );
  }

  #[test]
  fn migrate_20260412_2_adds_tab_title_change_delay() {
    let mut config: Value = toml::from_str(
      r#"
version = "20260412.2"
theme = "one"
font_size = 18.0
imports = []
"#,
    )
    .unwrap();
    let migrated = apply_migrations(&mut config);
    assert!(migrated);
    assert_eq!(
      config.get("version").unwrap().as_str().unwrap(),
      CURRENT_CONFIG_VERSION
    );
    assert_eq!(
      config
        .get("tab_title_change_delay_ms")
        .unwrap()
        .as_integer()
        .unwrap(),
      200
    );
  }

  #[test]
  fn chained_migrations_apply_in_order() {
    // Simulate a multi-step migration scenario by testing
    // that v0 config passes through the full chain
    let mut config = make_v0_config();
    let migrated = apply_migrations(&mut config);
    assert!(migrated);
    assert_eq!(
      config.get("version").unwrap().as_str().unwrap(),
      CURRENT_CONFIG_VERSION
    );
  }

  #[test]
  fn migrate_20260411_1_adds_start_maximized() {
    let mut config: Value = toml::from_str(
      r#"
version = "20260411.1"
theme = "one"
font_size = 18.0
"#,
    )
    .unwrap();

    let migrated = apply_migrations(&mut config);
    assert!(migrated);
    assert_eq!(
      config.get("version").unwrap().as_str().unwrap(),
      CURRENT_CONFIG_VERSION
    );
    assert_eq!(
      config.get("start_maximized").unwrap().as_bool().unwrap(),
      false
    );
  }

  #[test]
  fn migrate_20260411_2_adds_split_pane_divider_width() {
    let mut config: Value = toml::from_str(
      r#"
version = "20260411.2"
theme = "one"
font_size = 18.0
start_maximized = false
"#,
    )
    .unwrap();

    let migrated = apply_migrations(&mut config);
    assert!(migrated);
    assert_eq!(
      config.get("version").unwrap().as_str().unwrap(),
      CURRENT_CONFIG_VERSION
    );
    assert_eq!(
      config
        .get("split_pane_divider_width")
        .unwrap()
        .as_float()
        .unwrap(),
      6.0
    );
  }

  #[test]
  fn migrated_config_deserializes_to_config_struct() {
    let mut raw = make_v0_config();
    apply_migrations(&mut raw);
    let config: crate::Config = raw.try_into().unwrap();
    assert_eq!(config.version, CURRENT_CONFIG_VERSION);
    assert_eq!(config.theme, "one");
    assert_eq!(config.font_size, 18.0);
    assert_eq!(config.split_pane_divider_width, 6.0);
    assert!(!config.start_maximized);
    assert!((config.inactive_pane_opacity - 0.6).abs() < 0.001);
  }

  #[test]
  fn migrate_20260411_3_adds_inactive_pane_opacity() {
    let mut config: Value = toml::from_str(
      r#"
version = "20260411.3"
theme = "one"
font_size = 18.0
split_pane_divider_width = 6.0
"#,
    )
    .unwrap();

    let migrated = apply_migrations(&mut config);
    assert!(migrated);
    assert_eq!(
      config.get("version").unwrap().as_str().unwrap(),
      CURRENT_CONFIG_VERSION
    );
    assert_eq!(
      config
        .get("inactive_pane_opacity")
        .unwrap()
        .as_float()
        .unwrap(),
      0.6
    );
  }

  #[test]
  fn migrate_20260412_1_adds_imports() {
    let mut config: Value = toml::from_str(
      r#"
version = "20260412.1"
theme = "one"
font_size = 18.0
inactive_pane_opacity = 0.6
"#,
    )
    .unwrap();

    let migrated = apply_migrations(&mut config);
    assert!(migrated);
    assert_eq!(
      config.get("version").unwrap().as_str().unwrap(),
      CURRENT_CONFIG_VERSION
    );
    assert!(config.get("imports").unwrap().as_array().unwrap().is_empty());
  }
}
