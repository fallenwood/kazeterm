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
  assert_eq!(config.terminal.kernel, crate::TerminalKernel::Alacritty);
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
  let default_keybindings = crate::KeybindingConfig::default();
  let expected_new_tab = default_keybindings.new_tab.first().unwrap();
  let expected_new_tab_profile_1 = default_keybindings.new_tab_profile_1.first().unwrap();
  assert_eq!(
    get_nested(&config, "keybindings", expected_new_tab)
      .unwrap()
      .as_str()
      .unwrap(),
    "new_tab"
  );
  assert_eq!(
    get_nested(&config, "keybindings", expected_new_tab_profile_1)
      .unwrap()
      .as_str()
      .unwrap(),
    "new_tab_profile_1"
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
    get_nested(&config, "keybindings", expected_new_tab)
      .unwrap()
      .as_str()
      .unwrap(),
    "new_tab"
  );
  assert_eq!(
    get_nested(&config, "keybindings", expected_profile_1)
      .unwrap()
      .as_str()
      .unwrap(),
    "new_tab_profile_1"
  );
  assert_eq!(
    get_nested(&config, "keybindings", expected_profile_9)
      .unwrap()
      .as_str()
      .unwrap(),
    "new_tab_profile_9"
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
    get_nested(
      &config,
      "keybindings",
      default_keybindings.select_tab_1.first().unwrap()
    )
      .unwrap()
      .as_str()
      .unwrap(),
    "select_tab_1"
  );
  assert_eq!(
    get_nested(
      &config,
      "keybindings",
      default_keybindings.select_tab_9.first().unwrap()
    )
      .unwrap()
      .as_str()
      .unwrap(),
    "select_tab_9"
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
    get_nested(
      &config,
      "keybindings",
      default_keybindings.focus_pane_up.first().unwrap()
    )
      .unwrap()
      .as_str()
      .unwrap(),
    "focus_pane_up"
  );
  assert_eq!(
    get_nested(
      &config,
      "keybindings",
      default_keybindings.focus_pane_right.first().unwrap()
    )
      .unwrap()
      .as_str()
      .unwrap(),
    "focus_pane_right"
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

#[test]
fn migrate_20260417_2_adds_terminal_kernel() {
  let mut config: Value = toml::from_str(
    r#"
version = "20260417.2"

[terminal]
scrollback_lines = 10000
focus_terminal_on_hover = true
"#,
  )
  .unwrap();

  let migrated = apply_migrations(&mut config);
  assert!(migrated);
  assert_eq!(
    get_nested(&config, "terminal", "kernel")
      .unwrap()
      .as_str()
      .unwrap(),
    "alacritty"
  );
  assert_eq!(
    config.get("version").unwrap().as_str().unwrap(),
    CURRENT_CONFIG_VERSION
  );
}

#[test]
fn migrate_20260417_3_rewrites_keybindings_to_key_first_format() {
  let mut config: Value = toml::from_str(
    r##"
version = "20260417.3"

[keybindings]
copy = ["ctrl-shift-c", "ctrl-insert"]
paste = "ctrl-shift-v"
"##,
  )
  .unwrap();

  let migrated = apply_migrations(&mut config);
  assert!(migrated);

  let keybindings = config.get("keybindings").unwrap();
  assert!(keybindings.get("copy").is_none());
  assert!(keybindings.get("paste").is_none());
  assert_eq!(
    keybindings.get("ctrl-shift-c").unwrap().as_str().unwrap(),
    "copy"
  );
  assert_eq!(
    keybindings.get("ctrl-insert").unwrap().as_str().unwrap(),
    "copy"
  );
  assert_eq!(
    keybindings.get("ctrl-shift-v").unwrap().as_str().unwrap(),
    "paste"
  );
  assert_eq!(
    config.get("version").unwrap().as_str().unwrap(),
    CURRENT_CONFIG_VERSION
  );
}
