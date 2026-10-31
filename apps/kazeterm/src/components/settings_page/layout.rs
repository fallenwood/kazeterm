use gpui::{prelude::FluentBuilder, *};
use gpui_kit::component::{
  ActiveTheme, Disableable, IconName, Selectable, Sizable,
  button::{Button, ButtonCustomVariant, ButtonVariants},
  h_flex,
  input::Input,
  v_flex,
};

use super::{Config, SIDEBAR_WIDTH, Section, SettingsPage, chrome};

const NAVIGATION_GROUPS: &[(&str, &[Section])] = &[
  (
    "Workspace",
    &[Section::Startup, Section::Tabs, Section::Panes],
  ),
  (
    "Terminal",
    &[
      Section::Terminal,
      Section::Profiles,
      Section::Font,
      Section::Cursor,
      Section::Keybindings,
    ],
  ),
  (
    "Preferences",
    &[
      Section::Appearance,
      Section::Notifications,
      Section::Animation,
      Section::Updates,
    ],
  ),
  ("Advanced", &[Section::Environment, Section::Imports]),
];

impl SettingsPage {
  pub(super) fn select_section(
    &mut self,
    section: Section,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    self.section = section;
    self
      .search
      .update(cx, |input, cx| input.set_value("", window, cx));
    self.scroll.set_offset(Point::default());
    cx.notify();
  }

  pub(super) fn render_header(&self, cx: &mut Context<Self>) -> Div {
    chrome::toolbar(cx)
      .py_2()
      .child(
        div()
          .w(px(SIDEBAR_WIDTH))
          .flex_shrink_0()
          .px_3()
          .border_r_1()
          .border_color(cx.theme().transparent)
          .child(
            chrome::toolbar_button("settings-back")
              .debug_selector(|| "settings-back".into())
              .icon(IconName::ArrowLeft)
              .label("Back")
              .w_full()
              .disabled(self.busy)
              .on_click(cx.listener(|this, _, window, cx| this.request_close(window, cx))),
          ),
      )
      .child(
        div()
          .px_5()
          .font_weight(FontWeight::SEMIBOLD)
          .child("Settings"),
      )
  }

  pub(super) fn render_sidebar(&self, query: &str, cx: &mut Context<Self>) -> Div {
    v_flex()
      .w(px(SIDEBAR_WIDTH))
      .flex_shrink_0()
      .h_full()
      .min_h_0()
      .p_3()
      .gap_3()
      .bg(cx.theme().title_bar)
      .border_r_1()
      .border_color(cx.theme().border)
      .child(
        v_flex()
          .id("settings-navigation")
          .debug_selector(|| "settings-navigation".into())
          .flex_1()
          .min_h_0()
          .gap_4()
          .overflow_y_scroll()
          .children(NAVIGATION_GROUPS.iter().map(|(label, sections)| {
            v_flex()
              .flex_shrink_0()
              .gap_1()
              .child(
                div()
                  .px_2()
                  .pb_1()
                  .text_xs()
                  .font_weight(FontWeight::SEMIBOLD)
                  .text_color(cx.theme().muted_foreground)
                  .child(*label),
              )
              .children(sections.iter().copied().map(|section| {
                let selected = query.is_empty() && section == self.section;
                let theme = cx.theme();
                let variant = ButtonCustomVariant::new(cx)
                  .color(if selected {
                    theme.tab_active
                  } else {
                    theme.tab
                  })
                  .foreground(if selected {
                    theme.foreground
                  } else {
                    theme.muted_foreground
                  })
                  .hover(if selected {
                    theme.tab_active
                  } else {
                    theme.secondary_hover
                  })
                  .active(if selected {
                    theme.tab_active
                  } else {
                    theme.secondary_active
                  });
                let indicator = cx
                  .global::<themeing::SettingsStore>()
                  .theme()
                  .colors()
                  .text_accent;
                Button::new(("settings-section", section as usize))
                  .debug_selector(move || format!("settings-section-{}", section as usize))
                  .accessibility_label(section.label())
                  .custom(variant)
                  .selected(selected)
                  .small()
                  .w_full()
                  .child(div().w_full().text_left().child(section.label()))
                  .map(|button| chrome::navigation_item(button, selected, true, indicator, cx))
                  .on_click(cx.listener(move |this, _, window, cx| {
                    this.select_section(section, window, cx);
                  }))
              }))
          })),
      )
      .child(
        chrome::toolbar_button("settings-open-toml")
          .debug_selector(|| "settings-open-toml".into())
          .label("Open TOML file")
          .w_full()
          .on_click(cx.listener(|this, _, _, cx| {
            if let Some(file) = &this.file {
              cx.open_url(&format!("file://{}", file.path().display()));
            } else if let Some(path) = Config::get_config_file_path() {
              cx.open_url(&format!("file://{}", path.display()));
            }
          })),
      )
  }

