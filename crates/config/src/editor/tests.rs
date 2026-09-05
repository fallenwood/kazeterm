use super::*;
use crate::{CURRENT_CONFIG_VERSION, KeybindingList, Profile};

struct Fixture(PathBuf);

impl Fixture {
  fn new() -> Self {
    static NEXT_TEST: AtomicU64 = AtomicU64::new(0);
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
      ".editor-test-{}-{}-{}",
      std::process::id(),
      std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos(),
      NEXT_TEST.fetch_add(1, Ordering::Relaxed),
    ));
    fs::create_dir(&path).unwrap();
    Self(path)
  }

  fn path(&self) -> PathBuf {
    self.0.join("kazeterm.toml")
  }

  fn write(&self, body: &str) -> ConfigFile {
    fs::write(
      self.path(),
      format!("version = \"{CURRENT_CONFIG_VERSION}\"\n{body}"),
    )
    .unwrap();
    ConfigFile::load_from_path(self.path()).unwrap()
  }

  fn raw(&self) -> Value {
    toml::from_str(&fs::read_to_string(self.path()).unwrap()).unwrap()
  }

  fn assert_no_staging_files(&self) {
    assert!(fs::read_dir(&self.0).unwrap().all(|entry| {
      !entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with(".kazeterm-settings-")
    }));
  }
}

impl Drop for Fixture {
  fn drop(&mut self) {
    fs::remove_dir_all(&self.0).unwrap();
  }
}

#[cfg(unix)]
#[test]
fn editor_preserves_symlinks_and_rejects_retargeting() {
  let fixture = Fixture::new();
  let file = fixture.write("");
  let target = fixture.0.join("dotfile.toml");
  fs::rename(file.path(), &target).unwrap();
  std::os::unix::fs::symlink(&target, fixture.path()).unwrap();
  let mut file = ConfigFile::load_from_path(fixture.path()).unwrap();
  let mut edited = file.config().clone();
  edited.font.size = 22.0;
  file.save(&edited).unwrap();
  assert!(fs::symlink_metadata(fixture.path()).unwrap().is_symlink());
  let saved: Config = toml::from_str(&fs::read_to_string(&target).unwrap()).unwrap();
  assert_eq!(saved.font.size, 22.0);

  let replacement = fixture.0.join("replacement.toml");
  fs::copy(&target, &replacement).unwrap();
  fs::remove_file(fixture.path()).unwrap();
  std::os::unix::fs::symlink(&replacement, fixture.path()).unwrap();
  edited.font.size = 24.0;
  assert!(file.save(&edited).unwrap_err().contains("Reload"));
  assert_eq!(fs::read(&target).unwrap(), fs::read(&replacement).unwrap());
  fixture.assert_no_staging_files();
}

#[test]
fn editor_roundtrip_multiple_sections() {
  let fixture = Fixture::new();
  let mut file = fixture.write("");
  let mut edited = file.config().clone();
  edited.colors.theme = "nord".into();
  edited.font.size = 22.5;
  edited.font.ui_family = "Example UI".into();
  edited.appearance.background_opacity = 0.7;
  edited.animation.enabled = false;
  edited.window.start_maximized = true;
  edited.tab.vertical = true;
  edited.pane.inactive_opacity = 0.8;
  edited.cursor.shape = "beam".into();
  edited.notification.interval_secs = 15;
  edited.terminal.working_directory = Some("project".into());
  edited.terminal.env.insert("EDITOR".into(), "vim".into());
  edited.auto_update.proxy = Some("http://localhost:8080".into());
  edited.profiles.push(Profile {
    name: "Example".into(),
    shell: "example-shell".into(),
    args: vec!["--interactive".into(), "a\n  b".into()],
    working_directory: None,
  });
  file.save(&edited).unwrap();

  let reloaded = ConfigFile::load_from_path(fixture.path()).unwrap();
  assert_eq!(file.path(), fixture.path());
  assert_eq!(
    Value::try_from(reloaded.config()).unwrap(),
    Value::try_from(&edited).unwrap(),
  );
  assert_eq!(
    Value::try_from(file.config()).unwrap(),
    Value::try_from(&edited).unwrap(),
  );
  fixture.assert_no_staging_files();
}

