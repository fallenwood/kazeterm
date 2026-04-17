use toml::Value;

/// Current config version in YYYYMMDD.Rev format.
pub const CURRENT_CONFIG_VERSION: &str = "20260417.2";

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
    Migration {
      from_version: "20260412.3",
      to_version: "20260414.1",
      migrate: migrate_v20260412_3_to_20260414_1,
    },
    Migration {
      from_version: "20260414.1",
      to_version: "20260414.2",
      migrate: migrate_v20260414_1_to_20260414_2,
    },
    Migration {
      from_version: "20260414.2",
      to_version: "20260415.1",
      migrate: migrate_v20260414_2_to_20260415_1,
    },
    Migration {
      from_version: "20260415.1",
      to_version: "20260415.2",
      migrate: migrate_v20260415_1_to_20260415_2,
    },
    Migration {
      from_version: "20260415.2",
      to_version: "20260415.3",
      migrate: migrate_v20260415_2_to_20260415_3,
    },
    Migration {
      from_version: "20260415.3",
      to_version: "20260416.1",
      migrate: migrate_v20260415_3_to_20260416_1,
    },
    Migration {
      from_version: "20260416.1",
      to_version: "20260416.2",
      migrate: migrate_v20260416_1_to_20260416_2,
    },
    Migration {
      from_version: "20260416.2",
      to_version: "20260416.3",
      migrate: migrate_v20260416_2_to_20260416_3,
    },
    Migration {
      from_version: "20260416.3",
      to_version: "20260417.1",
      migrate: migrate_v20260416_3_to_20260417_1,
    },
    Migration {
      from_version: "20260417.1",
      to_version: "20260417.2",
      migrate: migrate_v20260417_1_to_20260417_2,
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
      let defaults = crate::KeybindingConfig::default();
      let default_profile_bindings = [
        &defaults.new_tab_profile_1,
        &defaults.new_tab_profile_2,
        &defaults.new_tab_profile_3,
        &defaults.new_tab_profile_4,
        &defaults.new_tab_profile_5,
        &defaults.new_tab_profile_6,
        &defaults.new_tab_profile_7,
        &defaults.new_tab_profile_8,
        &defaults.new_tab_profile_9,
      ];

      if !kb.contains_key("new_tab") {
        kb.insert(
          "new_tab".to_string(),
          Value::String(defaults.new_tab.first().unwrap().to_string()),
        );
      }
      for (i, binding) in default_profile_bindings.iter().enumerate() {
        let key = format!("new_tab_profile_{}", i + 1);
        if !kb.contains_key(&key) {
          kb.insert(key, Value::String(binding.first().unwrap().to_string()));
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
      table.insert("tab_title_change_delay_ms".to_string(), Value::Integer(200));
    }
    table.insert(
      "version".to_string(),
      Value::String("20260412.3".to_string()),
    );
  }
}

