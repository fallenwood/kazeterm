use gpui::Rgba;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub mod palette;
pub use palette::Palette;

mod theme;
pub use theme::{ThemeColors, ThemeFile, ThemeMode, load_theme, load_theme_from_assets};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Profile {
  pub name: String,
  pub shell: String,
  pub working_directory: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
  pub theme: String,
  pub theme_mode: ThemeMode,
  pub default_profile: Option<String>,
  #[serde(default)]
  pub profiles: Vec<Profile>,
  pub font_size: f32,
  pub font_family: String,
  pub ui_font_family: String,
  pub ui_font_size: f32,
  pub window_width: f32,
  pub window_height: f32,
}

impl Default for Config {
  fn default() -> Self {
    Self {
      theme: "one_dark".to_string(),
      theme_mode: ThemeMode::default(),
      default_profile: None,
      profiles: default_profiles(),
      font_size: 18.0,
      font_family: "Cascadia Code NF".to_string(),
      #[cfg(target_os = "windows")]
      ui_font_family: "Segoe UI".to_string(),
      #[cfg(not(target_os = "windows"))]
      ui_font_family: "Noto Sans".to_string(),
      ui_font_size: 18.0,
      window_width: 800.0,
      window_height: 600.0,
    }
  }
}

fn default_profiles() -> Vec<Profile> {
  match std::env::consts::OS {
    "windows" => vec![
      Profile {
        name: "PowerShell".to_string(),
        shell: "powershell.exe".to_string(),
        working_directory: None,
      },
      Profile {
        name: "Command Prompt".to_string(),
        shell: "cmd.exe".to_string(),
        working_directory: None,
      },
      Profile {
        name: "Pwsh 7".to_string(),
        shell: "pwsh.exe".to_string(),
        working_directory: None,
      },
    ],
    "macos" => vec![
      Profile {
        name: "Zsh".to_string(),
        shell: "zsh".to_string(),
        working_directory: None,
      },
      Profile {
        name: "Bash".to_string(),
        shell: "bash".to_string(),
        working_directory: None,
      },
    ],
    _ => vec![
      Profile {
        name: "sh".to_string(),
        shell: "sh".to_string(),
        working_directory: None,
      },
      Profile {
        name: "Bash".to_string(),
        shell: "bash".to_string(),
        working_directory: None,
      },
    ],
  }
}

impl Config {
  pub fn load() -> Self {
    let config_path = Self::get_config_path();

    if !config_path.exists() {
      // #[cfg(not(debug_assertions))]
      {
        // Create default config file
        if let Err(e) = Self::create_default_config(&config_path) {
          tracing::error!("Failed to create default config: {}", e);
          return Self::default();
        } else {
          tracing::info!("Created default config at: {}", config_path.display());
        }
      }
    }

    match Self::load_from_path(&config_path) {
      Ok(config) => {
        tracing::info!("Loaded config from: {}", config_path.display());
        tracing::debug!("Config: {:?}", config);
        return config;
      }
      Err(e) => {
        tracing::error!(
          "Failed to load config from {}: {}",
          config_path.display(),
          e
        );
      }
    }

    tracing::info!("Using default config");
    Self::default()
  }

