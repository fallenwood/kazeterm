use std::collections::{BTreeSet, HashSet};

use ::config::{Config, KeybindingConfig, ParsedKeybinding, Profile};
use gpui::*;
use gpui_kit::component::{
  ActiveTheme, Disableable, IconName, Sizable,
  button::{Button, ButtonVariants},
  h_flex,
  menu::PopupMenuItem,
  v_flex,
};

use super::{EditorInput, SettingsForm, SettingsPage, fields::Section, settings_dropdown};

pub(super) struct ProfileEditor {
  id: usize,
  pub(super) name: EditorInput,
  shell: EditorInput,
  directory: EditorInput,
  args: Vec<EditorInput>,
}

pub(super) struct BindingEditor {
  id: usize,
  key: EditorInput,
  action: String,
}

pub(super) struct EnvironmentEditor {
  id: usize,
  name: EditorInput,
  value: EditorInput,
}

pub(super) struct ImportEditor {
  id: usize,
  path: EditorInput,
}

impl SettingsForm {
  fn id(&mut self) -> usize {
    let id = self.next_id;
    self.next_id += 1;
    id
  }

  pub(super) fn populate_collections(
    &mut self,
    config: &Config,
    window: &mut Window,
    cx: &mut Context<SettingsPage>,
  ) -> Result<(), String> {
    for profile in &config.profiles {
      self.push_profile(profile, window, cx);
    }
    let bindings = toml::Value::try_from(&config.keybindings).map_err(|error| error.to_string())?;
    for (key, action) in bindings.as_table().ok_or("Keybindings must be a table.")? {
      let id = self.id();
      self.bindings.push(BindingEditor {
        id,
        key: EditorInput::new(key.clone(), "Shortcut", window, cx),
        action: action
          .as_str()
          .ok_or("A keybinding action must be text.")?
          .into(),
      });
    }
    let mut env: Vec<_> = config.terminal.env.iter().collect();
    env.sort_by_key(|(key, _)| *key);
    for (name, value) in env {
      let id = self.id();
      self.environment.push(EnvironmentEditor {
        id,
        name: EditorInput::new(name.clone(), "Variable name", window, cx),
        value: EditorInput::new(value.clone(), "Value", window, cx),
      });
    }
    for path in &config.imports {
      let id = self.id();
      self.imports.push(ImportEditor {
        id,
        path: EditorInput::new(path.clone(), "Config path", window, cx),
      });
    }
    Ok(())
  }

  fn push_profile(
    &mut self,
    profile: &Profile,
    window: &mut Window,
    cx: &mut Context<SettingsPage>,
  ) {
    let id = self.id();
    self.profiles.push(ProfileEditor {
      id,
      name: EditorInput::new(profile.name.clone(), "Profile name", window, cx),
      shell: EditorInput::new(profile.shell.clone(), "Shell executable", window, cx),
      directory: EditorInput::new(
        profile.working_directory.clone().unwrap_or_default(),
        "Working directory (optional)",
        window,
        cx,
      ),
      args: profile
        .args
        .iter()
        .map(|arg| EditorInput::new(arg.clone(), "Argument", window, cx))
        .collect(),
    });
  }