/// Restructure flat config into nested TOML tables (appearance, font, window, etc.).
fn migrate_v20260412_3_to_20260414_1(value: &mut Value) {
  if let Value::Table(table) = value {
    // Helper: move a key from root into a sub-table, creating the sub-table if needed.
    fn move_key(table: &mut toml::map::Map<String, Value>, key: &str, section: &str) {
      if let Some(val) = table.remove(key) {
        let sub = table
          .entry(section)
          .or_insert_with(|| Value::Table(toml::map::Map::new()));
        if let Value::Table(sub_table) = sub {
          sub_table.insert(key.to_string(), val);
        }
      }
    }

    // Helper: move a key from root into a sub-table under a different name.
    fn move_key_rename(
      table: &mut toml::map::Map<String, Value>,
      old_key: &str,
      new_key: &str,
      section: &str,
    ) {
      if let Some(val) = table.remove(old_key) {
        let sub = table
          .entry(section)
          .or_insert_with(|| Value::Table(toml::map::Map::new()));
        if let Value::Table(sub_table) = sub {
          sub_table.insert(new_key.to_string(), val);
        }
      }
    }

    // [appearance]
    move_key(table, "theme", "appearance");
    move_key(table, "theme_mode", "appearance");
    move_key(table, "themes_path", "appearance");
    move_key(table, "background_opacity", "appearance");
    move_key(table, "background_blur", "appearance");

    // [font]
    move_key_rename(table, "font_size", "size", "font");
    move_key_rename(table, "font_family", "family", "font");
    move_key_rename(table, "ui_font_family", "ui_family", "font");
    move_key_rename(table, "ui_font_size", "ui_size", "font");

    // [window]
    move_key_rename(table, "window_width", "width", "window");
    move_key_rename(table, "window_height", "height", "window");
    move_key(table, "start_maximized", "window");
    move_key(table, "restore_workspace", "window");

    // [tab]
    move_key_rename(table, "vertical_tabs", "vertical", "tab");
    move_key_rename(table, "close_on_last_tab", "close_on_last", "tab");
    move_key_rename(table, "tab_switcher_popup", "switcher_popup", "tab");
    move_key_rename(
      table,
      "tab_title_change_delay_ms",
      "title_change_delay_ms",
      "tab",
    );

    // [pane]
    move_key_rename(table, "split_pane_divider_width", "divider_width", "pane");
    move_key_rename(table, "inactive_pane_opacity", "inactive_opacity", "pane");

    // [terminal]
    move_key(table, "scrollback_lines", "terminal");
    move_key(table, "osc52", "terminal");
    move_key(table, "copy_on_select", "terminal");
    move_key(table, "right_click_context_menu", "terminal");
    move_key(table, "minimap_enabled", "terminal");
    move_key(table, "working_directory", "terminal");
    move_key(table, "default_profile", "terminal");
    move_key(table, "env", "terminal");

    // [cursor]
    move_key_rename(table, "cursor_shape", "shape", "cursor");
    move_key_rename(table, "cursor_blink", "blink", "cursor");
    move_key_rename(table, "cursor_blink_interval", "blink_interval", "cursor");

    // [notification]
    move_key(table, "long_running_threshold_secs", "notification");
    move_key_rename(
      table,
      "notification_interval_secs",
      "interval_secs",
      "notification",
    );

    table.insert(
      "version".to_string(),
      Value::String("20260414.1".to_string()),
    );
  }
}

