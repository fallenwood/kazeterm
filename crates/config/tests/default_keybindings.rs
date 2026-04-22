//! Integration tests for the default keybinding table.
//!
//! These tests lock down every default binding Kazeterm ships with,
//! so that accidental changes to `KeybindingConfig::default()` are caught by
//! CI rather than discovered by users.
//!
//! The expected bindings mirror the table in `.github/copilot-instructions.md`.

use config::KeybindingConfig;

/// Helper: assert that `binding` matches the given modifiers + key via
/// `KeybindingList::matches`, with no unintended extras.
fn assert_matches(
  list: &config::KeybindingList,
  control: bool,
  shift: bool,
  alt: bool,
  key: &str,
) {
  assert!(
    list.matches(control, shift, alt, /* platform */ false, key),
    "expected binding to match ctrl={control} shift={shift} alt={alt} key={key}, got {:?}",
    list.display_text()
  );
}

/// Like `assert_matches` but with `platform=true` (Cmd on macOS).
#[cfg(target_os = "macos")]
fn assert_matches_platform(
  list: &config::KeybindingList,
  control: bool,
  shift: bool,
  alt: bool,
  key: &str,
) {
  assert!(
    list.matches(control, shift, alt, /* platform */ true, key),
    "expected binding to match ctrl={control} shift={shift} alt={alt} platform=true key={key}, got {:?}",
    list.display_text()
  );
}

#[test]
fn default_copy_and_paste() {
  let kb = KeybindingConfig::default();
  #[cfg(not(target_os = "macos"))]
  {
    assert_matches(&kb.copy, true, true, false, "c");
    assert_matches(&kb.paste, true, true, false, "v");
  }
}

#[test]
fn default_zoom_bindings() {
  let kb = KeybindingConfig::default();
  #[cfg(target_os = "macos")]
  {
    assert_matches_platform(&kb.zoom_in, false, false, false, "=");
    assert_matches_platform(&kb.zoom_out, false, false, false, "-");
    assert_matches_platform(&kb.zoom_reset, false, false, false, "0");
  }
  #[cfg(not(target_os = "macos"))]
  {
    assert_matches(&kb.zoom_in, true, false, false, "=");
    assert_matches(&kb.zoom_out, true, false, false, "-");
    assert_matches(&kb.zoom_reset, true, false, false, "0");
  }
}

#[test]
fn default_tab_navigation_bindings() {
  let kb = KeybindingConfig::default();
  assert_matches(&kb.next_tab, true, false, false, "tab");
  assert_matches(&kb.previous_tab, true, true, false, "tab");
  #[cfg(not(target_os = "macos"))]
  {
    assert_matches(&kb.select_tab_1, true, false, true, "1");
    assert_matches(&kb.select_tab_8, true, false, true, "8");
    assert_matches(&kb.select_last_tab, true, false, true, "9");
  }
}

#[test]
fn default_split_and_pane_bindings() {
  let kb = KeybindingConfig::default();
  // Non-mac defaults: Alt+Shift+-, Alt+Shift+=, Ctrl+Shift+W, Ctrl+Shift+], [, X
  assert_matches(&kb.split_horizontal, false, true, true, "-");
  assert_matches(&kb.split_vertical, false, true, true, "=");
  #[cfg(not(target_os = "macos"))]
  {
    assert_matches(&kb.close_pane, true, true, false, "w");
    assert_matches(&kb.focus_next_pane, true, true, false, "]");
    assert_matches(&kb.focus_previous_pane, true, true, false, "[");
  }
  assert_matches(&kb.swap_split_panes, true, true, false, "x");
}

#[test]
fn default_search_and_tab_bar_bindings() {
  let kb = KeybindingConfig::default();
  #[cfg(not(target_os = "macos"))]
  assert_matches(&kb.toggle_search, true, true, false, "f");
  assert_matches(&kb.toggle_tab_bar, true, true, false, "b");
}

#[test]
fn default_fullscreen_binding() {
  let kb = KeybindingConfig::default();
  #[cfg(not(target_os = "macos"))]
  assert_matches(&kb.toggle_fullscreen, false, false, false, "f11");
}

#[test]
fn matches_main_window_shortcut_covers_defaults() {
  let kb = KeybindingConfig::default();
  assert!(kb.matches_main_window_shortcut(true, false, false, false, "tab"));
  assert!(kb.matches_main_window_shortcut(false, true, true, false, "-")); // split_horizontal
  #[cfg(not(target_os = "macos"))]
  {
    assert!(kb.matches_main_window_shortcut(true, true, false, false, "f")); // toggle_search
  }
  assert!(!kb.matches_main_window_shortcut(true, true, true, false, "q"));
}

#[test]
fn noop_is_empty_by_default() {
  let kb = KeybindingConfig::default();
  assert!(kb.noop.iter().next().is_none());
}