#[test]
fn editor_preserves_unknown_and_unexposed_values() {
  let fixture = Fixture::new();
  let mut file = fixture.write(
    r##"
future = "keep"
[font]
size = 16.0
ligatures = true
[future_section]
enabled = true
nested = { value = 42 }
[auto_update]
last_check_unix_secs = 12345
restore_workspace_once = true
[[profiles]]
name = "First"
shell = "first"
future = { value = "one" }
[[profiles]]
name = "Second"
shell = "second"
future = { value = "two" }
"##,
  );
  let before = fixture.raw();
  let mut edited = file.config().clone();
  edited.font.size = 20.0;
  edited.profiles[0].shell = "changed".into();
  edited.profiles[0].name = "Renamed".into();
  file.save(&edited).unwrap();
  let raw = fixture.raw();
  assert_eq!(raw["future"], before["future"]);
  assert_eq!(raw["future_section"], before["future_section"]);
  assert_eq!(raw["auto_update"], before["auto_update"]);
  assert_eq!(raw["font"]["ligatures"], true.into());
  assert_eq!(
    raw["profiles"][0]["future"],
    before["profiles"][0]["future"]
  );

  edited.profiles.swap(0, 1);
  file.save(&edited).unwrap();
  assert_eq!(
    fixture.raw()["profiles"][0]["future"],
    before["profiles"][1]["future"]
  );
  edited.profiles.remove(0);
  file.save(&edited).unwrap();
  assert_eq!(
    fixture.raw()["profiles"][0]["future"],
    before["profiles"][0]["future"]
  );
  fixture.assert_no_staging_files();
}

#[test]
fn editor_never_flattens_or_writes_imports() {
  let fixture = Fixture::new();
  let first_path = fixture.0.join("first.toml");
  let second_path = fixture.0.join("second.toml");
  let first =
    "# import stays byte-identical\n[font]\nsize = 30.0\n[terminal.env]\nIMPORTED = 'yes'\n";
  let second = "[font]\nsize = 40.0\n";
  fs::write(&first_path, first).unwrap();
  fs::write(&second_path, second).unwrap();
  let mut file = fixture.write("imports = ['first.toml', 'second.toml']\n[font]\nsize = 16.0\n");
  assert_eq!(file.config().font.size, 16.0);
  assert!(file.config().terminal.env.is_empty());
  let mut edited = file.config().clone();
  edited.font.size = 21.0;
  edited.imports.swap(0, 1);
  file.save(&edited).unwrap();

  let mut merged = fixture.raw();
  Config::apply_imports(
    &mut merged,
    &fixture.path(),
    &edited.imports,
    &mut std::collections::HashSet::new(),
  );
  assert_eq!(merged["font"]["size"].as_float(), Some(30.0));
  assert_eq!(fixture.raw()["font"]["size"].as_float(), Some(21.0));
  assert!(fixture.raw().get("terminal").is_none());
  assert_eq!(fs::read_to_string(&first_path).unwrap(), first);
  assert_eq!(fs::read_to_string(&second_path).unwrap(), second);

  // Existing missing or malformed imports keep the runtime loader's skip behavior.
  fs::remove_file(&first_path).unwrap();
  fs::write(&second_path, "not valid TOML!").unwrap();
  edited.font.size = 23.0;
  file.save(&edited).unwrap();
  assert!(!first_path.exists());
  assert_eq!(fs::read_to_string(second_path).unwrap(), "not valid TOML!");
}