/// Move theme/theme_mode from [appearance] to [colors], and bold_as_bright/minimum_contrast from [terminal] to [colors].
fn migrate_v20260414_1_to_20260414_2(value: &mut Value) {
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

/// Add ctrl_scroll_zoom configuration to [terminal].
fn migrate_v20260414_2_to_20260415_1(value: &mut Value) {
  if let Value::Table(table) = value {
    let terminal = table
      .entry("terminal")
      .or_insert_with(|| Value::Table(toml::map::Map::new()));
    if let Value::Table(terminal_table) = terminal {
      if !terminal_table.contains_key("ctrl_scroll_zoom") {
        terminal_table.insert("ctrl_scroll_zoom".to_string(), Value::Boolean(true));
      }
    }

    table.insert(
      "version".to_string(),
      Value::String("20260415.1".to_string()),
    );
  }
}

/// Add configurable tab label min/max widths to [tab].
fn migrate_v20260415_1_to_20260415_2(value: &mut Value) {
  if let Value::Table(table) = value {
    let tab = table
      .entry("tab")
      .or_insert_with(|| Value::Table(toml::map::Map::new()));
    if let Value::Table(tab_table) = tab {
      if !tab_table.contains_key("label_min_width") {
        tab_table.insert("label_min_width".to_string(), Value::Float(60.0));
      }
      if !tab_table.contains_key("label_max_width") {
        tab_table.insert("label_max_width".to_string(), Value::Float(200.0));
      }
    }

    table.insert(
      "version".to_string(),
      Value::String("20260415.2".to_string()),
    );
  }
}

/// Add hide_mouse_when_typing configuration to [terminal].
fn migrate_v20260415_2_to_20260415_3(value: &mut Value) {
  if let Value::Table(table) = value {
    let terminal = table
      .entry("terminal")
      .or_insert_with(|| Value::Table(toml::map::Map::new()));
    if let Value::Table(terminal_table) = terminal {
      if !terminal_table.contains_key("hide_mouse_when_typing") {
        terminal_table.insert("hide_mouse_when_typing".to_string(), Value::Boolean(false));
      }
    }

    table.insert(
      "version".to_string(),
      Value::String("20260415.3".to_string()),
    );
  }
}

/// Add character-based tab label min/max widths to [tab].
fn migrate_v20260415_3_to_20260416_1(value: &mut Value) {
  if let Value::Table(table) = value {
    // No new keys to insert: label_min_chars and label_max_chars default to None (absent).
    // Existing label_min_width / label_max_width are preserved as the pixel fallback.
    table.insert(
      "version".to_string(),
      Value::String("20260416.1".to_string()),
    );
  }
}

/// Fix macOS legacy tab shortcuts that were inserted with non-macOS defaults.
fn migrate_v20260416_1_to_20260416_2(value: &mut Value) {
  if let Value::Table(table) = value {
    if cfg!(target_os = "macos") {
      if let Some(Value::Table(kb)) = table.get_mut("keybindings") {
        let defaults = crate::KeybindingConfig::default();
        let default_profile_bindings = [
          &defaults.new_tab_profile_1,
          &defaults.new_tab_profile_2,
          &defaults.new_tab_profile_3,
          &defaults.new_tab_profile_4,
          &defaults.new_tab_profile_5,
          &defaults.new_tab_profile_6,
          &defaults.new_tab_profile_7,
          &defaults.new_tab_profile_8,
          &defaults.new_tab_profile_9,
        ];

        if matches!(kb.get("new_tab"), Some(Value::String(binding)) if binding == "ctrl-shift-t") {
          kb.insert(
            "new_tab".to_string(),
            Value::String(defaults.new_tab.first().unwrap().to_string()),
          );
        }

        for (i, default_binding) in default_profile_bindings.iter().enumerate() {
          let key = format!("new_tab_profile_{}", i + 1);
          let legacy_binding = format!("ctrl-shift-{}", i + 1);
          if matches!(kb.get(&key), Some(Value::String(binding)) if binding == &legacy_binding) {
            kb.insert(
              key,
              Value::String(default_binding.first().unwrap().to_string()),
            );
          }
        }
      }
    }

    table.insert(
      "version".to_string(),
      Value::String("20260416.2".to_string()),
    );
  }
}

/// Add direct tab selection keybindings (select_tab_1..9) to existing keybinding sections.
fn migrate_v20260416_2_to_20260416_3(value: &mut Value) {
  if let Value::Table(table) = value {
    if let Some(Value::Table(kb)) = table.get_mut("keybindings") {
      let defaults = crate::KeybindingConfig::default();
      let select_tab_bindings = [
        &defaults.select_tab_1,
        &defaults.select_tab_2,
        &defaults.select_tab_3,
        &defaults.select_tab_4,
        &defaults.select_tab_5,
        &defaults.select_tab_6,
        &defaults.select_tab_7,
        &defaults.select_tab_8,
        &defaults.select_tab_9,
      ];

      for (i, binding) in select_tab_bindings.iter().enumerate() {
        let key = format!("select_tab_{}", i + 1);
        if !kb.contains_key(&key) {
          kb.insert(key, Value::String(binding.first().unwrap().to_string()));
        }
      }
    }

    table.insert(
      "version".to_string(),
      Value::String("20260416.3".to_string()),
    );
  }
}

/// Add directional split-pane focus keybindings to existing keybinding sections.
fn migrate_v20260416_3_to_20260417_1(value: &mut Value) {
  if let Value::Table(table) = value {
    if let Some(Value::Table(kb)) = table.get_mut("keybindings") {
      let defaults = crate::KeybindingConfig::default();
      let directional_bindings = [
        ("focus_pane_up", &defaults.focus_pane_up),
        ("focus_pane_down", &defaults.focus_pane_down),
        ("focus_pane_left", &defaults.focus_pane_left),
        ("focus_pane_right", &defaults.focus_pane_right),
      ];

      for (key, binding) in directional_bindings {
        if !kb.contains_key(key) {
          kb.insert(
            key.to_string(),
            Value::String(binding.first().unwrap().to_string()),
          );
        }
      }
    }

    table.insert(
      "version".to_string(),
      Value::String("20260417.1".to_string()),
    );
  }
}

/// Add terminal hover-to-focus configuration.
fn migrate_v20260417_1_to_20260417_2(value: &mut Value) {
  if let Value::Table(table) = value {
    let terminal = table
      .entry("terminal")
      .or_insert_with(|| Value::Table(toml::map::Map::new()));
    if let Value::Table(terminal_table) = terminal
      && !terminal_table.contains_key("focus_terminal_on_hover")
    {
      terminal_table.insert("focus_terminal_on_hover".to_string(), Value::Boolean(true));
    }

    table.insert(
      "version".to_string(),
      Value::String("20260417.2".to_string()),
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

[colors]
theme = "one"

[font]
size = 18.0
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

  /// Helper to access a nested value by section.key path.
  fn get_nested<'a>(config: &'a Value, section: &str, key: &str) -> Option<&'a Value> {
    config.get(section).and_then(|s| s.get(key))
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
    // Original fields are migrated to nested tables
    assert_eq!(
      get_nested(&config, "colors", "theme")
        .unwrap()
        .as_str()
        .unwrap(),
      "one"
    );
    assert_eq!(
      get_nested(&config, "font", "size")
        .unwrap()
        .as_float()
        .unwrap(),
      18.0
    );
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
      get_nested(&config, "tab", "vertical")
        .unwrap()
        .as_bool()
        .unwrap(),
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
      get_nested(&config, "appearance", "background_opacity")
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
      get_nested(&config, "terminal", "scrollback_lines")
        .unwrap()
        .as_integer()
        .unwrap(),
      10_000
    );
    assert_eq!(
      get_nested(&config, "cursor", "shape")
        .unwrap()
        .as_str()
        .unwrap(),
      "block"
    );
    assert_eq!(
      get_nested(&config, "cursor", "blink")
        .unwrap()
        .as_bool()
        .unwrap(),
      true
    );
    assert_eq!(
      get_nested(&config, "cursor", "blink_interval")
        .unwrap()
        .as_integer()
        .unwrap(),
      750
    );
    assert_eq!(
      get_nested(&config, "terminal", "osc52")
        .unwrap()
        .as_str()
        .unwrap(),
      "copy_only"
    );
    assert_eq!(
      get_nested(&config, "terminal", "copy_on_select")
        .unwrap()
        .as_bool()
        .unwrap(),
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
      get_nested(&config, "appearance", "background_blur")
        .unwrap()
        .as_bool()
        .unwrap(),
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
      get_nested(&config, "terminal", "right_click_context_menu")
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
      get_nested(&config, "tab", "title_change_delay_ms")
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
      get_nested(&config, "window", "start_maximized")
        .unwrap()
        .as_bool()
        .unwrap(),
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
      get_nested(&config, "pane", "divider_width")
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
    assert_eq!(config.colors.theme, "one");
    assert_eq!(config.font.size, 18.0);
    assert_eq!(config.pane.divider_width, 6.0);
    assert!(!config.window.start_maximized);
    assert!((config.pane.inactive_opacity - 0.6).abs() < 0.001);
    assert_eq!(config.tab.label_min_width, 60.0);
    assert_eq!(config.tab.label_max_width, 200.0);
    assert!(!config.terminal.hide_mouse_when_typing);
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
      get_nested(&config, "pane", "inactive_opacity")
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
    assert!(
      config
        .get("imports")
        .unwrap()
        .as_array()
        .unwrap()
        .is_empty()
    );
  }

  #[test]
  fn migrate_20260415_1_adds_tab_label_widths() {
    let mut config: Value = toml::from_str(
      r#"
version = "20260415.1"

[tab]
vertical = false
close_on_last = true
switcher_popup = true
title_change_delay_ms = 200
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
      get_nested(&config, "tab", "label_min_width")
        .unwrap()
        .as_float()
        .unwrap(),
      60.0
    );
    assert_eq!(
      get_nested(&config, "tab", "label_max_width")
        .unwrap()
        .as_float()
        .unwrap(),
      200.0
    );
  }

  #[test]
  fn migrate_20260415_2_adds_hide_mouse_when_typing() {
    let mut config: Value = toml::from_str(
      r#"
version = "20260415.2"

[terminal]
scrollback_lines = 10000
osc52 = "copy_only"
copy_on_select = false
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
      get_nested(&config, "terminal", "hide_mouse_when_typing")
        .unwrap()
        .as_bool()
        .unwrap(),
      false
    );
  }

  #[test]
  fn migrate_20260407_1_adds_new_tab_keybindings_using_platform_defaults() {
    let mut config: Value = toml::from_str(
      r#"
version = "20260407.1"

[keybindings]
copy = "ctrl-shift-c"
"#,
    )
    .unwrap();

    let migrated = apply_migrations(&mut config);
    assert!(migrated);
    assert_eq!(
      get_nested(&config, "keybindings", "new_tab")
        .unwrap()
        .as_str()
        .unwrap(),
      crate::KeybindingConfig::default().new_tab.first().unwrap()
    );
    assert_eq!(
      get_nested(&config, "keybindings", "new_tab_profile_1")
        .unwrap()
        .as_str()
        .unwrap(),
      crate::KeybindingConfig::default()
        .new_tab_profile_1
        .first()
        .unwrap()
    );
    assert_eq!(
      config.get("version").unwrap().as_str().unwrap(),
      CURRENT_CONFIG_VERSION
    );
  }

  #[test]
  fn migrate_20260416_1_repairs_legacy_tab_shortcuts_only_on_macos() {
    let mut config: Value = toml::from_str(
      r#"
version = "20260416.1"

[keybindings]
new_tab = "ctrl-shift-t"
new_tab_profile_1 = "ctrl-shift-1"
new_tab_profile_9 = "ctrl-shift-9"
"#,
    )
    .unwrap();

    let migrated = apply_migrations(&mut config);
    assert!(migrated);

    let default_keybindings = crate::KeybindingConfig::default();
    let expected_new_tab = if cfg!(target_os = "macos") {
      default_keybindings.new_tab.first().unwrap()
    } else {
      "ctrl-shift-t"
    };
    let expected_profile_1 = if cfg!(target_os = "macos") {
      default_keybindings.new_tab_profile_1.first().unwrap()
    } else {
      "ctrl-shift-1"
    };
    let expected_profile_9 = if cfg!(target_os = "macos") {
      default_keybindings.new_tab_profile_9.first().unwrap()
    } else {
      "ctrl-shift-9"
    };

    assert_eq!(
      get_nested(&config, "keybindings", "new_tab")
        .unwrap()
        .as_str()
        .unwrap(),
      expected_new_tab
    );
    assert_eq!(
      get_nested(&config, "keybindings", "new_tab_profile_1")
        .unwrap()
        .as_str()
        .unwrap(),
      expected_profile_1
    );
    assert_eq!(
      get_nested(&config, "keybindings", "new_tab_profile_9")
        .unwrap()
        .as_str()
        .unwrap(),
      expected_profile_9
    );
    assert_eq!(
      config.get("version").unwrap().as_str().unwrap(),
      CURRENT_CONFIG_VERSION
    );
  }

  #[test]
  fn migrate_20260416_2_adds_select_tab_keybindings_using_platform_defaults() {
    let mut config: Value = toml::from_str(
      r#"
version = "20260416.2"

[keybindings]
copy = "ctrl-shift-c"
"#,
    )
    .unwrap();

    let migrated = apply_migrations(&mut config);
    assert!(migrated);

    let default_keybindings = crate::KeybindingConfig::default();
    assert_eq!(
      get_nested(&config, "keybindings", "select_tab_1")
        .unwrap()
        .as_str()
        .unwrap(),
      default_keybindings.select_tab_1.first().unwrap()
    );
    assert_eq!(
      get_nested(&config, "keybindings", "select_tab_9")
        .unwrap()
        .as_str()
        .unwrap(),
      default_keybindings.select_tab_9.first().unwrap()
    );
    assert_eq!(
      config.get("version").unwrap().as_str().unwrap(),
      CURRENT_CONFIG_VERSION
    );
  }

  #[test]
  fn migrate_20260416_3_adds_directional_pane_focus_keybindings_using_platform_defaults() {
    let mut config: Value = toml::from_str(
      r#"
version = "20260416.3"

[keybindings]
copy = "ctrl-shift-c"
"#,
    )
    .unwrap();

    let migrated = apply_migrations(&mut config);
    assert!(migrated);

    let default_keybindings = crate::KeybindingConfig::default();
    assert_eq!(
      get_nested(&config, "keybindings", "focus_pane_up")
        .unwrap()
        .as_str()
        .unwrap(),
      default_keybindings.focus_pane_up.first().unwrap()
    );
    assert_eq!(
      get_nested(&config, "keybindings", "focus_pane_right")
        .unwrap()
        .as_str()
        .unwrap(),
      default_keybindings.focus_pane_right.first().unwrap()
    );
    assert_eq!(
      config.get("version").unwrap().as_str().unwrap(),
      CURRENT_CONFIG_VERSION
    );
  }

  #[test]
  fn migrate_20260417_1_adds_focus_terminal_on_hover() {
    let mut config: Value = toml::from_str(
      r#"
version = "20260417.1"

[terminal]
scrollback_lines = 10000
"#,
    )
    .unwrap();

    let migrated = apply_migrations(&mut config);
    assert!(migrated);
    assert_eq!(
      get_nested(&config, "terminal", "focus_terminal_on_hover")
        .unwrap()
        .as_bool()
        .unwrap(),
      true
    );
    assert_eq!(
      config.get("version").unwrap().as_str().unwrap(),
      CURRENT_CONFIG_VERSION
    );
  }
}
