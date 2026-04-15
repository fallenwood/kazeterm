//! Hot reload support for config and theme files
//!
//! This module provides file watching capabilities to automatically reload
//! configuration and theme changes without restarting the application.

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;

use futures::FutureExt;
use gpui::{App, AppContext, AsyncApp, WindowBackgroundAppearance};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use smol::channel::unbounded;

use crate::config::create_settings_store;
use ::config::Config;

/// Debounce duration for file changes (in milliseconds)
const DEBOUNCE_MS: u64 = 200;

/// Represents the type of file that was changed
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
  let config_paths = Config::get_config_file_paths();
  let themes_path = ::config::get_custom_themes_path();

  // If no config file exists yet, nothing to watch
  if config_paths.is_empty() && themes_path.is_none() {
    tracing::debug!("No config or themes path to watch");
    return;
  }

  cx.spawn(async move |cx: &mut AsyncApp| {
    if let Err(e) = run_file_watcher(cx, config_paths, themes_path).await {
      tracing::error!("Config watcher error: {}", e);
    }
  })
  .detach();
}

/// Check if an event kind represents an actual content change
fn is_content_change(kind: &EventKind) -> bool {
  matches!(
    kind,
    // Any modification (data, metadata, rename, etc.)
    EventKind::Modify(_)
      // Some editors use create + rename pattern
      | EventKind::Create(_)
      // Atomic save patterns can remove/recreate files
      | EventKind::Remove(_)
  )
}

/// Run the file watcher loop
async fn run_file_watcher(
  cx: &mut AsyncApp,
  mut config_paths: Vec<PathBuf>,
  mut themes_path: Option<PathBuf>,
) -> anyhow::Result<()> {
  let (tx, rx) = unbounded::<notify::Result<notify::Event>>();

  // Create watcher with raw notify (not debounced) so we can filter events
  let mut watcher: RecommendedWatcher = Watcher::new(
    move |result| {
      let _ = tx.send_blocking(result);
    },
    notify::Config::default(),
  )?;

  let mut watched_config_targets = HashSet::new();
  let mut watched_theme_path = None;
  sync_config_watches(&mut watcher, &config_paths, &mut watched_config_targets)?;
  sync_theme_watch(&mut watcher, themes_path.as_ref(), &mut watched_theme_path)?;

  // Track pending changes for debouncing
  let mut pending_changes: HashSet<FileChangeType> = HashSet::new();
  let mut debounce_timer: Option<smol::Timer> = None;

  // Process events asynchronously
  loop {
    // Use select to handle both incoming events and debounce timer
    futures::select_biased! {
      result = rx.recv().fuse() => {
        match result {
          Ok(Ok(event)) => {
            // Filter: only process actual content changes
            if !is_content_change(&event.kind) {
              tracing::debug!("Ignoring non-content event: {:?}", event.kind);
              continue;
            }

            // Determine what changed and add to pending set
            for path in &event.paths {
              let change_type = determine_change_type(path, &config_paths, themes_path.as_ref());
              tracing::debug!("Content change detected: {:?} ({:?})", path, change_type);
              pending_changes.insert(change_type);
            }

            // Reset debounce timer
            debounce_timer = Some(smol::Timer::after(Duration::from_millis(DEBOUNCE_MS)));
          }
          Ok(Err(error)) => {
            tracing::warn!("File watcher error: {:?}", error);
          }
          Err(e) => {
            tracing::error!("File watcher channel error: {}", e);
            break;
          }
        }
      }

      _ = async {
        if let Some(timer) = &mut debounce_timer {
          timer.await;
        } else {
          // No timer, wait forever (will be interrupted by rx.recv)
          futures::future::pending::<()>().await;
        }
      }.fuse() => {
        // Debounce timer fired, process pending changes
        debounce_timer = None;

        if !pending_changes.is_empty() {
          // Prioritize config reload (it includes theme)
          let change_type = if pending_changes.contains(&FileChangeType::Config) {
            FileChangeType::Config
          } else {
            FileChangeType::Theme
          };

          tracing::info!("Processing file change: {:?}", change_type);

          let reconfigure_watches = change_type == FileChangeType::Config;

          if let Err(e) = handle_file_change(cx, change_type).await {
            tracing::error!("Failed to reload config/theme: {}", e);
          }

          if reconfigure_watches {
            config_paths = Config::get_config_file_paths();
            themes_path = ::config::get_custom_themes_path();

            sync_config_watches(&mut watcher, &config_paths, &mut watched_config_targets)?;
            sync_theme_watch(&mut watcher, themes_path.as_ref(), &mut watched_theme_path)?;
          }

          pending_changes.clear();
        }
      }
    }
  }

  Ok(())
}