#[test]
fn editor_rejects_added_unreadable_malformed_or_invalid_effective_imports() {
  let fixture = Fixture::new();
  let mut file = fixture.write("");
  let original = fs::read(fixture.path()).unwrap();
  let invalid_imports = [
    ("missing.toml", None, "Could not load added config import"),
    (
      "malformed.toml",
      Some("not valid TOML!"),
      "Could not load added config import",
    ),
    (
      "nested.toml",
      Some("imports = ['missing-child.toml']"),
      "missing-child.toml",
    ),
    (
      "kernel.toml",
      Some("[terminal]\nkernel = 'invalid'"),
      "configuration with imports is invalid",
    ),
    (
      "font.toml",
      Some("[font]\nsize = 'invalid'"),
      "configuration with imports is invalid",
    ),
    (
      "opacity.toml",
      Some("[appearance]\nbackground_opacity = 2.0"),
      "background_opacity",
    ),
  ];
  for (name, content, expected_error) in invalid_imports {
    let path = fixture.0.join(name);
    if let Some(content) = content {
      fs::write(&path, content).unwrap();
    }
    let mut edited = file.config().clone();
    edited.imports.push(name.into());
    edited.font.size = 21.0;
    let error = file.save(&edited).unwrap_err();
    assert!(error.contains(expected_error), "{error}");
    assert_eq!(fs::read(fixture.path()).unwrap(), original);
    assert!(file.config().imports.is_empty());
    if let Some(content) = content {
      assert_eq!(fs::read_to_string(path).unwrap(), content);
    }
    fixture.assert_no_staging_files();
  }
}

#[test]
fn editor_validates_final_import_priority_and_nested_relative_paths() {
  let fixture = Fixture::new();
  fs::create_dir(fixture.0.join("layers")).unwrap();
  fs::write(
    fixture.0.join("layers").join("first.toml"),
    "imports = ['nested.toml']\n[terminal]\nkernel = 'invalid'\n",
  )
  .unwrap();
  fs::write(
    fixture.0.join("layers").join("nested.toml"),
    "[terminal]\nkernel = 'alacritty'\n[font]\nsize = 30.0\n",
  )
  .unwrap();
  let mut file = fixture.write("imports = ['existing-missing.toml']\n");
  let mut edited = file.config().clone();
  edited.imports.push(
    PathBuf::from("layers")
      .join("first.toml")
      .to_string_lossy()
      .into_owned(),
  );
  edited.font.size = 21.0;
  file.save(&edited).unwrap();
  assert_eq!(file.config().font.size, 21.0);
  assert_eq!(fixture.raw()["font"]["size"].as_float(), Some(21.0));
  assert!(!fixture.0.join("existing-missing.toml").exists());
  let content = fs::read(fixture.path()).unwrap();
  fs::write(
    fixture.0.join("layers").join("nested.toml"),
    "[terminal]\nkernel = 'invalid'\n",
  )
  .unwrap();
  edited.font.size = 22.0;
  assert!(
    file
      .save(&edited)
      .unwrap_err()
      .contains("configuration with imports is invalid")
  );
  assert_eq!(fs::read(fixture.path()).unwrap(), content);
  assert_eq!(file.config().font.size, 21.0);
  // Removing the invalid import is permitted.
  edited.imports.pop();
  file.save(&edited).unwrap();
}

#[cfg(not(target_os = "linux"))]
#[test]
fn editor_rejects_platform_unsupported_imported_kernel() {
  let fixture = Fixture::new();
  let mut file = fixture.write("");
  fs::write(fixture.0.join("vte.toml"), "[terminal]\nkernel = 'vte'\n").unwrap();
  let original = fs::read(fixture.path()).unwrap();
  let mut edited = file.config().clone();
  edited.imports.push("vte.toml".into());
  assert!(file.save(&edited).unwrap_err().contains("not available"));
  assert_eq!(fs::read(fixture.path()).unwrap(), original);
}

#[test]
fn editor_rejects_nul_characters_in_profile_and_terminal_launch_settings() {
  let fixture = Fixture::new();
  let mut file = fixture.write("[[profiles]]\nname = 'Example'\nshell = 'example'\n");
  let original = fs::read(fixture.path()).unwrap();
  let invalid_edits: &[fn(&mut Config)] = &[
    |config| config.profiles[0].name = "bad\0name".into(),
    |config| config.profiles[0].shell = "bad\0shell".into(),
    |config| config.profiles[0].args = vec!["bad\0arg".into()],
    |config| config.profiles[0].working_directory = Some("bad\0path".into()),
    |config| config.terminal.working_directory = Some("bad\0path".into()),
    |config| {
      config
        .terminal
        .env
        .insert("NAME".into(), "bad\0value".into());
    },
  ];
  for edit in invalid_edits {
    let mut edited = file.config().clone();
    edit(&mut edited);
    assert!(file.save(&edited).unwrap_err().contains("NUL"));
    assert_eq!(fs::read(fixture.path()).unwrap(), original);
  }
}

