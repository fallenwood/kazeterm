mod collections;
mod fields;
#[cfg(test)]
mod tests;

use ::config::{Config, ConfigFile};
use gpui::{prelude::FluentBuilder, *};
use gpui_kit::component::{
  ActiveTheme, Disableable, ElementExt, IconName, Sizable,
  button::{Button, ButtonVariants},
  h_flex,
  input::{Input, InputEvent, InputState},
  menu::{DropdownMenu, PopupMenu, PopupMenuItem},
  switch::Switch,
  v_flex,
};

use super::menu_builder::scrollable_menu;
use collections::{BindingEditor, EnvironmentEditor, ImportEditor, ProfileEditor};
use fields::{FIELDS, FieldKind, Section};

fn settings_dropdown(
  id: impl Into<ElementId>,
  label: impl Into<SharedString>,
  disabled: bool,
  window: &mut Window,
  cx: &mut App,
  build: impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static,
) -> Div {
  let id = id.into();
  let width = window.use_keyed_state(
    SharedString::from(format!("settings-dropdown-width:{id:?}")),
    cx,
    |_, _| Pixels::ZERO,
  );
  let menu_width = width.clone();
  div()
    .relative()
    .w_full()
    .min_w_0()
    .on_prepaint(move |bounds, _, cx| {
      width.update(cx, |width, _| *width = bounds.size.width);
    })
    .child(
      Button::new(id)
        .label(label)
        .icon(IconName::ChevronDown)
        .small()
        .w_full()
        .disabled(disabled)
        .dropdown_menu(move |menu, window, cx| {
          let width = *menu_width.read(cx);
          build(
            scrollable_menu(menu, window, cx).min_w(width).max_w(width),
            window,
            cx,
          )
        }),
    )
}

pub(super) enum SettingsCloseEvent {
  Back,
  CloseWindow,
}
impl EventEmitter<SettingsCloseEvent> for SettingsPage {}

struct EditorInput {
  state: Entity<InputState>,
  _subscription: Subscription,
}

impl EditorInput {
  fn new(value: String, label: &str, window: &mut Window, cx: &mut Context<SettingsPage>) -> Self {
    let state = cx.new(|cx| {
      InputState::new(window, cx)
        .placeholder(label.to_string())
        .default_value(value)
    });
    let subscription = cx.subscribe(&state, |this, _, event, cx| {
      if matches!(event, InputEvent::Change) {
        this.changed(cx);
      }
    });
    Self {
      state,
      _subscription: subscription,
    }
  }

  fn value(&self, cx: &App) -> String {
    self.state.read(cx).value().to_string()
  }

  fn render(&self, label: &str, disabled: bool) -> Input {
    Input::new(&self.state)
      .aria_label(label.to_string())
      .disabled(disabled)
      .w_full()
      .small()
  }
}

struct SettingsForm {
  base: toml::Value,
  fields: Vec<EditorInput>,
  profiles: Vec<ProfileEditor>,
  bindings: Vec<BindingEditor>,
  environment: Vec<EnvironmentEditor>,
  imports: Vec<ImportEditor>,
  next_id: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PendingAction {
  Close,
  CloseWindow,
  Reload,
  Revert,
}

pub(crate) struct SettingsPage {
  focus_handle: FocusHandle,
  file: Option<ConfigFile>,
  form: Option<SettingsForm>,
  section: Section,
  search: Entity<InputState>,
  _search_subscription: Subscription,
  scroll: ScrollHandle,
  themes: Vec<String>,
  dirty: bool,
  busy: bool,
  error: Option<String>,
  status: String,
  pending: Option<PendingAction>,
  task: Task<()>,
}

impl SettingsPage {
  pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
    let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search settings"));
    let subscription = cx.subscribe(&search, |this, _, event, cx| {
      if matches!(event, InputEvent::Change) {
        this.scroll.set_offset(point(px(0.0), px(0.0)));
        cx.notify();
      }
    });
    Self {
      focus_handle: cx.focus_handle(),
      file: None,
      form: None,
      section: Section::Startup,
      search,
      _search_subscription: subscription,
      scroll: ScrollHandle::new(),
      themes: Vec::new(),
      dirty: false,
      busy: false,
      error: None,
      status: String::new(),
      pending: None,
      task: Task::ready(()),
    }
  }

