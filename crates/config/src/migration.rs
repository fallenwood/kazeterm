use toml::Value;

/// Current config version in YYYYMMDD.Rev format.
pub const CURRENT_CONFIG_VERSION: &str = "20260321.1";

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
      to_version: "20260321.1",
      migrate: migrate_v20260306_1_to_20260321_1,
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

/// Add terminal configuration options: scrollback, cursor, osc52, copy_on_select, env, working_directory.
fn migrate_v20260306_1_to_20260321_1(value: &mut Value) {
  if let Value::Table(table) = value {
    if !table.contains_key("scrollback_lines") {
      table.insert(
        "scrollback_lines".to_string(),
        Value::Integer(10_000),
      );
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
      table.insert(
        "cursor_blink_interval".to_string(),
        Value::Integer(750),
      );
    }
    if !table.contains_key("osc52") {
      table.insert(
        "osc52".to_string(),
        Value::String("copy_only".to_string()),
      );
    }
    if !table.contains_key("copy_on_select") {
      table.insert("copy_on_select".to_string(), Value::Boolean(false));
    }
    table.insert(
      "version".to_string(),
      Value::String("20260321.1".to_string()),
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
      config.get("scrollback_lines").unwrap().as_integer().unwrap(),
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
  fn migrated_config_deserializes_to_config_struct() {
    let mut raw = make_v0_config();
    apply_migrations(&mut raw);
    let config: crate::Config = raw.try_into().unwrap();
    assert_eq!(config.version, CURRENT_CONFIG_VERSION);
    assert_eq!(config.theme, "one");
    assert_eq!(config.font_size, 18.0);
  }
}