  pub(super) fn write_collections(&self, value: &mut toml::Value, cx: &App) -> Result<(), String> {
    let mut names = HashSet::new();
    let mut profiles = Vec::new();
    for profile in &self.profiles {
      let name = profile.name.value(cx);
      let shell = profile.shell.value(cx);
      if name.trim().is_empty() || shell.trim().is_empty() {
        return Err("Profiles: each profile needs a name and shell executable.".into());
      }
      if !names.insert(name.clone()) {
        return Err(format!("Profiles: duplicate name '{name}'."));
      }
      let directory = profile.directory.value(cx);
      profiles.push(Profile {
        name,
        shell,
        args: profile.args.iter().map(|arg| arg.value(cx)).collect(),
        working_directory: (!directory.is_empty()).then_some(directory),
      });
    }
    value["profiles"] = toml::Value::try_from(&profiles).map_err(|error| error.to_string())?;

    let mut env = toml::map::Map::new();
    for entry in &self.environment {
      let name = entry.name.value(cx);
      let entry_value = entry.value.value(cx);
      if name.trim().is_empty() || name.contains(['=', '\0']) || entry_value.contains('\0') {
        return Err(
          "Environment: names must be nonempty without '=' or NUL; values cannot contain NUL."
            .into(),
        );
      }
      #[cfg(target_os = "windows")]
      if env
        .keys()
        .any(|key: &String| key.eq_ignore_ascii_case(&name))
      {
        return Err(format!("Environment: duplicate variable '{name}'."));
      }
      if env
        .insert(name.clone(), toml::Value::String(entry_value))
        .is_some()
      {
        return Err(format!("Environment: duplicate variable '{name}'."));
      }
    }
    value["terminal"]["env"] = toml::Value::Table(env);

    let mut imports = Vec::new();
    let mut paths = HashSet::new();
    for entry in &self.imports {
      let path = entry.path.value(cx);
      if path.trim().is_empty() {
        return Err("Imports: enter a config file path or remove the empty row.".into());
      }
      if !paths.insert(path.clone()) {
        return Err(format!("Imports: duplicate path '{path}'."));
      }
      imports.push(toml::Value::String(path));
    }
    value["imports"] = toml::Value::Array(imports);

    let bindings = self
      .bindings
      .iter()
      .map(|entry| (entry.key.value(cx), entry.action.clone()))
      .collect::<Vec<_>>();
    value["keybindings"] = build_bindings(&self.base["keybindings"], &bindings)?;
    Ok(())
  }
}

fn build_bindings(base: &toml::Value, rows: &[(String, String)]) -> Result<toml::Value, String> {
  // Deserialization restores default bindings. Explicit noops keep removed or renamed keys disabled.
  let mut bindings = base
    .as_table()
    .ok_or("Keybindings must be a table.")?
    .keys()
    .map(|key| (key.clone(), toml::Value::String("noop".into())))
    .collect::<toml::map::Map<_, _>>();
  let mut identities = HashSet::new();
  for (key, action) in rows {
    let key = key.trim().to_lowercase();
    let parsed = ParsedKeybinding::parse(&key);
    if parsed.key.is_empty()
      || parsed.key.chars().any(char::is_whitespace)
      || (parsed.key.len() > 1 && parsed.key.contains('-'))
    {
      return Err(format!(
        "Keybindings: '{key}' is not a valid shortcut. Use e.g. ctrl-shift-c."
      ));
    }
    let identity = (
      parsed.control,
      parsed.shift,
      parsed.alt,
      parsed.platform,
      parsed.key,
    );
    if !identities.insert(identity) {
      return Err(format!("Keybindings: duplicate shortcut '{key}'."));
    }
    bindings.insert(key, toml::Value::String(action.clone()));
  }
  let value = toml::Value::Table(bindings);
  value
    .clone()
    .try_into::<KeybindingConfig>()
    .map_err(|error| format!("Keybindings: {error}"))?;
  Ok(value)
}

fn actions() -> Vec<String> {
  // Default serialization is key-first, so multiple shortcuts may name the same action.
  let defaults = toml::Value::try_from(KeybindingConfig::default())
    .expect("keybinding defaults are serializable");
  let mut names: BTreeSet<String> = defaults
    .as_table()
    .expect("keybindings serialize as a table")
    .values()
    .filter_map(|action| action.as_str().map(str::to_string))
    .collect();
  names.insert("noop".into());
  names.into_iter().collect()
}

fn labeled_input(label: &str, input: &EditorInput, disabled: bool, cx: &App) -> Div {
  v_flex()
    .gap_1()
    .min_w_0()
    .child(
      div()
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .child(label.to_string()),
    )
    .child(input.render(label, disabled))
}

