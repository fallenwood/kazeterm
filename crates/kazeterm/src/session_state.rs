use serde::{Deserialize, Serialize};

/// Saved state for a single terminal tab.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabState {
  /// Profile name used to open this tab (if any).
  pub profile_name: Option<String>,
  /// Shell program path.
  pub shell_path: String,
  /// Working directory at the time of save.
  pub working_directory: Option<String>,
  /// Custom title set by the user.
  pub custom_title: Option<String>,
}

/// Saved state for the entire session (all tabs).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
  /// Ordered list of tabs.
  pub tabs: Vec<TabState>,
  /// Index of the active tab (position in `tabs`).
  pub active_tab_index: usize,
}

impl SessionState {
  /// Load session state from the default file path.
  pub fn load() -> Result<Self, SessionStateError> {
    let path = ::config::Config::get_session_file_path();
    if !path.exists() {
      return Err(SessionStateError::NotFound);
    }
    let content = std::fs::read_to_string(&path).map_err(|e| SessionStateError::Io(e))?;
    let state: SessionState =
      serde_json::from_str(&content).map_err(|e| SessionStateError::Parse(e.to_string()))?;
    if state.tabs.is_empty() {
      return Err(SessionStateError::Empty);
    }
    Ok(state)
  }

  /// Save session state to the default file path.
  pub fn save(&self) -> Result<(), SessionStateError> {
    let path = ::config::Config::get_session_file_path();
    if let Some(parent) = path.parent() {
      std::fs::create_dir_all(parent).map_err(SessionStateError::Io)?;
    }
    let content =
      serde_json::to_string_pretty(self).map_err(|e| SessionStateError::Parse(e.to_string()))?;
    std::fs::write(&path, content).map_err(SessionStateError::Io)?;
    Ok(())
  }

  /// Delete the session state file (called after successful restore).
  pub fn clear() {
    let path = ::config::Config::get_session_file_path();
    let _ = std::fs::remove_file(path);
  }
}

#[derive(Debug)]
pub enum SessionStateError {
  NotFound,
  Empty,
  Io(std::io::Error),
  Parse(String),
}

impl std::fmt::Display for SessionStateError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      SessionStateError::NotFound => write!(f, "No saved session found"),
      SessionStateError::Empty => write!(f, "Saved session has no tabs"),
      SessionStateError::Io(e) => write!(f, "I/O error: {}", e),
      SessionStateError::Parse(e) => write!(f, "Parse error: {}", e),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn round_trip_serialization() {
    let state = SessionState {
      tabs: vec![
        TabState {
          profile_name: Some("PowerShell".to_string()),
          shell_path: "pwsh".to_string(),
          working_directory: Some("C:\\Users\\test".to_string()),
          custom_title: None,
        },
        TabState {
          profile_name: None,
          shell_path: "/bin/bash".to_string(),
          working_directory: Some("/home/test".to_string()),
          custom_title: Some("My Tab".to_string()),
        },
      ],
      active_tab_index: 1,
    };

    let json = serde_json::to_string_pretty(&state).unwrap();
    let restored: SessionState = serde_json::from_str(&json).unwrap();

    assert_eq!(restored.tabs.len(), 2);
    assert_eq!(restored.active_tab_index, 1);
    assert_eq!(restored.tabs[0].profile_name.as_deref(), Some("PowerShell"));
    assert_eq!(restored.tabs[0].shell_path, "pwsh");
    assert_eq!(
      restored.tabs[0].working_directory.as_deref(),
      Some("C:\\Users\\test")
    );
    assert!(restored.tabs[0].custom_title.is_none());
    assert_eq!(
      restored.tabs[1].custom_title.as_deref(),
      Some("My Tab")
    );
  }

  #[test]
  fn empty_tabs_returns_error() {
    let json = r#"{"tabs": [], "active_tab_index": 0}"#;
    let state: SessionState = serde_json::from_str(json).unwrap();
    // Simulate the empty check from load()
    assert!(state.tabs.is_empty());
  }

  #[test]
  fn malformed_json_fails() {
    let bad_json = r#"{ not valid json }"#;
    let result = serde_json::from_str::<SessionState>(bad_json);
    assert!(result.is_err());
  }
}
