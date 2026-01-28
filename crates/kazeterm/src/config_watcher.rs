//! Hot reload support for config and theme files
//!
//! This module provides file watching capabilities to automatically reload
//! configuration and theme changes without restarting the application.

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use gpui::{App, AsyncApp};
use notify_debouncer_mini::{DebouncedEventKind, new_debouncer};

use crate::config::create_settings_store;
use ::config::Config;

/// Debounce duration for file changes (in milliseconds)
const DEBOUNCE_MS: u64 = 300;

/// Represents the type of file that was changed
#[derive(Debug, Clone, PartialEq)]
pub enum FileChangeType {
  /// The main config file changed
  Config,
  /// A theme file changed
  Theme,
}

/// Start watching config and theme files for changes
///
/// This function spawns a background task that watches:
/// - The main config file (kazeterm.toml)
/// - The themes directory (if it exists)
///
/// When changes are detected, it reloads the config/theme and updates
/// the global state, triggering a re-render of all components.
pub fn start_config_watcher(cx: &mut App) {
  // Get paths to watch
  let config_path = Config::get_config_file_path();
  let themes_path = ::config::get_custom_themes_path();

  // If no config file exists yet, nothing to watch
  if config_path.is_none() && themes_path.is_none() {
    tracing::debug!("No config or themes path to watch");
    return;
  }

  cx.spawn(async move |cx: &mut AsyncApp| {
    if let Err(e) = run_file_watcher(cx, config_path, themes_path).await {
      tracing::error!("Config watcher error: {}", e);
    }
  })
  .detach();
}

/// Run the file watcher loop
async fn run_file_watcher(
  cx: &mut AsyncApp,
  config_path: Option<PathBuf>,
  themes_path: Option<PathBuf>,
) -> anyhow::Result<()> {
  let (tx, rx) = mpsc::channel();

  // Create debounced watcher
  let mut debouncer = new_debouncer(Duration::from_millis(DEBOUNCE_MS), tx)?;

  // Watch config file
  if let Some(path) = &config_path {
    if path.exists() {
      tracing::info!("Watching config file: {}", path.display());
      debouncer
        .watcher()
        .watch(path, notify::RecursiveMode::NonRecursive)?;
    }
  }

  // Watch themes directory
  if let Some(path) = &themes_path {
    if path.exists() && path.is_dir() {
      tracing::info!("Watching themes directory: {}", path.display());
      debouncer
        .watcher()
        .watch(path, notify::RecursiveMode::Recursive)?;
    }
  }

  // Process events
  loop {
    match rx.recv() {
      Ok(result) => match result {
        Ok(events) => {
          for event in events {
            if event.kind == DebouncedEventKind::Any {
              let change_type = determine_change_type(&event.path, &config_path, &themes_path);
              tracing::info!("File changed: {:?} ({:?})", event.path, change_type);

              // Handle the reload
              if let Err(e) = handle_file_change(cx, change_type).await {
                tracing::error!("Failed to reload config/theme: {}", e);
              }
            }
          }
        }
        Err(error) => {
          tracing::warn!("File watcher error: {:?}", error);
        }
      },
      Err(e) => {
        tracing::error!("File watcher channel error: {}", e);
        break;
      }
    }
  }

  Ok(())
}

/// Determine what type of file changed
fn determine_change_type(
  changed_path: &PathBuf,
  config_path: &Option<PathBuf>,
  themes_path: &Option<PathBuf>,
) -> FileChangeType {
  // Check if it's the config file
  if let Some(cp) = config_path {
    if changed_path == cp {
      return FileChangeType::Config;
    }
  }

  // Check if it's in the themes directory
  if let Some(tp) = themes_path {
    if changed_path.starts_with(tp) && changed_path.extension().is_some_and(|e| e == "toml") {
      return FileChangeType::Theme;
    }
  }

  // Default to config (will reload everything anyway)
  FileChangeType::Config
}

/// Handle a file change by reloading config/theme
async fn handle_file_change(cx: &mut AsyncApp, change_type: FileChangeType) -> anyhow::Result<()> {
  cx.update(|cx| {
    reload_config_and_theme(cx, change_type);
  })?;

  Ok(())
}

/// Reload config and/or theme based on what changed
fn reload_config_and_theme(cx: &mut App, change_type: FileChangeType) {
  match change_type {
    FileChangeType::Config => {
      // Reload the entire config (which includes theme settings)
      let new_config = Config::load();
      tracing::info!("Reloaded config: theme={}", new_config.theme);

      // Update the themes path if it changed
      if let Some(themes_path) = &new_config.themes_path {
        let path = PathBuf::from(themes_path);
        if path.exists() && path.is_dir() {
          ::config::set_custom_themes_path(path);
        }
      }

      // Detect system dark mode
      let system_is_dark = crate::detect_system_dark_mode();

      // Create new settings store with updated theme
      let settings = create_settings_store(&new_config, system_is_dark);

      // Update globals
      cx.set_global(new_config);
      cx.set_global(settings);

      // Re-initialize gpui-component theme
      themeing::SettingsStore::init_gpui_component_theme(cx);

      tracing::info!("Config and theme reloaded successfully");
    }
    FileChangeType::Theme => {
      // Only reload the theme, not the entire config
      let config = cx.global::<Config>().clone();
      let system_is_dark = crate::detect_system_dark_mode();

      // Create new settings store with reloaded theme
      let settings = create_settings_store(&config, system_is_dark);

      // Update theme global
      cx.set_global(settings);

      // Re-initialize gpui-component theme
      themeing::SettingsStore::init_gpui_component_theme(cx);

      tracing::info!("Theme reloaded successfully: {}", config.theme);
    }
  }
}

/// Add a new path to watch (e.g., when themes_path changes)
///
/// This is useful when the user changes the themes_path in config
/// and we need to start watching the new directory.
#[allow(dead_code)]
pub fn watch_additional_path(path: PathBuf) {
  // For now, this would require restarting the watcher
  // A more sophisticated implementation could use channels to communicate
  // with the watcher task
  tracing::info!(
    "Additional path requested for watching: {} (requires restart)",
    path.display()
  );
}