impl SettingsPage {
  pub(super) fn render_collections(
    &self,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) -> Vec<AnyElement> {
    let Some(form) = &self.form else {
      return Vec::new();
    };
    let busy = self.busy;
    let mut rows = Vec::new();
    match self.section {
      Section::Profiles => {
        for (ix, profile) in form.profiles.iter().enumerate() {
          let id = profile.id;
          let args = profile.args.iter().enumerate().map(|(arg_ix, arg)| {
            h_flex()
              .gap_2()
              .child(
                div()
                  .flex_1()
                  .min_w_0()
                  .child(arg.render("Argument (one value per row)", busy)),
              )
              .child(
                Button::new(SharedString::from(format!(
                  "settings-remove-arg-{id}-{arg_ix}"
                )))
                .icon(IconName::Close)
                .ghost()
                .small()
                .disabled(busy)
                .on_click(cx.listener(move |this, _, _, cx| {
                  if let Some(form) = &mut this.form {
                    form.profiles[ix].args.remove(arg_ix);
                  }
                  this.changed(cx);
                })),
              )
          });
          rows.push(
            v_flex()
              .gap_3()
              .p_4()
              .rounded_md()
              .border_1()
              .border_color(cx.theme().border)
              .child(
                h_flex()
                  .justify_between()
                  .gap_2()
                  .child(
                    div()
                      .font_weight(FontWeight::SEMIBOLD)
                      .child(format!("Profile {}", ix + 1)),
                  )
                  .child(
                    Button::new(("settings-remove-profile", id))
                      .label("Remove")
                      .danger()
                      .small()
                      .disabled(busy)
                      .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(form) = &mut this.form {
                          form.profiles.remove(ix);
                        }
                        this.changed(cx);
                      })),
                  ),
              )
              .child(labeled_input("Name", &profile.name, busy, cx))
              .child(labeled_input("Shell executable", &profile.shell, busy, cx))
              .child(labeled_input(
                "Working directory (optional)",
                &profile.directory,
                busy,
                cx,
              ))
              .child(
                div()
                  .text_sm()
                  .child("Arguments (one value per row; no shell quoting required)"),
              )
              .children(args)
              .child(
                Button::new(("settings-add-arg", id))
                  .label("Add argument")
                  .small()
                  .disabled(busy)
                  .on_click(cx.listener(move |this, _, window, cx| {
                    if let Some(form) = &mut this.form {
                      form.profiles[ix].args.push(EditorInput::new(
                        String::new(),
                        "Argument",
                        window,
                        cx,
                      ));
                    }
                    this.changed(cx);
                  })),
              )
              .into_any_element(),
          );
        }
        rows.push(
          Button::new("settings-add-profile")
            .label("Add profile")
            .icon(IconName::Plus)
            .disabled(busy)
            .on_click(cx.listener(|this, _, window, cx| {
              if let Some(form) = &mut this.form {
                form.push_profile(
                  &Profile {
                    name: String::new(),
                    shell: String::new(),
                    args: Vec::new(),
                    working_directory: None,
                  },
                  window,
                  cx,
                );
              }
              this.changed(cx);
            }))
            .into_any_element(),
        );
        rows.push(div().text_sm().text_color(cx.theme().muted_foreground)
          .child("Container and SSH profiles are discovered automatically. The default profile is selected under Startup.").into_any_element());
      }
      Section::Keybindings => {
        let action_names = actions();
        rows.push(div().text_sm().text_color(cx.theme().muted_foreground)
          .child("Each shortcut maps to an action. Removing or changing a shortcut disables its old assignment; noop explicitly disables a shortcut.").into_any_element());
        for (ix, binding) in form.bindings.iter().enumerate() {
          let actions = action_names.clone();
          let view = cx.entity().downgrade();
          rows.push(
            v_flex()
              .gap_2()
              .p_3()
              .rounded_md()
              .border_1()
              .border_color(cx.theme().border)
              .child(labeled_input("Shortcut", &binding.key, busy, cx))
              .child(
                h_flex()
                  .gap_2()
                  .flex_wrap()
                  .child(settings_dropdown(
                    ("settings-binding-action", binding.id),
                    binding.action.clone(),
                    busy,
                    window,
                    cx,
                    move |mut menu, _, _| {
                      for action in &actions {
                        let action = action.clone();
                        let view = view.clone();
                        menu = menu.item(PopupMenuItem::new(action.clone()).on_click(
                          move |_, _, cx| {
                            let _ = view.update(cx, |this, cx| {
                              if let Some(form) = &mut this.form {
                                form.bindings[ix].action = action.clone();
                              }
                              this.changed(cx);
                            });
                          },
                        ));
                      }
                      menu
                    },
                  ))
                  .child(
                    Button::new(("settings-remove-binding", binding.id))
                      .label("Remove")
                      .ghost()
                      .small()
                      .disabled(busy)
                      .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(form) = &mut this.form {
                          form.bindings.remove(ix);
                        }
                        this.changed(cx);
                      })),
                  ),
              )
              .into_any_element(),
          );
        }
        rows.push(
          Button::new("settings-add-binding")
            .label("Add shortcut")
            .icon(IconName::Plus)
            .disabled(busy)
            .on_click(cx.listener(|this, _, window, cx| {
              if let Some(form) = &mut this.form {
                let id = form.id();
                form.bindings.push(BindingEditor {
                  id,
                  key: EditorInput::new(String::new(), "Shortcut", window, cx),
                  action: "noop".into(),
                });
              }
              this.changed(cx);
            }))
            .into_any_element(),
        );
      }
      Section::Environment => {
        for (ix, entry) in form.environment.iter().enumerate() {
          rows.push(
            v_flex()
              .gap_2()
              .p_3()
              .rounded_md()
              .border_1()
              .border_color(cx.theme().border)
              .child(labeled_input("Variable name", &entry.name, busy, cx))
              .child(labeled_input("Value", &entry.value, busy, cx))
              .child(
                Button::new(("settings-remove-env", entry.id))
                  .label("Remove")
                  .ghost()
                  .small()
                  .disabled(busy)
                  .on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(form) = &mut this.form {
                      form.environment.remove(ix);
                    }
                    this.changed(cx);
                  })),
              )
              .into_any_element(),
          );
        }
        rows.push(
          Button::new("settings-add-env")
            .label("Add variable")
            .icon(IconName::Plus)
            .disabled(busy)
            .on_click(cx.listener(|this, _, window, cx| {
              if let Some(form) = &mut this.form {
                let id = form.id();
                form.environment.push(EnvironmentEditor {
                  id,
                  name: EditorInput::new(String::new(), "Variable name", window, cx),
                  value: EditorInput::new(String::new(), "Value", window, cx),
                });
              }
              this.changed(cx);
            }))
            .into_any_element(),
        );
      }
      Section::Imports => {
        rows.push(div().text_sm().text_color(cx.theme().muted_foreground)
          .child("Paths are relative to the main config file, or absolute. Imports override the main file; later entries have higher priority. Imported files are never edited here.").into_any_element());
        for (ix, entry) in form.imports.iter().enumerate() {
          rows.push(
            v_flex()
              .gap_2()
              .p_3()
              .rounded_md()
              .border_1()
              .border_color(cx.theme().border)
              .child(labeled_input(
                &format!("Import {} (TOML path)", ix + 1),
                &entry.path,
                busy,
                cx,
              ))
              .child(
                h_flex()
                  .gap_2()
                  .child(
                    Button::new(("settings-import-up", entry.id))
                      .icon(IconName::ArrowUp)
                      .tooltip("Move earlier")
                      .small()
                      .disabled(busy || ix == 0)
                      .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(form) = &mut this.form {
                          form.imports.swap(ix, ix - 1);
                        }
                        this.changed(cx);
                      })),
                  )
                  .child(
                    Button::new(("settings-import-down", entry.id))
                      .icon(IconName::ArrowDown)
                      .tooltip("Move later")
                      .small()
                      .disabled(busy || ix + 1 == form.imports.len())
                      .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(form) = &mut this.form {
                          form.imports.swap(ix, ix + 1);
                        }
                        this.changed(cx);
                      })),
                  )
                  .child(
                    Button::new(("settings-remove-import", entry.id))
                      .label("Remove")
                      .ghost()
                      .small()
                      .disabled(busy)
                      .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(form) = &mut this.form {
                          form.imports.remove(ix);
                        }
                        this.changed(cx);
                      })),
                  ),
              )
              .into_any_element(),
          );
        }
        rows.push(
          Button::new("settings-add-import")
            .label("Add import")
            .icon(IconName::Plus)
            .disabled(busy)
            .on_click(cx.listener(|this, _, window, cx| {
              if let Some(form) = &mut this.form {
                let id = form.id();
                form.imports.push(ImportEditor {
                  id,
                  path: EditorInput::new(String::new(), "Config path", window, cx),
                });
              }
              this.changed(cx);
            }))
            .into_any_element(),
        );
      }
      _ => {}
    }
    rows
  }
}

