//! Import Alacritty configuration into Kazeterm
//!
//! Parses `alacritty.toml` and converts relevant settings into
//! Kazeterm's `Config` and `ThemeFile` structures.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::{Config, Profile, ThemeColors, ThemeFile};

// ---------------------------------------------------------------------------
// Alacritty TOML structs (subset of settings we can map)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct AlacrittyConfig {
  font: AlacrittyFont,
  window: AlacrittyWindow,
  colors: AlacrittyColors,
  terminal: AlacrittyTerminal,
  scrolling: AlacrittyScrolling,
  cursor: AlaccrityCursor,
  selection: AlacrittySelection,
  #[serde(default)]
  env: HashMap<String, String>,
  general: AlacrittyGeneral,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct AlacrittyFont {
  normal: AlacrittyFontNormal,
  size: Option<f32>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct AlacrittyFontNormal {
  family: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct AlacrittyWindow {
  opacity: Option<f32>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct AlacrittyColors {
  primary: AlacrittyPrimaryColors,
  normal: AlacrittyAnsiColors,
  bright: AlacrittyAnsiColors,
  cursor: AlaccrityCursorColors,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct AlacrittyPrimaryColors {
  background: Option<String>,
  foreground: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct AlacrittyAnsiColors {
  black: Option<String>,
  red: Option<String>,
  green: Option<String>,
  yellow: Option<String>,
  blue: Option<String>,
  magenta: Option<String>,
  cyan: Option<String>,
  white: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct AlaccrityCursorColors {
  cursor: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct AlacrittyTerminal {
  shell: Option<AlacrittyShell>,
  osc52: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum AlacrittyShell {
  Simple(String),
  Detailed(AlacrittyShellDetailed),
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct AlacrittyShellDetailed {
  program: Option<String>,
  #[serde(default)]
  args: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct AlacrittyScrolling {
  history: Option<u32>,
  multiplier: Option<u8>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct AlaccrityCursor {
  style: AlaccrityCursorStyle,
  blink_interval: Option<u64>,
  unfocused_hollow: Option<bool>,
  thickness: Option<f32>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct AlaccrityCursorStyle {
  shape: Option<String>,
  blinking: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct AlacrittySelection {
  save_to_clipboard: Option<bool>,
  semantic_escape_chars: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct AlacrittyGeneral {
  working_directory: Option<String>,
}

// ---------------------------------------------------------------------------
// Conversion result
// ---------------------------------------------------------------------------

/// Result of importing an Alacritty configuration.
pub struct AlacrittyImportResult {
  /// Settings that should be merged into the Kazeterm config.
  pub config_patch: AlacrittyConfigPatch,
  /// A Kazeterm theme file derived from Alacritty's color scheme, if colors were present.
  pub theme: Option<ThemeFile>,
}

/// Subset of Kazeterm config fields that should be overwritten.
pub struct AlacrittyConfigPatch {
  pub font_family: Option<String>,
  pub font_size: Option<f32>,
  pub background_opacity: Option<f32>,
  pub shell_profile: Option<Profile>,
  pub scrollback_lines: Option<u32>,
  pub cursor_shape: Option<String>,
  pub cursor_blink: Option<bool>,
  pub cursor_blink_interval: Option<u64>,
  pub osc52: Option<String>,
  pub copy_on_select: Option<bool>,
  pub env: HashMap<String, String>,
  pub working_directory: Option<String>,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Get the default path to `alacritty.toml` on the current platform.
pub fn default_alacritty_config_path() -> Option<PathBuf> {
  #[cfg(target_os = "windows")]
  {
    dirs::config_dir().map(|d| d.join("alacritty").join("alacritty.toml"))
  }
  #[cfg(not(target_os = "windows"))]
  {
    dirs::home_dir().map(|d| d.join(".config").join("alacritty").join("alacritty.toml"))
  }
}

/// Parse an Alacritty config file and convert it to Kazeterm structures.
pub fn import_alacritty_config(
  path: &Path,
) -> Result<AlacrittyImportResult, Box<dyn std::error::Error>> {
  let content = std::fs::read_to_string(path)?;
  import_alacritty_config_str(&content)
}

/// Parse an Alacritty config string and convert it to Kazeterm structures.
pub fn import_alacritty_config_str(
  content: &str,
) -> Result<AlacrittyImportResult, Box<dyn std::error::Error>> {
  let alacritty: AlacrittyConfig = toml::from_str(content)?;

  let config_patch = build_config_patch(&alacritty);
  let theme = build_theme(&alacritty.colors);

  Ok(AlacrittyImportResult {
    config_patch,
    theme,
  })
}

/// Apply an import result to a Kazeterm `Config`, mutating it in place.
///
/// If a theme is present, it is saved to the themes directory and the config's
/// `theme` field is updated to reference it.
pub fn apply_import(config: &mut Config, result: AlacrittyImportResult) {
  let patch = result.config_patch;
  if let Some(v) = patch.font_family {
    config.font.family = v;
  }
  if let Some(v) = patch.font_size {
    config.font.size = v;
  }
  if let Some(v) = patch.background_opacity {
    config.appearance.background_opacity = v;
  }
  if let Some(profile) = patch.shell_profile {
    // Replace or add an "Alacritty" profile
    if let Some(existing) = config.profiles.iter_mut().find(|p| p.name == profile.name) {
      *existing = profile;
    } else {
      config.profiles.push(profile);
    }
  }
  if let Some(v) = patch.scrollback_lines {
    config.terminal.scrollback_lines = v;
  }
  if let Some(v) = patch.cursor_shape {
    config.cursor.shape = v;
  }
  if let Some(v) = patch.cursor_blink {
    config.cursor.blink = v;
  }
  if let Some(v) = patch.cursor_blink_interval {
    config.cursor.blink_interval = v;
  }
  if let Some(v) = patch.osc52 {
    config.terminal.osc52 = v;
  }
  if let Some(v) = patch.copy_on_select {
    config.terminal.copy_on_select = v;
  }
  if !patch.env.is_empty() {
    config.terminal.env = patch.env;
  }
  if let Some(v) = patch.working_directory {
    config.terminal.working_directory = Some(v);
  }
  if let Some(ref theme_file) = result.theme {
    config.colors.theme = theme_name_to_id(&theme_file.name);
  }
}

/// Save an imported theme file to the Kazeterm themes directory.
///
/// Returns the path where the file was written.
pub fn save_imported_theme(theme: &ThemeFile) -> Result<PathBuf, Box<dyn std::error::Error>> {
  let themes_dir = crate::get_custom_themes_path().unwrap_or_else(|| {
    let base = Config::get_config_path();
    base.join("themes")
  });
  std::fs::create_dir_all(&themes_dir)?;

  let file_name = format!("{}.toml", theme_name_to_id(&theme.name));
  let dest = themes_dir.join(file_name);

  let toml_str = toml::to_string_pretty(theme)?;
  std::fs::write(&dest, toml_str)?;
  Ok(dest)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn theme_name_to_id(name: &str) -> String {
  name.to_lowercase().replace(' ', "-")
}

fn build_config_patch(alacritty: &AlacrittyConfig) -> AlacrittyConfigPatch {
  let shell_profile = alacritty.terminal.shell.as_ref().map(|shell| match shell {
    AlacrittyShell::Simple(program) => Profile {
      name: "Alacritty".to_string(),
      shell: program.clone(),
      args: vec![],
      working_directory: None,
    },
    AlacrittyShell::Detailed(detailed) => Profile {
      name: "Alacritty".to_string(),
      shell: detailed.program.clone().unwrap_or_else(|| String::from("")),
      args: detailed.args.clone(),
      working_directory: None,
    },
  });

  // Map Alacritty cursor shape to Kazeterm's lowercase format
  let cursor_shape = alacritty.cursor.style.shape.as_ref().map(|s| {
    match s.to_lowercase().as_str() {
      "block" => "block",
      "underline" => "underline",
      "beam" => "beam",
      _ => "block",
    }
    .to_string()
  });

  // Map Alacritty blinking mode to a boolean
  let cursor_blink =
    alacritty
      .cursor
      .style
      .blinking
      .as_ref()
      .map(|s| match s.to_lowercase().as_str() {
        "always" | "on" => true,
        "never" | "off" => false,
        _ => true,
      });

  // Map Alacritty osc52 mode to Kazeterm's format
  let osc52 = alacritty.terminal.osc52.as_ref().map(|s| {
    match s.to_lowercase().as_str() {
      "disabled" => "disabled",
      "onlycopy" | "copy_only" => "copy_only",
      "onlypaste" | "paste_only" => "paste_only",
      "copypaste" | "copy_paste" => "copy_paste",
      _ => "copy_only",
    }
    .to_string()
  });

  // Filter out "None" working directory (Alacritty uses "None" string)
  let working_directory = alacritty
    .general
    .working_directory
    .clone()
    .filter(|wd| !wd.eq_ignore_ascii_case("none") && !wd.is_empty());

  AlacrittyConfigPatch {
    font_family: alacritty.font.normal.family.clone(),
    font_size: alacritty.font.size,
    background_opacity: alacritty.window.opacity,
    shell_profile,
    scrollback_lines: alacritty.scrolling.history,
    cursor_shape,
    cursor_blink,
    cursor_blink_interval: alacritty.cursor.blink_interval,
    osc52,
    copy_on_select: alacritty.selection.save_to_clipboard,
    env: alacritty.env.clone(),
    working_directory,
  }
}

fn normalize_color(hex: &str) -> String {
  let hex = hex.trim_start_matches('#');
  if hex.starts_with("0x") || hex.starts_with("0X") {
    return format!("#{}", &hex[2..]);
  }
  format!("#{hex}")
}

fn build_theme(colors: &AlacrittyColors) -> Option<ThemeFile> {
  // Only produce a theme if at least one color is specified
  let has_any = colors.primary.background.is_some()
    || colors.primary.foreground.is_some()
    || colors.normal.black.is_some()
    || colors.bright.black.is_some();

  if !has_any {
    return None;
  }

  let theme_colors = ThemeColors {
    background: colors
      .primary
      .background
      .as_ref()
      .map(|s| normalize_color(s)),
    foreground: colors
      .primary
      .foreground
      .as_ref()
      .map(|s| normalize_color(s)),
    accent: colors
      .primary
      .foreground
      .as_ref()
      .map(|s| normalize_color(s)),
    border: None,
    black: colors.normal.black.as_ref().map(|s| normalize_color(s)),
    red: colors.normal.red.as_ref().map(|s| normalize_color(s)),
    green: colors.normal.green.as_ref().map(|s| normalize_color(s)),
    yellow: colors.normal.yellow.as_ref().map(|s| normalize_color(s)),
    blue: colors.normal.blue.as_ref().map(|s| normalize_color(s)),
    magenta: colors.normal.magenta.as_ref().map(|s| normalize_color(s)),
    cyan: colors.normal.cyan.as_ref().map(|s| normalize_color(s)),
    white: colors.normal.white.as_ref().map(|s| normalize_color(s)),
    bright_black: colors.bright.black.as_ref().map(|s| normalize_color(s)),
    bright_red: colors.bright.red.as_ref().map(|s| normalize_color(s)),
    bright_green: colors.bright.green.as_ref().map(|s| normalize_color(s)),
    bright_yellow: colors.bright.yellow.as_ref().map(|s| normalize_color(s)),
    bright_blue: colors.bright.blue.as_ref().map(|s| normalize_color(s)),
    bright_magenta: colors.bright.magenta.as_ref().map(|s| normalize_color(s)),
    bright_cyan: colors.bright.cyan.as_ref().map(|s| normalize_color(s)),
    bright_white: colors.bright.white.as_ref().map(|s| normalize_color(s)),
    cursor: colors.cursor.cursor.as_ref().map(|s| normalize_color(s)),
  };

  Some(ThemeFile {
    name: "Alacritty Import".to_string(),
    dark: theme_colors,
    light: None,
  })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parse_minimal_config() {
    let toml = r##"
[font]
size = 14.0

[font.normal]
family = "JetBrains Mono"
"##;
    let result = import_alacritty_config_str(toml).unwrap();
    assert_eq!(
      result.config_patch.font_family.as_deref(),
      Some("JetBrains Mono")
    );
    assert_eq!(result.config_patch.font_size, Some(14.0));
    assert!(result.theme.is_none());
  }

  #[test]
  fn parse_full_colors() {
    let toml = r##"
[colors.primary]
background = "#1d1f21"
foreground = "#c5c8c6"

[colors.normal]
black   = "#1d1f21"
red     = "#cc6666"
green   = "#b5bd68"
yellow  = "#f0c674"
blue    = "#81a2be"
magenta = "#b294bb"
cyan    = "#8abeb7"
white   = "#c5c8c6"

[colors.bright]
black   = "#666666"
red     = "#d54e53"
green   = "#b9ca4a"
yellow  = "#e7c547"
blue    = "#7aa6da"
magenta = "#c397d8"
cyan    = "#70c0b1"
white   = "#eaeaea"
"##;
    let result = import_alacritty_config_str(toml).unwrap();
    let theme = result.theme.unwrap();
    assert_eq!(theme.name, "Alacritty Import");
    assert_eq!(theme.dark.background.as_deref(), Some("#1d1f21"));
    assert_eq!(theme.dark.bright_red.as_deref(), Some("#d54e53"));
  }

  #[test]
  fn parse_shell_simple() {
    let toml = r##"
[terminal]
shell = "/usr/bin/fish"
"##;
    let result = import_alacritty_config_str(toml).unwrap();
    let profile = result.config_patch.shell_profile.unwrap();
    assert_eq!(profile.shell, "/usr/bin/fish");
    assert!(profile.args.is_empty());
  }

  #[test]
  fn parse_shell_detailed() {
    let toml = r##"
[terminal.shell]
program = "/bin/zsh"
args = ["-l", "--login"]
"##;
    let result = import_alacritty_config_str(toml).unwrap();
    let profile = result.config_patch.shell_profile.unwrap();
    assert_eq!(profile.shell, "/bin/zsh");
    assert_eq!(profile.args, vec!["-l", "--login"]);
  }

  #[test]
  fn parse_window_opacity() {
    let toml = r##"
[window]
opacity = 0.9
"##;
    let result = import_alacritty_config_str(toml).unwrap();
    assert_eq!(result.config_patch.background_opacity, Some(0.9));
  }

  #[test]
  fn apply_import_updates_config() {
    let toml = r##"
[font]
size = 16.0

[font.normal]
family = "Fira Code"

[window]
opacity = 0.85

[colors.primary]
background = "#000000"
foreground = "#ffffff"
"##;
    let result = import_alacritty_config_str(toml).unwrap();
    let mut config = Config::default();
    apply_import(&mut config, result);

    assert_eq!(config.font.family, "Fira Code");
    assert_eq!(config.font.size, 16.0);
    assert!((config.appearance.background_opacity - 0.85).abs() < 0.001);
    assert_eq!(config.colors.theme, "alacritty-import");
  }

  #[test]
  fn normalize_color_handles_formats() {
    assert_eq!(normalize_color("#FF0000"), "#FF0000");
    assert_eq!(normalize_color("0xFF0000"), "#FF0000");
    assert_eq!(normalize_color("FF0000"), "#FF0000");
  }

  #[test]
  fn empty_config_parses_ok() {
    let toml = "";
    let result = import_alacritty_config_str(toml).unwrap();
    assert!(result.config_patch.font_family.is_none());
    assert!(result.theme.is_none());
  }

  #[test]
  fn parse_scrolling_history() {
    let toml = r##"
[scrolling]
history = 5000
"##;
    let result = import_alacritty_config_str(toml).unwrap();
    assert_eq!(result.config_patch.scrollback_lines, Some(5000));
  }

  #[test]
  fn parse_cursor_config() {
    let toml = r##"
[cursor.style]
shape = "Beam"
blinking = "Always"

[cursor]
blink_interval = 600
"##;
    let result = import_alacritty_config_str(toml).unwrap();
    assert_eq!(result.config_patch.cursor_shape.as_deref(), Some("beam"));
    assert_eq!(result.config_patch.cursor_blink, Some(true));
    assert_eq!(result.config_patch.cursor_blink_interval, Some(600));
  }

  #[test]
  fn parse_cursor_blinking_off() {
    let toml = r##"
[cursor.style]
blinking = "Off"
"##;
    let result = import_alacritty_config_str(toml).unwrap();
    assert_eq!(result.config_patch.cursor_blink, Some(false));
  }

  #[test]
  fn parse_selection_save_to_clipboard() {
    let toml = r##"
[selection]
save_to_clipboard = true
"##;
    let result = import_alacritty_config_str(toml).unwrap();
    assert_eq!(result.config_patch.copy_on_select, Some(true));
  }

  #[test]
  fn parse_osc52() {
    let toml = r##"
[terminal]
osc52 = "CopyPaste"
"##;
    let result = import_alacritty_config_str(toml).unwrap();
    assert_eq!(result.config_patch.osc52.as_deref(), Some("copy_paste"));
  }

  #[test]
  fn parse_env_vars() {
    let toml = r##"
[env]
TERM = "xterm-256color"
MY_VAR = "hello"
"##;
    let result = import_alacritty_config_str(toml).unwrap();
    assert_eq!(
      result.config_patch.env.get("TERM").map(|s| s.as_str()),
      Some("xterm-256color")
    );
    assert_eq!(
      result.config_patch.env.get("MY_VAR").map(|s| s.as_str()),
      Some("hello")
    );
  }

  #[test]
  fn parse_working_directory() {
    let toml = r##"
[general]
working_directory = "/home/user"
"##;
    let result = import_alacritty_config_str(toml).unwrap();
    assert_eq!(
      result.config_patch.working_directory.as_deref(),
      Some("/home/user")
    );
  }

  #[test]
  fn parse_working_directory_none() {
    let toml = r##"
[general]
working_directory = "None"
"##;
    let result = import_alacritty_config_str(toml).unwrap();
    assert!(result.config_patch.working_directory.is_none());
  }

  #[test]
  fn apply_import_with_new_fields() {
    let toml = r##"
[scrolling]
history = 5000

[cursor.style]
shape = "Underline"
blinking = "Never"

[cursor]
blink_interval = 600

[terminal]
osc52 = "CopyPaste"

[selection]
save_to_clipboard = true

[env]
MY_VAR = "test"

[general]
working_directory = "/tmp"
"##;
    let result = import_alacritty_config_str(toml).unwrap();
    let mut config = Config::default();
    apply_import(&mut config, result);

    assert_eq!(config.terminal.scrollback_lines, 5000);
    assert_eq!(config.cursor.shape, "underline");
    assert_eq!(config.cursor.blink, false);
    assert_eq!(config.cursor.blink_interval, 600);
    assert_eq!(config.terminal.osc52, "copy_paste");
    assert_eq!(config.terminal.copy_on_select, true);
    assert_eq!(config.terminal.env.get("MY_VAR").map(|s| s.as_str()), Some("test"));
    assert_eq!(config.terminal.working_directory.as_deref(), Some("/tmp"));
  }
}
