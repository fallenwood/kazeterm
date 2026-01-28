//! Shell detection module for platform-specific shell discovery.
//!
//! This module provides functions to detect available shells on the system
//! with priority ordering for each platform.

use std::path::Path;

/// Represents a detected shell with its display name and command.
#[derive(Debug, Clone)]
pub struct DetectedShell {
  pub name: String,
  pub command: String,
}

/// Shell candidate with priority (lower number = higher priority).
struct ShellCandidate {
  name: &'static str,
  command: &'static str,
  /// Paths to check for the shell executable (platform-specific).
  paths: &'static [&'static str],
}

/// Check if an executable exists at any of the given paths or in PATH.
fn shell_exists(command: &str, paths: &[&str]) -> bool {
  // First check explicit paths
  for path in paths {
    if Path::new(path).exists() {
      return true;
    }
  }

  // Then check if it's in PATH using `which` on Unix or `where` on Windows
  #[cfg(unix)]
  {
    std::process::Command::new("which")
      .arg(command)
      .stdout(std::process::Stdio::null())
      .stderr(std::process::Stdio::null())
      .status()
      .map(|s| s.success())
      .unwrap_or(false)
  }

  #[cfg(windows)]
  {
    std::process::Command::new("where")
      .arg(command)
      .stdout(std::process::Stdio::null())
      .stderr(std::process::Stdio::null())
      .status()
      .map(|s| s.success())
      .unwrap_or(false)
  }
}

/// Get shell candidates for Windows with priority ordering.
#[cfg(target_os = "windows")]
fn get_shell_candidates() -> Vec<ShellCandidate> {
  vec![
    ShellCandidate {
      name: "PowerShell 7",
      command: "pwsh.exe",
      paths: &[
        "C:\\Program Files\\PowerShell\\7\\pwsh.exe",
        "C:\\Program Files (x86)\\PowerShell\\7\\pwsh.exe",
      ],
    },
    ShellCandidate {
      name: "PowerShell",
      command: "powershell.exe",
      paths: &["C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe"],
    },
    ShellCandidate {
      name: "Command Prompt",
      command: "cmd.exe",
      paths: &["C:\\Windows\\System32\\cmd.exe"],
    },
    ShellCandidate {
      name: "Git Bash",
      command: "C:\\Program Files\\Git\\bin\\bash.exe",
      paths: &[
        "C:\\Program Files\\Git\\bin\\bash.exe",
        "C:\\Program Files (x86)\\Git\\bin\\bash.exe",
      ],
    },
    ShellCandidate {
      name: "WSL",
      command: "wsl.exe",
      paths: &["C:\\Windows\\System32\\wsl.exe"],
    },
  ]
}

/// Get shell candidates for macOS with priority ordering.
#[cfg(target_os = "macos")]
fn get_shell_candidates() -> Vec<ShellCandidate> {
  vec![
    ShellCandidate {
      name: "Zsh",
      command: "zsh",
      paths: &["/bin/zsh", "/usr/bin/zsh", "/usr/local/bin/zsh"],
    },
    ShellCandidate {
      name: "Bash",
      command: "bash",
      paths: &["/bin/bash", "/usr/bin/bash", "/usr/local/bin/bash"],
    },
    ShellCandidate {
      name: "Fish",
      command: "fish",
      paths: &["/usr/local/bin/fish", "/opt/homebrew/bin/fish"],
    },
    ShellCandidate {
      name: "Nushell",
      command: "nu",
      paths: &["/usr/local/bin/nu", "/opt/homebrew/bin/nu"],
    },
    ShellCandidate {
      name: "sh",
      command: "sh",
      paths: &["/bin/sh"],
    },
  ]
}

/// Get shell candidates for Linux with priority ordering.
#[cfg(all(unix, not(target_os = "macos")))]
fn get_shell_candidates() -> Vec<ShellCandidate> {
  vec![
    ShellCandidate {
      name: "Bash",
      command: "bash",
      paths: &["/bin/bash", "/usr/bin/bash"],
    },
    ShellCandidate {
      name: "Zsh",
      command: "zsh",
      paths: &["/bin/zsh", "/usr/bin/zsh"],
    },
    ShellCandidate {
      name: "Fish",
      command: "fish",
      paths: &["/usr/bin/fish", "/usr/local/bin/fish"],
    },
    ShellCandidate {
      name: "Nushell",
      command: "nu",
      paths: &["/usr/bin/nu", "/usr/local/bin/nu"],
    },
    ShellCandidate {
      name: "sh",
      command: "sh",
      paths: &["/bin/sh"],
    },
  ]
}

/// Detect all available shells on the system, ordered by priority.
pub fn detect_shells() -> Vec<DetectedShell> {
  let candidates = get_shell_candidates();
  let mut detected = Vec::new();

  // On Unix, check if $SHELL is set and add it first if it's valid
  #[cfg(unix)]
  {
    if let Ok(shell_env) = std::env::var("SHELL") {
      if Path::new(&shell_env).exists() {
        // Extract shell name from path
        let name = Path::new(&shell_env)
          .file_name()
          .and_then(|n| n.to_str())
          .map(|n| {
            // Capitalize first letter for display
            let mut chars = n.chars();
            match chars.next() {
              Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
              None => n.to_string(),
            }
          })
          .unwrap_or_else(|| "Default Shell".to_string());

        detected.push(DetectedShell {
          name: format!("{} (Default)", name),
          command: shell_env,
        });
      }
    }
  }

  // Add all detected shells from candidates
  for candidate in candidates {
    if shell_exists(candidate.command, candidate.paths) {
      // Avoid duplicating the $SHELL entry
      let already_added = detected.iter().any(|d| {
        d.command == candidate.command || d.command.ends_with(&format!("/{}", candidate.command))
      });

      if !already_added {
        detected.push(DetectedShell {
          name: candidate.name.to_string(),
          command: candidate.command.to_string(),
        });
      }
    }
  }

  detected
}

/// Get the first available shell (highest priority).
/// Returns the command string for the shell.
pub fn get_default_shell() -> Option<String> {
  detect_shells().first().map(|s| s.command.clone())
}

/// Get a fallback shell command for the current platform.
/// This is used when no shells are detected (should be rare).
pub fn fallback_shell() -> String {
  #[cfg(target_os = "windows")]
  {
    "cmd.exe".to_string()
  }
  #[cfg(target_os = "macos")]
  {
    "/bin/zsh".to_string()
  }
  #[cfg(all(unix, not(target_os = "macos")))]
  {
    "/bin/sh".to_string()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_detect_shells_returns_non_empty() {
    let shells = detect_shells();
    // Should always find at least one shell on any system
    assert!(!shells.is_empty(), "Should detect at least one shell");
  }

  #[test]
  fn test_get_default_shell() {
    let default = get_default_shell();
    assert!(default.is_some(), "Should have a default shell");
  }

  #[test]
  fn test_fallback_shell_is_valid() {
    let fallback = fallback_shell();
    assert!(!fallback.is_empty(), "Fallback shell should not be empty");
  }
}
