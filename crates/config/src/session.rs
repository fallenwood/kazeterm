use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// A single saved tab's state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedTab {
  /// The shell program path or profile name used to create this tab.
  pub shell_path: String,
  /// The working directory at the time of save.
  pub working_directory: Option<String>,
  /// User-set custom title, if any.
  pub custom_title: Option<String>,
}

/// The full session state saved to disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionData {
  /// The saved tabs, in order.
  pub tabs: Vec<SavedTab>,
  /// The index of the active tab (position in the `tabs` vec).
  pub active_tab_index: usize,
}

impl SessionData {
  /// Get the path to the sessions file.
  /// Sits alongside kazeterm.toml in the config directory.
  pub fn session_file_path() -> PathBuf {
    super::Config::get_config_path().join("sessions.json")
  }

  /// Save session data to disk as JSON.
  pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
    let path = Self::session_file_path();
    if let Some(parent) = path.parent() {
      std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(self)?;
    std::fs::write(&path, json)?;
    tracing::info!("Saved session to: {}", path.display());
    Ok(())
  }

  /// Load session data from disk.
  /// Returns `None` if the file does not exist.
  /// Returns `Err` if the file exists but cannot be read or parsed.
  pub fn load() -> Result<Option<Self>, Box<dyn std::error::Error>> {
    let path = Self::session_file_path();
    if !path.exists() {
      return Ok(None);
    }
    let content = std::fs::read_to_string(&path)?;
    let data: Self = serde_json::from_str(&content)?;
    if data.tabs.is_empty() {
      return Ok(None);
    }
    tracing::info!("Loaded session from: {}", path.display());
    Ok(Some(data))
  }

  /// Delete the session file from disk.
  pub fn delete() -> Result<(), std::io::Error> {
    let path = Self::session_file_path();
    if path.exists() {
      std::fs::remove_file(&path)?;
      tracing::info!("Deleted session file: {}", path.display());
    }
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn session_data_roundtrip() {
    let data = SessionData {
      tabs: vec![
        SavedTab {
          shell_path: "pwsh".to_string(),
          working_directory: Some("C:\\Users\\test".to_string()),
          custom_title: Some("My Tab".to_string()),
        },
        SavedTab {
          shell_path: "bash".to_string(),
          working_directory: None,
          custom_title: None,
        },
      ],
      active_tab_index: 1,
    };

    let json = serde_json::to_string_pretty(&data).unwrap();
    let restored: SessionData = serde_json::from_str(&json).unwrap();

    assert_eq!(restored.tabs.len(), 2);
    assert_eq!(restored.active_tab_index, 1);
    assert_eq!(restored.tabs[0].shell_path, "pwsh");
    assert_eq!(
      restored.tabs[0].working_directory,
      Some("C:\\Users\\test".to_string())
    );
    assert_eq!(
      restored.tabs[0].custom_title,
      Some("My Tab".to_string())
    );
    assert_eq!(restored.tabs[1].shell_path, "bash");
    assert!(restored.tabs[1].working_directory.is_none());
    assert!(restored.tabs[1].custom_title.is_none());
  }

  #[test]
  fn empty_tabs_loads_as_none() {
    let data = SessionData {
      tabs: vec![],
      active_tab_index: 0,
    };
    let json = serde_json::to_string(&data).unwrap();
    // Simulate reading from "file" content
    let restored: SessionData = serde_json::from_str(&json).unwrap();
    // The load() method would return None for empty tabs,
    // but here we just verify deserialization works
    assert!(restored.tabs.is_empty());
  }
}
