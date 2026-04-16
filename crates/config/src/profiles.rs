use serde::{Deserialize, Serialize};

use crate::shell;
use crate::{Config, ssh};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Profile {
  pub name: String,
  pub shell: String,
  #[serde(default)]
  pub args: Vec<String>,
  pub working_directory: Option<String>,
}

pub(super) fn default_profiles() -> Vec<Profile> {
  let detected = shell::detect_shells();

  if detected.is_empty() {
    // Fallback if no shells detected (should be rare)
    return vec![Profile {
      name: "Shell".to_string(),
      shell: shell::fallback_shell(),
      args: vec![],
      working_directory: None,
    }];
  }

  detected
    .into_iter()
    .map(|s| Profile {
      name: s.name,
      shell: s.command,
      args: vec![],
      working_directory: None,
    })
    .collect()
}

pub(super) fn detect_container_profiles() -> Vec<Profile> {
  shell::detect_container_shells()
    .into_iter()
    .map(|s| {
      // Split command string into shell and args for containers
      // We know format is "docker exec -it ... /bin/sh" or "podman ..."
      let parts: Vec<String> = s
        .command
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();
      let (shell, args) = if !parts.is_empty() {
        (parts[0].clone(), parts[1..].to_vec())
      } else {
        (s.command, vec![])
      };

      Profile {
        name: s.name,
        shell,
        args,
        working_directory: None,
      }
    })
    .collect()
}

impl Config {
  pub fn get_shell(&self) -> String {
    self
      .get_default_profile()
      .map(|p| p.shell.clone())
      .unwrap_or_else(|| {
        // Try to get the first detected shell, or fall back to platform default
        shell::get_default_shell().unwrap_or_else(shell::fallback_shell)
      })
  }

  pub fn get_default_profile(&self) -> Option<&Profile> {
    if self.profiles.is_empty() {
      return None;
    }

    if let Some(ref default_name) = self.terminal.default_profile {
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
    self
      .profiles
      .iter()
      .find(|p| p.name == name)
      .or_else(|| self.container_profiles.iter().find(|p| p.name == name))
  }

  pub fn get_shell_for_profile(&self, profile_name: &str) -> Option<String> {
    self.get_profile(profile_name).map(|p| p.shell.clone())
  }

  pub fn get_local_profile_names(&self) -> Vec<String> {
    self.profiles.iter().map(|p| p.name.clone()).collect()
  }

  /// Get local profiles with their shell paths (name, shell_path)
  pub fn get_local_profiles_with_shells(&self) -> Vec<(String, String)> {
    self
      .profiles
      .iter()
      .map(|p| (p.name.clone(), p.shell.clone()))
      .collect()
  }

  pub fn get_container_profile_names(&self) -> Vec<String> {
    self
      .container_profiles
      .iter()
      .map(|p| p.name.clone())
      .collect()
  }

  /// Get container profiles with their shell paths (name, shell_path)
  pub fn get_container_profiles_with_shells(&self) -> Vec<(String, String)> {
    self
      .container_profiles
      .iter()
      .map(|p| (p.name.clone(), p.shell.clone()))
      .collect()
  }

  pub fn get_all_profile_names(&self) -> Vec<String> {
    let mut names: Vec<String> = self.profiles.iter().map(|p| p.name.clone()).collect();
    // Add container profiles
    for profile in &self.container_profiles {
      if !names.contains(&profile.name) {
        names.push(profile.name.clone());
      }
    }
    // Add SSH hosts
    let ssh_hosts = ssh::get_ssh_hosts();
    for host in ssh_hosts {
      if !names.contains(&host) {
        names.push(host);
      }
    }
    names
  }
}

#[cfg(test)]
mod tests {
  use crate::{
    AppearanceConfig, CURRENT_CONFIG_VERSION, ColorsConfig, Config, CursorConfig, FontConfig,
    KeybindingConfig, NotificationConfig, PaneConfig, Profile, TabConfig, TerminalConfig,
    ThemeMode, WindowConfig,
  };

  #[test]
  fn get_profile_helpers() {
    let profiles = vec![
      Profile {
        name: "one".to_string(),
        shell: "sh".to_string(),
        args: vec![],
        working_directory: None,
      },
      Profile {
        name: "two".to_string(),
        shell: "bash".to_string(),
        args: vec![],
        working_directory: Some("/tmp".to_string()),
      },
    ];

    let config = Config {
      version: CURRENT_CONFIG_VERSION.to_string(),
      imports: vec![],
      colors: ColorsConfig {
        theme: "one".into(),
        theme_mode: ThemeMode::Dark,
        bold_as_bright: false,
        minimum_contrast: 45.0,
      },
      appearance: AppearanceConfig {
        themes_path: None,
        background_opacity: 1.0,
        background_blur: false,
      },
      font: FontConfig {
        size: 12.0,
        family: "Cascadia Code".into(),
        #[cfg(target_os = "windows")]
        ui_family: "Segoe UI".into(),
        #[cfg(not(target_os = "windows"))]
        ui_family: "Noto Sans".into(),
        ui_size: 12.0,
      },
      window: WindowConfig {
        width: 100.0,
        height: 50.0,
        start_maximized: false,
        restore_workspace: true,
        key_debug_mode: false,
      },
      tab: TabConfig::default(),
      pane: PaneConfig::default(),
      terminal: TerminalConfig {
        default_profile: Some("two".into()),
        ..TerminalConfig::default()
      },
      cursor: CursorConfig::default(),
      notification: NotificationConfig::default(),
      profiles: profiles.clone(),
      keybindings: KeybindingConfig::default(),
      container_profiles: vec![],
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
    let names = config.get_local_profile_names();
    assert_eq!(names, vec!["one", "two"]);
  }
}