#[cfg(test)]
mod tests {
  use super::{EditorInput, EnvironmentEditor, ImportEditor, SettingsPage, build_bindings};
  use config::{Config, KeybindingConfig, Profile};
  use gpui::TestAppContext;

  #[test]
  fn settings_bindings_disable_removed_defaults_and_validate_duplicates() {
    let base = toml::Value::try_from(KeybindingConfig::default()).unwrap();
    let key = base.as_table().unwrap().keys().next().unwrap().clone();
    let value = build_bindings(&base, &[("ctrl-alt-f9".into(), "new_tab".into())]).unwrap();
    assert_eq!(value[&key].as_str(), Some("noop"));
    let bindings: KeybindingConfig = value.try_into().unwrap();
    assert!(bindings.new_tab.iter().any(|value| value == "ctrl-alt-f9"));
    assert!(bindings.noop.iter().any(|value| value == key));
    assert!(
      build_bindings(
        &base,
        &[
          ("ctrl-a".into(), "copy".into()),
          ("ctrl-a".into(), "paste".into())
        ]
      )
      .is_err()
    );
    assert!(
      build_bindings(
        &base,
        &[
          ("cmd-a".into(), "copy".into()),
          ("super-a".into(), "paste".into())
        ]
      )
      .is_err()
    );
    assert!(build_bindings(&base, &[("ctrl-".into(), "copy".into())]).is_err());
    assert!(build_bindings(&base, &[("ctrl-a".into(), "unknown".into())]).is_err());
  }