/// Determine what type of file changed
fn determine_change_type(
  changed_path: &PathBuf,
  config_paths: &[PathBuf],
  themes_path: Option<&PathBuf>,
) -> FileChangeType {
  for config_path in config_paths {
    if changed_path == config_path {
      return FileChangeType::Config;
    }

    if let (Some(config_parent), Some(config_name)) =
      (config_path.parent(), config_path.file_name())
      && let (Some(changed_parent), Some(changed_name)) =
        (changed_path.parent(), changed_path.file_name())
      && config_parent == changed_parent
      && config_name == changed_name
    {
      return FileChangeType::Config;
    }
  }

  // Check if it's in the themes directory
  if let Some(tp) = themes_path
    && changed_path.starts_with(tp)
    && changed_path.extension().is_some_and(|e| e == "toml")
  {
    return FileChangeType::Theme;
  }

  // Default to config (will reload everything anyway)
  FileChangeType::Config
}

fn sync_config_watches(
  watcher: &mut RecommendedWatcher,
  config_paths: &[PathBuf],
  watched_targets: &mut HashSet<PathBuf>,
) -> notify::Result<()> {
  let desired_targets = config_watch_targets(config_paths);

  for removed_path in watched_targets.difference(&desired_targets) {
    tracing::info!("Stopped watching config path: {}", removed_path.display());
    watcher.unwatch(removed_path)?;
  }

  for added_path in desired_targets.difference(watched_targets) {
    tracing::info!("Watching config path: {}", added_path.display());
    watcher.watch(added_path, RecursiveMode::NonRecursive)?;
  }

  *watched_targets = desired_targets;
  Ok(())
}

fn sync_theme_watch(
  watcher: &mut RecommendedWatcher,
  theme_path: Option<&PathBuf>,
  watched_theme_path: &mut Option<PathBuf>,
) -> notify::Result<()> {
  let desired_theme_path = theme_path
    .filter(|path| path.exists() && path.is_dir())
    .cloned();

  if let Some(current_path) = watched_theme_path.as_ref()
    && desired_theme_path.as_ref() != Some(current_path)
  {
    tracing::info!(
      "Stopped watching themes directory: {}",
      current_path.display()
    );
    watcher.unwatch(current_path)?;
    *watched_theme_path = None;
  }

  if let Some(path) = desired_theme_path
    && watched_theme_path.as_ref() != Some(&path)
  {
    tracing::info!("Watching themes directory: {}", path.display());
    watcher.watch(&path, RecursiveMode::Recursive)?;
    *watched_theme_path = Some(path);
  }

  Ok(())
}

fn config_watch_targets(config_paths: &[PathBuf]) -> HashSet<PathBuf> {
  config_paths
    .iter()
    .filter_map(|path| {
      if let Some(parent) = path.parent()
        && parent.exists()
      {
        return Some(parent.to_path_buf());
      }

      path.exists().then(|| path.to_path_buf())
    })
    .collect()
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
      tracing::info!("Reloaded config: theme={}", new_config.colors.theme);

      // Update the themes path if it changed
      if let Some(themes_path) = &new_config.appearance.themes_path {
        let path = PathBuf::from(themes_path);
        if path.exists() && path.is_dir() {
          ::config::set_custom_themes_path(path);
        }
      }

      // Detect system dark mode
      let system_is_dark = crate::system_is_dark(cx);

      // Create new settings store with updated theme
      let settings = create_settings_store(&new_config, system_is_dark);

      // Update globals
      cx.set_global(new_config.clone());
      cx.set_global(settings);

      // Re-bind terminal keybindings with updated config
      terminal::bind_terminal_keys(cx, &new_config.keybindings);

      // Re-initialize gpui-component theme
      themeing::SettingsStore::init_gpui_component_theme(cx);

      // Update window background appearance for transparency
      update_window_background_appearance(cx, &new_config);

      tracing::info!("Config and theme reloaded successfully");
    }
    FileChangeType::Theme => {
      // Only reload the theme, not the entire config
      let config = cx.global::<Config>().clone();
      let system_is_dark = crate::system_is_dark(cx);

      // Create new settings store with reloaded theme
      let settings = create_settings_store(&config, system_is_dark);

      // Update theme global
      cx.set_global(settings);

      // Re-initialize gpui-component theme
      themeing::SettingsStore::init_gpui_component_theme(cx);

      tracing::info!("Theme reloaded successfully: {}", config.colors.theme);
    }
  }
}

/// Reload config and theme from an external event.
///
/// This function can be called from the event system to trigger
/// a full configuration reload.
pub fn reload_config_and_theme_from_event(cx: &mut App) {
  reload_config_and_theme(cx, FileChangeType::Config);
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

/// Update the window background appearance for all open windows based on opacity config.
///
/// When `background_opacity` < 1.0, the window is set to transparent (or blurred if
/// `background_blur` is enabled) so the desktop behind it is visible.
/// When fully opaque, the window is set back to opaque mode.
fn update_window_background_appearance(cx: &mut App, config: &Config) {
  let opacity = config.appearance.get_background_opacity();
  let appearance = if opacity < 1.0 {
    if config.appearance.background_blur {
      WindowBackgroundAppearance::Blurred
    } else {
      WindowBackgroundAppearance::Transparent
    }
  } else {
    WindowBackgroundAppearance::Opaque
  };

  for window_handle in cx.windows() {
    let _ = cx.update_window(window_handle, |_, window, _| {
      window.set_background_appearance(appearance);
    });
  }
}