#[test]
fn editor_rejects_external_edits_and_deletion_without_writing() {
  let fixture = Fixture::new();
  let mut file = fixture.write("# original\n");
  let original = fs::read(fixture.path()).unwrap();
  let mut edited = file.config().clone();
  edited.font.size = 25.0;
  for replacement in [b"# external comment\n".as_slice(), b"\xff"] {
    fs::write(fixture.path(), replacement).unwrap();
    let error = file.save(&edited).unwrap_err();
    assert!(error.contains("Reload settings"));
    assert_eq!(fs::read(fixture.path()).unwrap(), replacement);
    assert_eq!(file.config().font.size, 18.0);
    fixture.assert_no_staging_files();
  }
  fs::remove_file(fixture.path()).unwrap();
  assert!(file.save(&edited).unwrap_err().contains("Reload settings"));
  assert!(!fixture.path().exists());
  // A conflict must not advance the snapshot.
  fs::write(fixture.path(), original).unwrap();
  file.save(&edited).unwrap();
  assert_eq!(file.config().font.size, 25.0);
}

#[test]
fn editor_validates_edits_before_writing() {
  let fixture = Fixture::new();
  let mut file = fixture.write("");
  let original = fs::read(fixture.path()).unwrap();
  let invalid_edits: &[fn(&mut Config)] = &[
    |config| config.font.size = 0.0,
    |config| config.font.ui_size = f32::NAN,
    |config| config.window.width = f32::INFINITY,
    |config| config.appearance.background_opacity = 1.1,
    |config| config.pane.inactive_opacity = -0.1,
    |config| config.colors.minimum_contrast = -1.0,
    |config| config.font.family.clear(),
    |config| config.cursor.shape = "unknown".into(),
    |config| config.terminal.osc52 = "unknown".into(),
    |config| config.imports.push(String::new()),
    |config| {
      config
        .terminal
        .env
        .insert("BAD=NAME".into(), "value".into());
    },
    |config| {
      config.profiles.push(Profile {
        name: String::new(),
        shell: "shell".into(),
        args: vec![],
        working_directory: None,
      });
    },
  ];
  for edit in invalid_edits {
    let mut edited = file.config().clone();
    edit(&mut edited);
    assert!(file.save(&edited).is_err());
    assert_eq!(fs::read(fixture.path()).unwrap(), original);
    fixture.assert_no_staging_files();
  }
  #[cfg(not(target_os = "linux"))]
  {
    let mut edited = file.config().clone();
    edited.terminal.kernel = crate::TerminalKernel::Vte;
    assert!(file.save(&edited).unwrap_err().contains("not available"));
    assert_eq!(fs::read(fixture.path()).unwrap(), original);
  }
}

#[test]
fn editor_noop_save_keeps_comments_and_does_not_write_migrations() {
  let fixture = Fixture::new();
  let original = "# custom spacing and comments\nversion = '20260512.1'\n[font]\nsize=19.0\n";
  fs::write(fixture.path(), original).unwrap();
  let mut file = ConfigFile::load_from_path(fixture.path()).unwrap();
  assert_eq!(file.config().version, CURRENT_CONFIG_VERSION);
  assert_eq!(fs::read_to_string(fixture.path()).unwrap(), original);
  file.save(&file.config().clone()).unwrap();
  assert_eq!(fs::read_to_string(fixture.path()).unwrap(), original);
  assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 1);

  let mut edited = file.config().clone();
  edited.font.size = 20.0;
  file.save(&edited).unwrap();
  assert_eq!(
    fixture.raw()["version"].as_str(),
    Some(CURRENT_CONFIG_VERSION)
  );
  assert_eq!(
    ConfigFile::load_from_path(fixture.path())
      .unwrap()
      .config()
      .font
      .size,
    20.0
  );
}