  #[gpui::test]
  fn settings_collection_edits_preserve_argument_boundaries_and_import_order(
    cx: &mut TestAppContext,
  ) {
    crate::test_support::init_test_app(cx);
    let window = cx.add_window(|window, cx| {
      let mut page = SettingsPage::new(window, cx);
      let mut config = cx.global::<Config>().clone();
      config.profiles = vec![Profile {
        name: "Shell".into(),
        shell: "shell executable".into(),
        args: vec![
          "--command".into(),
          "argument with spaces".into(),
          String::new(),
        ],
        working_directory: None,
      }];
      config.imports = vec!["first.toml".into(), "second.toml".into()];
      page.form = Some(page.make_form(&config, window, cx).unwrap());
      page
    });
    window
      .update(cx, |page, window, cx| {
        let form = page.form.as_mut().unwrap();
        form.imports.swap(0, 1);
        let id = form.id();
        form.environment.push(EnvironmentEditor {
          id,
          name: EditorInput::new("EDITOR".into(), "Name", window, cx),
          value: EditorInput::new("editor --wait".into(), "Value", window, cx),
        });
        let draft = page.build_config(cx).unwrap();
        assert_eq!(draft.imports, ["second.toml", "first.toml"]);
        assert_eq!(
          draft.profiles[0].args,
          ["--command", "argument with spaces", ""]
        );
        assert_eq!(draft.terminal.env["EDITOR"], "editor --wait");
        assert_eq!(cx.global::<Config>().terminal.env.get("EDITOR"), None);

        let form = page.form.as_mut().unwrap();
        let id = form.id();
        form.imports.push(ImportEditor {
          id,
          path: EditorInput::new(String::new(), "Path", window, cx),
        });
        assert!(page.build_config(cx).unwrap_err().contains("Imports"));
        page.form.as_mut().unwrap().imports.pop();
        let form = page.form.as_mut().unwrap();
        let id = form.id();
        form.environment.push(EnvironmentEditor {
          id,
          name: EditorInput::new("EDITOR".into(), "Name", window, cx),
          value: EditorInput::new(String::new(), "Value", window, cx),
        });
        assert!(page.build_config(cx).unwrap_err().contains("duplicate"));
        page.form.as_mut().unwrap().environment.pop();
        page.form.as_mut().unwrap().profiles[0]
          .name
          .state
          .update(cx, |input, cx| {
            input.set_value("", window, cx);
          });
        assert!(page.build_config(cx).unwrap_err().contains("Profiles"));
      })
      .unwrap();
  }
}