  /// Get the config file path
  /// On Windows: ~/AppData/Roaming/kazeterm/kazeterm.toml
  /// On other platforms: ~/.config/kazeterm/kazeterm.toml
  fn get_config_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
      if let Some(app_data) = dirs::data_dir() {
        return app_data.join("kazeterm").join("kazeterm.toml");
      }
    }

    #[cfg(not(target_os = "windows"))]
    {
      if let Some(home_dir) = dirs::home_dir() {
        return home_dir
          .join(".config")
          .join("kazeterm")
          .join("kazeterm.toml");
      }
    }

    unreachable!("Could not determine config file path because home/data directory is not found");
  }

  /// Create a default config file at the specified path
  #[allow(unused)]
  fn create_default_config(path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    // Create parent directories if they don't exist
    if let Some(parent) = path.parent() {
      std::fs::create_dir_all(parent)?;
    }

    // Generate config from default
    let default_config = Self::default();
    let config_str = toml::to_string_pretty(&default_config)?;

    // Add header comment
    let content = format!(
      "# Kazeterm Configuration\n# Generated automatically\n\n{}",
      config_str
    );

    std::fs::write(path, content)?;
    Ok(())
  }

  fn load_from_path(path: &PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;
    let config: Config = toml::from_str(&content)?;
    Ok(config)
  }

  pub fn get_shell(&self) -> String {
    self
      .get_default_profile()
      .map(|p| p.shell.clone())
      .unwrap_or_else(|| {
        std::env::var("SHELL").unwrap_or_else(|_| match std::env::consts::OS {
          "windows" => "powershell.exe".to_string(),
          "macos" => "zsh".to_string(),
          _ => "bash".to_string(),
        })
      })
  }

  pub fn get_default_profile(&self) -> Option<&Profile> {
    if self.profiles.is_empty() {
      return None;
    }

    if let Some(ref default_name) = self.default_profile {
      tracing::debug!("Looking for default profile: {}", default_name);
      if let Some(profile) = self.profiles.iter().find(|p| &p.name == default_name) {
        return Some(profile);
      }
    } else {
      tracing::warn!("not found default profile");
    }

    self.profiles.first()
  }

  pub fn get_profile(&self, name: &str) -> Option<&Profile> {
    self.profiles.iter().find(|p| p.name == name)
  }

  pub fn get_shell_for_profile(&self, profile_name: &str) -> Option<String> {
    self.get_profile(profile_name).map(|p| p.shell.clone())
  }

  pub fn get_profile_names(&self) -> Vec<&str> {
    self.profiles.iter().map(|p| p.name.as_str()).collect()
  }

  pub fn get_config_file_path() -> Option<PathBuf> {
    let path = Self::get_config_path();
    if path.exists() { Some(path) } else { None }
  }
}

impl gpui::Global for Config {}

pub fn to_hex_string(rgba: &Rgba) -> String {
  format!(
    "#{:02X}{:02X}{:02X}{:02X}",
    (rgba.r * 255.0) as u8,
    (rgba.g * 255.0) as u8,
    (rgba.b * 255.0) as u8,
    (rgba.a * 255.0) as u8
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  fn rgba(r: u8, g: u8, b: u8, a: u8) -> Rgba {
    Rgba {
      r: r as f32 / 255.0,
      g: g as f32 / 255.0,
      b: b as f32 / 255.0,
      a: a as f32 / 255.0,
    }
  }

  #[test]
  fn to_hex_string_formats_uppercase_rgba() {
    assert_eq!(to_hex_string(&rgba(255, 0, 0, 255)), "#FF0000FF");
    assert_eq!(to_hex_string(&rgba(0, 255, 0, 128)), "#00FF0080");
    assert_eq!(to_hex_string(&rgba(34, 85, 136, 255)), "#225588FF");
  }

  #[test]
  fn get_profile_helpers() {
    let profiles = vec![
      Profile {
        name: "one".to_string(),
        shell: "sh".to_string(),
        working_directory: None,
      },
      Profile {
        name: "two".to_string(),
        shell: "bash".to_string(),
        working_directory: Some("/tmp".to_string()),
      },
    ];

    let config = Config {
      theme: "one_dark".into(),
      theme_mode: ThemeMode::Dark,
      default_profile: Some("two".into()),
      profiles: profiles.clone(),
      font_size: 12.0,
      font_family: "Cascadia Code".into(),
      #[cfg(target_os = "windows")]
      ui_font_family: "Segoe UI".into(),
      #[cfg(not(target_os = "windows"))]
      ui_font_family: "Noto Sans".into(),
      ui_font_size: 12.0,
      window_width: 100.0,
      window_height: 50.0,
    };

    // get_profile
    assert!(config.get_profile("one").is_some());
    assert!(config.get_profile("missing").is_none());

    // get_default_profile prioritizes configured default
    assert_eq!(config.get_default_profile().unwrap().name, "two");

    // get_shell_for_profile
    assert_eq!(
      config.get_shell_for_profile("two").unwrap(),
      "bash".to_string()
    );
    assert!(config.get_shell_for_profile("missing").is_none());

    // get_profile_names preserves order
    let names = config.get_profile_names();
    assert_eq!(names, vec!["one", "two"]);
  }
}