#[test]
fn editor_repeated_saves_remove_options_and_map_keys_without_materializing_defaults() {
  let fixture = Fixture::new();
  let mut file =
    fixture.write("[terminal]\nworking_directory = 'project'\n[terminal.env]\nOLD = 'value'\n");
  let mut edited = file.config().clone();
  edited.font.size = 21.0;
  file.save(&edited).unwrap();
  let raw = fixture.raw();
  assert_eq!(raw["font"].as_table().unwrap().len(), 1);
  assert!(raw.get("window").is_none());
  edited.terminal.working_directory = None;
  edited.terminal.env.remove("OLD");
  edited.terminal.env.insert("NEW".into(), "next".into());
  edited.font.size = 22.0;
  file.save(&edited).unwrap();
  let raw = fixture.raw();
  assert!(raw["terminal"].get("working_directory").is_none());
  assert!(raw["terminal"]["env"].get("OLD").is_none());
  assert_eq!(raw["terminal"]["env"]["NEW"].as_str(), Some("next"));
  assert_eq!(file.config().font.size, 22.0);
  let content = fs::read(fixture.path()).unwrap();
  file.save(&edited).unwrap();
  assert_eq!(fs::read(fixture.path()).unwrap(), content);
  fixture.assert_no_staging_files();
}

#[test]
fn editor_missing_file_uses_static_defaults_and_creates_parents() {
  let fixture = Fixture::new();
  let path = fixture.0.join("nested").join("kazeterm.toml");
  let mut file = ConfigFile::load_from_path(path.clone()).unwrap();
  assert!(path.exists());
  assert_eq!(file.config().font.size, crate::FontConfig::default().size);
  assert!(file.config().profiles.is_empty());
  assert!(file.config().container_profiles.is_empty());
  let mut edited = file.config().clone();
  edited.font.size = 23.0;
  file.save(&edited).unwrap();
  assert_eq!(
    ConfigFile::load_from_path(path).unwrap().config().font.size,
    23.0
  );
}

#[test]
fn editor_preserves_keybinding_semantics_when_editing_legacy_entries() {
  let fixture = Fixture::new();
  let mut file = fixture.write("[keybindings]\ncopy = 'alt-c'\n");
  let mut edited = file.config().clone();
  let default_copy = crate::KeybindingConfig::default().copy;
  edited.keybindings.copy = KeybindingList::new("alt-shift-c");
  for binding in default_copy.iter() {
    edited.keybindings.noop.insert(binding);
  }
  file.save(&edited).unwrap();
  let reloaded = ConfigFile::load_from_path(fixture.path()).unwrap();
  assert_eq!(reloaded.config().keybindings.copy, "alt-shift-c");
  assert!(fixture.raw()["keybindings"].get("copy").is_none());
  assert!(fixture.raw()["keybindings"].get("alt-c").is_none());
  assert_eq!(
    fixture.raw()["keybindings"].as_table().unwrap().len(),
    default_copy.iter().count() + 1
  );
}

#[test]
fn editor_roundtrips_custom_reassigned_and_disabled_shortcuts_in_both_formats() {
  for body in [
    "[keybindings]\ncopy = 'alt-c'\npaste = 'alt-v'\nnoop = 'alt-x'\n",
    "[keybindings]\n'alt-c' = 'copy'\n'alt-v' = 'paste'\n'alt-x' = 'noop'\n",
  ] {
    let fixture = Fixture::new();
    let mut file = fixture.write(body);
    let default_copy = crate::KeybindingConfig::default()
      .copy
      .first()
      .unwrap()
      .to_string();
    let mut value = Value::try_from(file.config()).unwrap();
    let bindings = value["keybindings"].as_table_mut().unwrap();
    bindings.insert(default_copy.clone(), Value::String("paste".into()));
    bindings.insert("alt-c".into(), Value::String("noop".into()));
    bindings.insert("alt-shift-c".into(), Value::String("copy".into()));
    let edited: Config = value.try_into().unwrap();
    file.save(&edited).unwrap();

    let reloaded = ConfigFile::load_from_path(fixture.path()).unwrap();
    assert_eq!(
      Value::try_from(reloaded.config()).unwrap(),
      Value::try_from(&edited).unwrap(),
    );
    assert_eq!(reloaded.config().keybindings.copy, "alt-shift-c");
    assert!(
      reloaded
        .config()
        .keybindings
        .paste
        .iter()
        .any(|binding| binding == default_copy)
    );
    assert!(
      reloaded
        .config()
        .keybindings
        .noop
        .iter()
        .any(|binding| binding == "alt-c")
    );
    let raw = fixture.raw();
    assert_eq!(raw["keybindings"]["alt-x"].as_str(), Some("noop"));
    assert_eq!(raw["keybindings"]["alt-v"].as_str(), Some("paste"));
    assert!(raw["keybindings"].get("copy").is_none());
    assert!(raw["keybindings"].get("paste").is_none());
    assert!(raw["keybindings"].get("noop").is_none());

    let mut value = Value::try_from(file.config()).unwrap();
    let bindings = value["keybindings"].as_table_mut().unwrap();
    bindings.insert("alt-c".into(), Value::String("paste".into()));
    bindings.insert("alt-v".into(), Value::String("noop".into()));
    let edited: Config = value.try_into().unwrap();
    file.save(&edited).unwrap();
    assert_eq!(
      Value::try_from(ConfigFile::load_from_path(fixture.path()).unwrap().config()).unwrap(),
      Value::try_from(&edited).unwrap(),
    );
    assert_eq!(fixture.raw()["keybindings"]["alt-v"].as_str(), Some("noop"));
    fixture.assert_no_staging_files();
  }
}