  pub(super) fn render_section_header(
    &self,
    query: &str,
    window: &Window,
    cx: &mut Context<Self>,
  ) -> Div {
    let stacked = window.viewport_size().width < px(1000.0);
    h_flex()
      .flex_shrink_0()
      .min_w_0()
      .justify_between()
      .gap_4()
      .px_5()
      .py_3()
      .when(stacked, |header| header.flex_col().items_start())
      .child(
        div()
          .debug_selector(|| "settings-section-title".into())
          .text_xl()
          .font_weight(FontWeight::SEMIBOLD)
          .child(if query.is_empty() {
            self.section.label()
          } else {
            "Search results"
          }),
      )
      .child(
        div()
          .debug_selector(|| "settings-search".into())
          .min_w_0()
          .when(stacked, |search| search.w_full())
          .when(!stacked, |search| search.w(px(280.0)))
          .child(
            Input::new(&self.search)
              .aria_label("Search settings")
              .prefix(IconName::Search)
              .w_full()
              .when(!query.is_empty(), |input| {
                input.suffix(
                  chrome::toolbar_button("settings-clear-search")
                    .debug_selector(|| "settings-clear-search".into())
                    .icon(IconName::Close)
                    .accessibility_label("Clear search")
                    .tooltip("Clear search")
                    .ghost()
                    .xsmall()
                    .on_click(cx.listener(|this, _, window, cx| {
                      this.select_section(this.section, window, cx);
                    })),
                )
              }),
          ),
      )
  }

  pub(super) fn render_empty_state(&self, query: &str, cx: &mut Context<Self>) -> Div {
    v_flex()
      .debug_selector(|| "settings-empty-state".into())
      .w_full()
      .items_center()
      .gap_3()
      .py_8()
      .px_4()
      .text_color(cx.theme().muted_foreground)
      .child(
        div()
          .font_weight(FontWeight::SEMIBOLD)
          .text_color(cx.theme().foreground)
          .child(if self.form.is_none() {
            if self.busy {
              "Loading settings…"
            } else {
              "Settings are unavailable. Reload to try again."
            }
          } else {
            "No matching settings"
          }),
      )
      .when(!query.is_empty(), |empty| {
        empty.child(
          Button::new("settings-reset-search")
            .label("Clear search")
            .outline()
            .small()
            .on_click(cx.listener(|this, _, window, cx| {
              this.select_section(this.section, window, cx);
            })),
        )
      })
  }
}

#[cfg(test)]
mod tests {
  use super::{NAVIGATION_GROUPS, Section};

  #[test]
  fn settings_navigation_groups_cover_every_section_once() {
    let sections = NAVIGATION_GROUPS
      .iter()
      .flat_map(|(_, sections)| sections.iter().copied())
      .collect::<Vec<_>>();
    assert_eq!(sections.len(), Section::ALL.len());
    for section in Section::ALL {
      assert_eq!(sections.iter().filter(|&&item| item == section).count(), 1);
    }
  }
}
