use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use toml::Value;

use crate::{Config, GENERATED_CONFIG_HEADER, migration};

/// An editable snapshot of the primary configuration, without import overlays.
///
/// Loading migrates only the in-memory document. Saving changes only edited values,
/// retaining unknown settings and import order. An unchanged save leaves the original
/// text intact; changed saves use TOML serialization and cannot preserve comments.
#[derive(Debug, Clone)]
pub struct ConfigFile {
  path: PathBuf,
  target_path: PathBuf,
  content: String,
  raw: Value,
  config: Config,
}

impl ConfigFile {
  /// Load the primary configuration, creating the normal defaults if it is missing.
  pub fn load() -> Result<Self, String> {
    let path = Config::get_config_file_path_impl();
    if !path.try_exists().map_err(|error| error.to_string())? {
      Config::create_default_config(&path).map_err(|error| error.to_string())?;
    }
    Self::load_from_path(path)
  }

  /// Load a base file without reading imports or discovering shells and containers.
  /// Missing files are created with static defaults and no discovered profiles.
  pub fn load_from_path(path: PathBuf) -> Result<Self, String> {
    let path = if path.is_absolute() {
      path
    } else {
      std::env::current_dir()
        .map_err(|error| error.to_string())?
        .join(path)
    };
    if !path.try_exists().map_err(|error| error.to_string())? {
      Config::create_config_file(&path, &Config::file_defaults())
        .map_err(|error| format!("Could not create settings at {}: {error}", path.display()))?;
    }
    let (content, mut raw) = Config::read_raw_config_with_content(&path)
      .map_err(|error| format!("Could not load settings at {}: {error}", path.display()))?;
    let target_path = fs::canonicalize(&path).map_err(|error| error.to_string())?;
    migration::apply_migrations(&mut raw);
    let config = raw
      .clone()
      .try_into()
      .map_err(|error| format!("Could not read settings at {}: {error}", path.display()))?;
    Ok(Self {
      path,
      target_path,
      content,
      raw,
      config,
    })
  }

  /// Base settings only; imported files may override these values at runtime.
  pub fn config(&self) -> &Config {
    &self.config
  }

  pub fn path(&self) -> &Path {
    &self.path
  }

  /// Reload this file through the runtime loader, including its import overlays.
  /// The editable snapshot remains the base file only.
  pub fn load_effective(&self) -> Result<Config, String> {
    Config::load_from_path(&self.path).map_err(|error| error.to_string())
  }

  /// Validate and atomically replace the base file if its loaded text is unchanged.
  ///
  /// Conflicts leave both the on-disk file and this snapshot untouched. Imports
  /// are read only to validate the effective configuration and are never written.
  /// The snapshot advances only after a successful save.
  pub fn save(&mut self, edited: &Config) -> Result<(), String> {
    self.check_unchanged()?;
    validate_edited(edited)?;
    let old = Value::try_from(&self.config).map_err(|error| error.to_string())?;
    let new = Value::try_from(edited).map_err(|error| error.to_string())?;
    if old == new {
      self.validate_effective(&self.raw, edited)?;
      self.config = edited.clone();
      return Ok(());
    }

    let mut raw = self.raw.clone();
    // Legacy action-first bindings are accepted even in current-version files.
    // Normalize only when editing bindings so the serialized diff targets real keys.
    if old.get("keybindings") != new.get("keybindings")
      && let Some(Value::Table(bindings)) = raw.get_mut("keybindings")
    {
      crate::keybinding::rewrite_keybinding_table_to_key_first(bindings);
    }
    apply_diff(&mut raw, &old, &new);
    let persisted: Config = raw
      .clone()
      .try_into()
      .map_err(|error| format!("Could not serialize edited settings: {error}"))?;
    if Value::try_from(&persisted).map_err(|error| error.to_string())? != new {
      return Err(
        "Edited settings would change when reloaded. Use explicit 'noop' bindings to disable default shortcuts."
          .to_string(),
      );
    }
    self.validate_effective(&raw, edited)?;
    let content = format!(
      "{GENERATED_CONFIG_HEADER}{}",
      Config::to_toml_pretty(&raw).map_err(|error| error.to_string())?,
    );
    self.replace_file(&content)?;
    self.content = content;
    self.raw = raw;
    self.config = edited.clone();
    Ok(())
  }

  fn validate_effective(&self, raw: &Value, edited: &Config) -> Result<(), String> {
    let mut merged = raw.clone();
    let mut visited = std::collections::HashSet::from([Config::normalize_path(&self.path)]);
    for import in &edited.imports {
      Self::merge_import_for_validation(
        &mut merged,
        &self.path,
        import,
        !self.config.imports.contains(import),
        &mut visited,
      )?;
    }
    let effective: Config = merged
      .try_into()
      .map_err(|error| format!("The configuration with imports is invalid: {error}"))?;
    validate_edited(&effective)
      .map_err(|error| format!("The configuration with imports is invalid: {error}"))
  }

