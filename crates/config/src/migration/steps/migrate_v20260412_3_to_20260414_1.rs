use toml::Value;

/// Restructure flat config into nested TOML tables (appearance, font, window, etc.).
pub(crate) fn migrate_v20260412_3_to_20260414_1(value: &mut Value) {
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