  pub(super) fn load(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    self.busy = true;
    self.pending = None;
    self.error = None;
    self.status = "Loading settings...".into();
    cx.notify();
    self.task = cx.spawn_in(window, async move |this, cx| {
      let (result, themes) =
        smol::unblock(|| (ConfigFile::load(), ::config::list_available_themes())).await;
      let _ = this.update_in(cx, |this, window, cx| {
        this.busy = false;
        this.themes = themes;
        match result {
          Ok(file) => this.install_file(file, window, cx),
          Err(error) => this.show_error(error, cx),
        }
      });
    });
  }

  fn install_file(&mut self, file: ConfigFile, window: &mut Window, cx: &mut Context<Self>) {
    match self.make_form(file.config(), window, cx) {
      Ok(form) => {
        self.form = Some(form);
        self.file = Some(file);
        self.dirty = false;
        self.pending = None;
        self.error = None;
        self.status.clear();
        cx.notify();
      }
      Err(error) => self.show_error(error, cx),
    }
  }

  fn make_form(
    &self,
    config: &Config,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) -> Result<SettingsForm, String> {
    let base = toml::Value::try_from(config).map_err(|error| error.to_string())?;
    let fields = FIELDS
      .iter()
      .map(|spec| {
        let input = EditorInput::new(spec.value(&base), spec.label, window, cx);
        if matches!(spec.kind, FieldKind::Number { .. }) {
          input.state.update(cx, |state, cx| {
            state.set_validator(move |text, _| spec.accepts_input(text), cx);
          });
        }
        input
      })
      .collect();
    let mut form = SettingsForm {
      base,
      fields,
      profiles: Vec::new(),
      bindings: Vec::new(),
      environment: Vec::new(),
      imports: Vec::new(),
      next_id: 0,
    };
    form.populate_collections(config, window, cx)?;
    Ok(form)
  }

  fn changed(&mut self, cx: &mut Context<Self>) {
    self.dirty = true;
    self.error = None;
    self.status = "Unsaved changes".into();
    cx.notify();
  }

  fn show_error(&mut self, error: String, cx: &mut Context<Self>) {
    tracing::warn!("Settings: {error}");
    self.status.clear();
    self.error = Some(error);
    cx.notify();
  }

  fn build_config(&self, cx: &App) -> Result<Config, String> {
    let form = self.form.as_ref().ok_or("Settings have not been loaded.")?;
    let mut value = form.base.clone();
    for (spec, input) in FIELDS.iter().zip(&form.fields) {
      let text = input.value(cx);
      if text != spec.value(&form.base) {
        spec.write(&mut value, &text)?;
      }
    }
    form.write_collections(&mut value, cx)?;
    let config: Config = value
      .try_into()
      .map_err(|error: toml::de::Error| error.to_string())?;
    if config.imports.is_empty()
      && let Some(default) = &config.terminal.default_profile
      && !config
        .profiles
        .iter()
        .any(|profile| &profile.name == default)
    {
      return Err(format!(
        "Startup: default profile '{default}' no longer exists. Select another profile or Automatic."
      ));
    }
    Ok(config)
  }

  fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    if self.busy {
      return;
    }
    let edited = match self.build_config(cx) {
      Ok(config) => config,
      Err(error) => {
        self.show_error(error, cx);
        return;
      }
    };
    let Some(mut file) = self.file.take() else {
      self.show_error("Reload the settings file before saving.".into(), cx);
      return;
    };
    self.busy = true;
    self.error = None;
    self.status = "Saving settings...".into();
    cx.notify();
    self.task = cx.spawn_in(window, async move |this, cx| {
      let (file, result, reloaded) = smol::unblock(move || {
        let result = file.save(&edited);
        let reloaded = result.as_ref().ok().map(|_| file.load_effective());
        (file, result, reloaded)
      })
      .await;
      let _ = this.update_in(cx, |this, window, cx| {
        this.busy = false;
        if let Err(error) = result {
          this.file = Some(file);
          this.show_error(error, cx);
          return;
        }
        let pending = this.pending;
        this.install_file(file, window, cx);
        match reloaded {
          Some(Ok(config)) => {
            // Applying configuration updates all windows; defer until this window update ends.
            cx.defer(move |cx| crate::config_watcher::apply_loaded_config(config, cx));
            this.status = "Saved and applied.".into();
            if let Some(action @ (PendingAction::Close | PendingAction::CloseWindow)) = pending {
              this.perform(action, window, cx);
            }
          }
          Some(Err(error)) => this.show_error(format!("Saved, but could not reload: {error}"), cx),
          None => unreachable!("a successful save always reloads configuration"),
        }
        cx.notify();
      });
    });
  }

  pub(super) fn request_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    self.request(PendingAction::Close, window, cx);
  }

  pub(super) fn request_window_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    self.request(PendingAction::CloseWindow, window, cx);
  }

  fn request(&mut self, action: PendingAction, window: &mut Window, cx: &mut Context<Self>) {
    if self.busy {
      return;
    }
    if self.dirty {
      self.pending = Some(action);
      cx.notify();
    } else {
      self.perform(action, window, cx);
    }
  }

  fn perform(&mut self, action: PendingAction, window: &mut Window, cx: &mut Context<Self>) {
    self.pending = None;
    match action {
      PendingAction::Close => cx.emit(SettingsCloseEvent::Back),
      PendingAction::CloseWindow => cx.emit(SettingsCloseEvent::CloseWindow),
      PendingAction::Reload => self.load(window, cx),
      PendingAction::Revert => {
        if let Some(config) = self.file.as_ref().map(|file| file.config().clone()) {
          match self.make_form(&config, window, cx) {
            Ok(form) => {
              self.form = Some(form);
              self.dirty = false;
              self.error = None;
              self.status = "Changes reverted.".into();
            }
            Err(error) => self.show_error(error, cx),
          }
        }
      }
    }
    cx.notify();
  }

  fn set_field(&mut self, ix: usize, value: String, window: &mut Window, cx: &mut Context<Self>) {
    if let Some(form) = &self.form {
      form.fields[ix]
        .state
        .update(cx, |state, cx| state.set_value(value, window, cx));
      self.changed(cx);
    }
  }

  fn choices(&self, kind: FieldKind, cx: &App) -> Vec<String> {
    match kind {
      FieldKind::Choice(values) => values.iter().map(|value| value.to_string()).collect(),
      FieldKind::Theme => self.themes.clone(),
      FieldKind::DefaultProfile => std::iter::once(String::new())
        .chain(
          self
            .form
            .iter()
            .flat_map(|form| form.profiles.iter().map(|profile| profile.name.value(cx))),
        )
        .collect(),
      _ => Vec::new(),
    }
  }

  fn render_fields(
    &self,
    query: &str,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) -> Vec<AnyElement> {
    let Some(form) = &self.form else {
      return Vec::new();
    };
    FIELDS
      .iter()
      .enumerate()
      .filter(|(_, spec)| {
        if query.is_empty() {
          spec.section == self.section
        } else {
          format!(
            "{} {} {} {} {}",
            spec.section.label(),
            spec.table,
            spec.key,
            spec.label,
            spec.description
          )
          .to_lowercase()
          .contains(query)
        }
      })
      .map(|(ix, spec)| {
        let input = &form.fields[ix];
        let value = input.value(cx);
        let control = match spec.kind {
          FieldKind::Bool => Switch::new(("settings-toggle", ix))
            .accessibility_label(spec.label)
            .checked(value == "true")
            .disabled(self.busy)
            .on_click(cx.listener(move |this, value: &bool, window, cx| {
              this.set_field(ix, value.to_string(), window, cx);
            }))
            .into_any_element(),
          FieldKind::Choice(_) | FieldKind::DefaultProfile => self
            .choice_button(ix, &value, window, cx)
            .into_any_element(),
          FieldKind::Theme => h_flex()
            .w_full()
            .gap_2()
            .child(input.render(spec.label, self.busy))
            .child(self.choice_button(ix, &value, window, cx))
            .into_any_element(),
          _ => input.render(spec.label, self.busy).into_any_element(),
        };
        v_flex()
          .debug_selector(move || format!("settings-field-{}-{}", spec.table, spec.key))
          .gap_2()
          .p_4()
          .rounded_md()
          .border_1()
          .border_color(cx.theme().border)
          .child(div().font_weight(FontWeight::SEMIBOLD).child(spec.label))
          .child(div().w_full().child(control))
          .into_any_element()
      })
      .collect()
  }

  fn choice_button(
    &self,
    ix: usize,
    value: &str,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) -> AnyElement {
    let choices = self.choices(FIELDS[ix].kind, cx);
    let view = cx.entity().downgrade();
    settings_dropdown(
      ("settings-choice", ix),
      if value.is_empty() {
        "Automatic".to_string()
      } else {
        value.to_string()
      },
      self.busy,
      window,
      cx,
      move |mut menu, _, _| {
        for choice in &choices {
          let choice = choice.clone();
          let view = view.clone();
          menu = menu.item(
            PopupMenuItem::new(if choice.is_empty() {
              "Automatic".to_string()
            } else {
              choice.clone()
            })
            .on_click(move |_, window, cx| {
              let _ = view.update(cx, |this, cx| {
                this.set_field(ix, choice.clone(), window, cx)
              });
            }),
          );
        }
        menu
      },
    )
    .into_any_element()
  }

  fn render_footer(&self, cx: &mut Context<Self>) -> Div {
    if let Some(action) = self.pending {
      return v_flex()
        .debug_selector(|| "settings-footer".into())
        .gap_2()
        .p_3()
        .border_t_1()
        .border_color(cx.theme().border)
        .child("You have unsaved changes.")
        .when_some(self.error.clone(), |footer, error| {
          footer.child(div().text_sm().text_color(cx.theme().danger).child(error))
        })
        .child(
          h_flex()
            .gap_2()
            .flex_wrap()
            .child(
              Button::new("settings-keep-editing")
                .label("Keep editing")
                .small()
                .disabled(self.busy)
                .on_click(cx.listener(|this, _, _, cx| {
                  this.pending = None;
                  cx.notify();
                })),
            )
            .child(
              Button::new("settings-discard")
                .label("Discard changes")
                .danger()
                .small()
                .disabled(self.busy)
                .on_click(cx.listener(move |this, _, window, cx| this.perform(action, window, cx))),
            )
            .when(
              matches!(action, PendingAction::Close | PendingAction::CloseWindow),
              |row| {
                row.child(
                  Button::new("settings-save-close")
                    .label("Save changes")
                    .primary()
                    .small()
                    .disabled(self.busy)
                    .on_click(cx.listener(|this, _, window, cx| this.save(window, cx))),
                )
              },
            ),
        );
    }
    v_flex()
      .debug_selector(|| "settings-footer".into())
      .gap_2()
      .p_3()
      .border_t_1()
      .border_color(cx.theme().border)
      .when_some(self.error.clone(), |footer, error| {
        footer.child(div().text_sm().text_color(cx.theme().danger).child(error))
      })
      .when(!self.status.is_empty(), |footer| {
        footer.child(
          div()
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .child(self.status.clone()),
        )
      })
      .child(
        h_flex()
          .gap_2()
          .flex_wrap()
          .justify_end()
          .child(
            Button::new("settings-reload")
              .label("Reload file")
              .small()
              .disabled(self.busy)
              .on_click(
                cx.listener(|this, _, window, cx| this.request(PendingAction::Reload, window, cx)),
              ),
          )
          .child(
            Button::new("settings-revert")
              .label("Revert changes")
              .small()
              .disabled(self.busy || !self.dirty)
              .on_click(
                cx.listener(|this, _, window, cx| this.request(PendingAction::Revert, window, cx)),
              ),
          )
          .child(
            Button::new("settings-save")
              .label("Save")
              .primary()
              .small()
              .disabled(self.busy || !self.dirty)
              .on_click(cx.listener(|this, _, window, cx| this.save(window, cx))),
          ),
      )
  }
}