  fn check_unchanged(&self) -> Result<(), String> {
    if fs::canonicalize(&self.path).ok().as_ref() != Some(&self.target_path) {
      return Err(format!(
        "Settings at {} were moved, deleted, or redirected. Reload settings before saving.",
        self.path.display(),
      ));
    }
    match fs::read(&self.path) {
      Ok(content) if content == self.content.as_bytes() => Ok(()),
      Ok(_) => Err(format!(
        "Settings at {} changed outside this editor. Reload settings before saving.",
        self.path.display(),
      )),
      Err(error) => Err(format!(
        "Settings at {} could not be checked ({error}). The file may have been deleted. Reload settings before saving.",
        self.path.display(),
      )),
    }
  }

  fn merge_import_for_validation(
    merged: &mut Value,
    current_path: &Path,
    import: &str,
    required: bool,
    visited: &mut std::collections::HashSet<PathBuf>,
  ) -> Result<(), String> {
    let path = Config::resolve_import_path(current_path, import);
    let duplicate = !visited.insert(Config::normalize_path(&path));
    if duplicate && !required {
      return Ok(());
    }
    let imported = match Config::read_raw_config(&path) {
      Ok(imported) => imported,
      Err(error) if required => {
        return Err(format!(
          "Could not load added config import {}: {error}",
          path.display(),
        ));
      }
      // Preserve the runtime loader's tolerance for missing or malformed existing imports.
      Err(_) => return Ok(()),
    };
    if duplicate {
      return Ok(());
    }
    let nested = Config::extract_imports(&imported);
    Config::merge_config_value(merged, imported);
    for import in nested {
      Self::merge_import_for_validation(merged, &path, &import, required, visited)?;
    }
    Ok(())
  }

  fn replace_file(&self, content: &str) -> Result<(), String> {
    static NEXT_FILE: AtomicU64 = AtomicU64::new(0);
    let permissions = fs::metadata(&self.target_path)
      .map_err(|error| error.to_string())?
      .permissions();
    if permissions.readonly() {
      return Err(format!(
        "Settings at {} are read-only.",
        self.path.display()
      ));
    }
    let parent = self
      .target_path
      .parent()
      .ok_or("Settings path has no parent directory.")?;
    let (staged, mut file) = loop {
      let id = NEXT_FILE.fetch_add(1, Ordering::Relaxed);
      let path = parent.join(format!(".kazeterm-settings-{}-{id}", std::process::id()));
      match OpenOptions::new().write(true).create_new(true).open(&path) {
        Ok(file) => break (StagedFile(path), file),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
        Err(error) => return Err(format!("Could not stage settings: {error}")),
      }
    };
    let result = (|| -> Result<(), String> {
      file
        .set_permissions(permissions)
        .map_err(|error| error.to_string())?;
      file
        .write_all(content.as_bytes())
        .and_then(|_| file.sync_all())
        .map_err(|error| format!("Could not write settings: {error}"))?;
      // Close the staging handle before replacement, including on Windows.
      drop(file);
      self.check_unchanged()?;
      // Replace the resolved file, not a user's dotfile symlink.
      fs::rename(&staged.0, &self.target_path).map_err(|error| {
        format!(
          "Could not replace settings at {}: {error}",
          self.path.display()
        )
      })
    })();
    drop(staged);
    result
  }
}

struct StagedFile(PathBuf);

impl Drop for StagedFile {
  fn drop(&mut self) {
    if let Err(error) = fs::remove_file(&self.0)
      && error.kind() != std::io::ErrorKind::NotFound
    {
      tracing::warn!(
        "Could not remove staged settings {}: {error}",
        self.0.display()
      );
    }
  }
}