#[test]
fn editor_cloned_snapshots_detect_later_saves() {
  let fixture = Fixture::new();
  let mut file = fixture.write("");
  let mut stale = file.clone();
  let mut edited = file.config().clone();
  edited.font.size = 22.0;
  file.save(&edited).unwrap();
  assert!(stale.save(&edited).unwrap_err().contains("Reload settings"));
  assert_eq!(stale.config().font.size, 18.0);
  assert_eq!(file.config().font.size, 22.0);
}

#[test]
fn editor_rejects_non_roundtripping_default_key_removal() {
  let fixture = Fixture::new();
  let mut file = fixture.write("");
  let original = fs::read(fixture.path()).unwrap();
  let mut edited = file.config().clone();
  edited.keybindings.copy.clear();
  assert!(file.save(&edited).unwrap_err().contains("noop"));
  assert_eq!(fs::read(fixture.path()).unwrap(), original);
}

#[test]
fn editor_invalid_document_load_is_read_only() {
  let fixture = Fixture::new();
  let original = "[font]\nsize = 'invalid'\n";
  fs::write(fixture.path(), original).unwrap();
  assert!(ConfigFile::load_from_path(fixture.path()).is_err());
  assert_eq!(fs::read_to_string(fixture.path()).unwrap(), original);
  assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 1);
}

#[test]
fn editor_read_only_save_failure_keeps_snapshot_and_cleans_staging() {
  let fixture = Fixture::new();
  let mut file = fixture.write("");
  let original = fs::read(fixture.path()).unwrap();
  let permissions = fs::metadata(fixture.path()).unwrap().permissions();
  let mut read_only = permissions.clone();
  read_only.set_readonly(true);
  fs::set_permissions(fixture.path(), read_only).unwrap();
  let mut edited = file.config().clone();
  edited.font.size = 21.0;
  let result = file.save(&edited);
  fs::set_permissions(fixture.path(), permissions).unwrap();
  assert!(result.unwrap_err().contains("read-only"));
  assert_eq!(fs::read(fixture.path()).unwrap(), original);
  assert_eq!(file.config().font.size, 18.0);
  fixture.assert_no_staging_files();
  file.save(&edited).unwrap();
  assert_eq!(file.config().font.size, 21.0);
}

#[cfg(unix)]
#[test]
fn editor_save_preserves_private_file_permissions() {
  use std::os::unix::fs::PermissionsExt as _;
  let fixture = Fixture::new();
  let mut file = fixture.write("");
  fs::set_permissions(fixture.path(), fs::Permissions::from_mode(0o600)).unwrap();
  let mut edited = file.config().clone();
  edited.font.size = 21.0;
  file.save(&edited).unwrap();
  assert_eq!(
    fs::metadata(fixture.path()).unwrap().permissions().mode() & 0o777,
    0o600
  );
}