impl Focusable for SettingsPage {
  fn focus_handle(&self, _: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl SettingsPage {
  fn render_page(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Div {
    let query = self.search.read(cx).value().trim().to_lowercase();
    let mut content = self.render_fields(&query, window, cx);
    if query.is_empty() {
      content.extend(self.render_collections(window, cx));
    } else {
      for section in [
        Section::Profiles,
        Section::Keybindings,
        Section::Environment,
        Section::Imports,
      ] {
        if section.label().to_lowercase().contains(&query) {
          content.push(
            Button::new(("settings-search-section", section as usize))
              .label(format!("Open {}", section.label()))
              .on_click(cx.listener(move |this, _, window, cx| {
                this.section = section;
                this
                  .search
                  .update(cx, |input, cx| input.set_value("", window, cx));
                cx.notify();
              }))
              .into_any_element(),
          );
        }
      }
    }
    let no_results = content.is_empty() && self.form.is_some();
    v_flex()
      .size_full()
      .min_h_0()
      .bg(cx.theme().background)
      .text_color(cx.theme().foreground)
      .font_family(cx.theme().font_family.clone())
      .text_size(cx.theme().font_size)
      .track_focus(&self.focus_handle)
      .key_context("Settings")
      .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
        let key = event.keystroke.key.to_lowercase();
        let modifiers = event.keystroke.modifiers;
        if key == "escape" {
          if this.pending.is_some() {
            this.pending = None;
            cx.notify();
          } else {
            this.request_close(window, cx);
          }
          cx.stop_propagation();
        } else if key == "s"
          && (modifiers.control || modifiers.platform)
          && !modifiers.alt
          && !modifiers.shift
        {
          this.save(window, cx);
          cx.stop_propagation();
        } else {
          let bindings = &cx.global::<Config>().keybindings;
          if bindings.toggle_fullscreen.matches(
            modifiers.control,
            modifiers.shift,
            modifiers.alt,
            modifiers.platform,
            &key,
          ) {
            window.toggle_fullscreen();
            cx.stop_propagation();
          } else if bindings.quit.matches(
            modifiers.control,
            modifiers.shift,
            modifiers.alt,
            modifiers.platform,
            &key,
          ) {
            this.request_window_close(window, cx);
            cx.stop_propagation();
          }
        }
      }))
      .child(
        h_flex()
          .gap_3()
          .p_3()
          .border_b_1()
          .border_color(cx.theme().border)
          .child(
            Button::new("settings-back")
              .icon(IconName::ArrowLeft)
              .label("Back to terminal")
              .ghost()
              .disabled(self.busy)
              .on_click(cx.listener(|this, _, window, cx| this.request_close(window, cx))),
          )
          .child(
            div()
              .text_lg()
              .font_weight(FontWeight::SEMIBOLD)
              .child("Settings"),
          ),
      )
      .child(
        h_flex()
          .flex_1()
          .min_h_0()
          .min_w_0()
          .child(
            v_flex()
              .w(px(180.0))
              .flex_shrink_0()
              .h_full()
              .p_2()
              .gap_1()
              .border_r_1()
              .border_color(cx.theme().border)
              .child(
                div()
                  .id("settings-navigation")
                  .flex_1()
                  .min_h_0()
                  .overflow_y_scroll()
                  .children(Section::ALL.into_iter().map(|section| {
                    Button::new(("settings-section", section as usize))
                      .label(section.label())
                      .ghost()
                      .w_full()
                      .when(section == self.section, |button| button.primary())
                      .on_click(cx.listener(move |this, _, window, cx| {
                        this.section = section;
                        this
                          .search
                          .update(cx, |input, cx| input.set_value("", window, cx));
                        this.scroll.set_offset(point(px(0.0), px(0.0)));
                        cx.notify();
                      }))
                  })),
              )
              .child(
                Button::new("settings-open-toml")
                  .label("Open TOML file")
                  .ghost()
                  .small()
                  .on_click(cx.listener(|this, _, _, cx| {
                    if let Some(file) = &this.file {
                      cx.open_url(&format!("file://{}", file.path().display()));
                    } else if let Some(path) = Config::get_config_file_path() {
                      cx.open_url(&format!("file://{}", path.display()));
                    }
                  })),
              ),
          )
          .child(
            v_flex()
              .flex_1()
              .h_full()
              .min_w_0()
              .min_h_0()
              .child(
                div()
                  .debug_selector(|| "settings-search".into())
                  .p_3()
                  .child(
                    Input::new(&self.search)
                      .aria_label("Search settings")
                      .w_full(),
                  ),
              )
              .child(
                v_flex()
                  .id("settings-content")
                  .flex_1()
                  .min_h_0()
                  .overflow_y_scroll()
                  .track_scroll(&self.scroll)
                  .p_4()
                  .gap_3()
                  .child(div().text_xl().font_weight(FontWeight::SEMIBOLD).child(
                    if query.is_empty() {
                      self.section.label()
                    } else {
                      "Search results"
                    },
                  ))
                  .when(
                    self
                      .form
                      .as_ref()
                      .is_some_and(|form| !form.imports.is_empty()),
                    |body| {
                      body.child(
                        div()
                          .p_3()
                          .rounded_md()
                          .bg(cx.theme().muted)
                          .text_sm()
                          .child("Imported files may override these settings."),
                      )
                    },
                  )
                  .children(content)
                  .when(no_results, |body| body.child("No matching settings.")),
              )
              .child(self.render_footer(cx)),
          ),
      )
  }
}

impl Render for SettingsPage {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    self.render_page(window, cx)
  }
}