fn apply_diff(raw: &mut Value, old: &Value, new: &Value) {
  if old == new {
    return;
  }
  match (raw, old, new) {
    (Value::Table(raw), Value::Table(old), Value::Table(new)) => {
      for key in old.keys().filter(|key| !new.contains_key(*key)) {
        raw.remove(key);
      }
      for (key, new_value) in new {
        match old.get(key) {
          Some(old_value) if old_value != new_value => {
            let raw_value = raw
              .entry(key.clone())
              .or_insert_with(|| Value::Table(Default::default()));
            apply_diff(raw_value, old_value, new_value);
          }
          None => {
            raw.insert(key.clone(), new_value.clone());
          }
          _ => {}
        }
      }
    }
    (Value::Array(raw), Value::Array(old), Value::Array(new))
      if old.iter().chain(new).all(Value::is_table) =>
    {
      // Profile names keep unknown fields attached to their profile across removals
      // and reordering. A rename in an unchanged slot retains its unknown fields too.
      let mut used = vec![false; old.len()];
      let mut updated = Vec::with_capacity(new.len());
      for (index, new_value) in new.iter().enumerate() {
        let matched = old.iter().enumerate().position(|(index, old_value)| {
          !used[index] && old_value.get("name") == new_value.get("name")
        });
        let matched = matched.or_else(|| {
          (old.len() == new.len()
            && !used[index]
            && !new
              .iter()
              .any(|value| value.get("name") == old[index].get("name")))
          .then_some(index)
        });
        if let Some(index) = matched {
          used[index] = true;
          let mut value = raw
            .get(index)
            .cloned()
            .unwrap_or_else(|| old[index].clone());
          apply_diff(&mut value, &old[index], new_value);
          updated.push(value);
        } else {
          updated.push(new_value.clone());
        }
      }
      *raw = updated;
    }
    (raw, _, new) => *raw = new.clone(),
  }
}

fn validate_edited(config: &Config) -> Result<(), String> {
  config.validate().map_err(|error| error.to_string())?;
  for (name, value) in [
    ("font.size", config.font.size),
    ("font.ui_size", config.font.ui_size),
    ("window.width", config.window.width),
    ("window.height", config.window.height),
    ("tab.label_min_width", config.tab.label_min_width),
    ("tab.label_max_width", config.tab.label_max_width),
    ("pane.divider_width", config.pane.divider_width),
  ] {
    if !value.is_finite() || value <= 0.0 {
      return Err(format!("{name} must be a finite number greater than zero."));
    }
  }
  for (name, value) in [
    (
      "appearance.background_opacity",
      config.appearance.background_opacity,
    ),
    (
      "animation.fade_start_opacity",
      config.animation.fade_start_opacity,
    ),
    ("pane.inactive_opacity", config.pane.inactive_opacity),
  ] {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
      return Err(format!("{name} must be between 0 and 1."));
    }
  }
  if !config.colors.minimum_contrast.is_finite() || config.colors.minimum_contrast < 0.0 {
    return Err("colors.minimum_contrast must be a finite, nonnegative number.".into());
  }
  for (name, value) in [
    ("colors.theme", config.colors.theme.as_str()),
    ("font.family", config.font.family.as_str()),
    ("font.ui_family", config.font.ui_family.as_str()),
  ] {
    if value.trim().is_empty() {
      return Err(format!("{name} must not be empty."));
    }
  }
  if !matches!(config.cursor.shape.as_str(), "block" | "underline" | "beam") {
    return Err("cursor.shape must be block, underline, or beam.".into());
  }
  if !matches!(
    config.terminal.osc52.as_str(),
    "disabled" | "copy_only" | "paste_only" | "copy_paste"
  ) {
    return Err("terminal.osc52 must be disabled, copy_only, paste_only, or copy_paste.".into());
  }
  if config
    .imports
    .iter()
    .any(|path| path.trim().is_empty() || path.contains('\0'))
  {
    return Err("imports must contain nonempty file paths without NUL characters.".into());
  }
  let mut names = std::collections::HashSet::new();
  for profile in &config.profiles {
    if profile.name.trim().is_empty() || profile.shell.trim().is_empty() {
      return Err("Each profile must have a nonempty name and shell.".into());
    }
    if !names.insert(&profile.name) {
      return Err(format!("Profile name '{}' is duplicated.", profile.name));
    }
    if [&profile.name, &profile.shell]
      .into_iter()
      .chain(&profile.args)
      .chain(&profile.working_directory)
      .any(|value| value.contains('\0'))
    {
      return Err(format!(
        "Profile '{}' names, shells, arguments, and working directories cannot contain NUL characters.",
        profile.name,
      ));
    }
  }
  if config
    .terminal
    .working_directory
    .iter()
    .chain(&config.terminal.default_profile)
    .chain(&config.appearance.themes_path)
    .any(|value| value.contains('\0'))
  {
    return Err(
      "Working directories, profile names, and theme paths cannot contain NUL characters.".into(),
    );
  }
  for (name, value) in &config.terminal.env {
    if name.is_empty() || name.contains(['=', '\0']) || value.contains('\0') {
      return Err("Environment variable names must be nonempty and contain no '='; names and values cannot contain NUL characters.".into());
    }
  }
  Ok(())
}

#[cfg(test)]
mod tests;
