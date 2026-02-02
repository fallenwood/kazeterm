//! Shell detection module for platform-specific shell discovery.
//!
//! This module provides functions to detect available shells on the system
//! with priority ordering for each platform.

use std::path::Path;
use std::process::Command;

/// Create a Command that won't show a console window on Windows.
#[cfg(windows)]
fn hidden_command(program: &str) -> Command {
  use std::os::windows::process::CommandExt;
  const CREATE_NO_WINDOW: u32 = 0x08000000;
  let mut cmd = Command::new(program);
  cmd.creation_flags(CREATE_NO_WINDOW);
  cmd
}

#[cfg(not(windows))]
fn hidden_command(program: &str) -> Command {
  Command::new(program)
}

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

  // Then check if it's in PATH using the `which` crate
  which::which(command).is_ok()
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
    ShellCandidate {
      name: "Nushell",
      command: "nu.exe",
      paths: &[],
    },
  ]
}

/// Detect Visual Studio Developer Command Prompts on Windows.
#[cfg(target_os = "windows")]
fn detect_vs_dev_shells() -> Vec<DetectedShell> {
  let mut shells = Vec::new();

  // Common Visual Studio installation paths
  // VS 2026 (v18) uses version number in path instead of year
  let vs_paths = [
    (
      "Visual Studio 2026",
      "C:\\Program Files\\Microsoft Visual Studio\\18",
    ),
    (
      "Visual Studio 2022",
      "C:\\Program Files\\Microsoft Visual Studio\\2022",
    ),
    (
      "Visual Studio 2019",
      "C:\\Program Files (x86)\\Microsoft Visual Studio\\2019",
    ),
    (
      "Visual Studio 2017",
      "C:\\Program Files (x86)\\Microsoft Visual Studio\\2017",
    ),
  ];

  let editions = ["Enterprise", "Professional", "Community", "BuildTools"];

  for (vs_name, base_path) in &vs_paths {
    for edition in &editions {
      let vcvars_path = format!(
        "{}\\{}\\VC\\Auxiliary\\Build\\vcvars64.bat",
        base_path, edition
      );
      if Path::new(&vcvars_path).exists() {
        // Use cmd.exe with /k to initialize the VS environment
        shells.push(DetectedShell {
          name: format!("{} {} Developer Command Prompt", vs_name, edition),
          command: format!("cmd.exe /k \"{}\"", vcvars_path),
        });
        // Also add x86 variant if available
        let vcvars32_path = format!(
          "{}\\{}\\VC\\Auxiliary\\Build\\vcvars32.bat",
          base_path, edition
        );
        if Path::new(&vcvars32_path).exists() {
          shells.push(DetectedShell {
            name: format!("{} {} Developer Command Prompt (x86)", vs_name, edition),
            command: format!("cmd.exe /k \"{}\"", vcvars32_path),
          });
        }
      }
    }
  }

  // Also check for Visual Studio Developer PowerShell
  for (vs_name, base_path) in &vs_paths {
    for edition in &editions {
      let launch_devshell = format!(
        "{}\\{}\\Common7\\Tools\\Launch-VsDevShell.ps1",
        base_path, edition
      );
      if Path::new(&launch_devshell).exists() {
        shells.push(DetectedShell {
          name: format!("{} {} Developer PowerShell", vs_name, edition),
          command: format!(
            "powershell.exe -NoExit -Command \"& '{}'\"",
            launch_devshell
          ),
        });
      }
    }
  }

  shells
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

/// Detect Linux containers (Docker, Podman, distrobox) that can be used as shell environments.
/// This works on Linux and macOS where container runtimes are commonly available.
#[cfg(unix)]
pub fn detect_container_shells() -> Vec<DetectedShell> {
  let mut shells = Vec::new();

  // Detect Docker containers
  if let Ok(output) = hidden_command("docker")
    .args(["ps", "--format", "{{.Names}}"])
    .output()
  {
    if output.status.success() {
      let containers = String::from_utf8_lossy(&output.stdout);
      for container in containers.lines() {
        let container = container.trim();
        if !container.is_empty() {
          shells.push(DetectedShell {
            name: format!("[Docker] {}", container),
            command: format!("docker exec -it {} /bin/sh", container),
          });
        }
      }
    }
  }

  // Detect Podman containers
  if let Ok(output) = hidden_command("podman")
    .args(["ps", "--format", "{{.Names}}"])
    .output()
  {
    if output.status.success() {
      let containers = String::from_utf8_lossy(&output.stdout);
      for container in containers.lines() {
        let container = container.trim();
        if !container.is_empty() {
          shells.push(DetectedShell {
            name: format!("[Podman] {}", container),
            command: format!("podman exec -it {} /bin/sh", container),
          });
        }
      }
    }
  }

  // Detect distrobox containers
  if let Ok(output) = hidden_command("distrobox")
    .args(["list", "--no-color"])
    .output()
  {
    if output.status.success() {
      let containers = String::from_utf8_lossy(&output.stdout);
      for line in containers.lines().skip(1) {
        // Skip header line
        // distrobox list format: ID | NAME | STATUS | IMAGE
        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() >= 2 {
          let container_name = parts[1].trim();
          if !container_name.is_empty() && container_name != "NAME" {
            shells.push(DetectedShell {
              name: format!("[Distrobox] {}", container_name),
              command: format!("distrobox enter {}", container_name),
            });
          }
        }
      }
    }
  }

  shells
}

/// Detect Linux containers on Windows (via Docker Desktop or WSL).
#[cfg(target_os = "windows")]
pub fn detect_container_shells() -> Vec<DetectedShell> {
  let mut shells = Vec::new();

  // Detect Docker containers (Docker Desktop on Windows)
  if let Ok(output) = hidden_command("docker")
    .args(["ps", "--format", "{{.Names}}"])
    .output()
  {
    if output.status.success() {
      let containers = String::from_utf8_lossy(&output.stdout);
      for container in containers.lines() {
        let container = container.trim();
        if !container.is_empty() {
          shells.push(DetectedShell {
            name: format!("[Docker] {}", container),
            command: format!("docker exec -it {} /bin/sh", container),
          });
        }
      }
    }
  }

  // Detect Podman containers (Podman Desktop on Windows)
  if let Ok(output) = hidden_command("podman")
    .args(["ps", "--format", "{{.Names}}"])
    .output()
  {
    if output.status.success() {
      let containers = String::from_utf8_lossy(&output.stdout);
      for container in containers.lines() {
        let container = container.trim();
        if !container.is_empty() {
          shells.push(DetectedShell {
            name: format!("[Podman] {}", container),
            command: format!("podman exec -it {} /bin/sh", container),
          });
        }
      }
    }
  }

  shells
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
      let already_added = detected.iter().any(|d: &DetectedShell| {
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

  // Add Visual Studio Developer Command Prompts on Windows
  #[cfg(target_os = "windows")]
  {
    detected.extend(detect_vs_dev_shells());
  }

  // Add container shells (Docker, Podman, distrobox)
  // detected.extend(detect_container_shells());

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
