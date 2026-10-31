use std::collections::HashMap;

use config::{Palette, ThemeFile};
use gpui::{
  App, Context, Div, FontWeight, InteractiveElement, ParentElement, SharedString, Styled, Window,
  WindowAppearance, div, prelude::FluentBuilder, px, relative,
};
use gpui_kit::component::{
  ActiveTheme, Disableable, Icon, IconName, Selectable, Sizable,
  button::{Button, ButtonCustomVariant, ButtonVariants},
  h_flex, v_flex,
};

use super::{FIELDS, SIDEBAR_WIDTH, SettingsPage, chrome};

const CARD_HEIGHT: f32 = 176.0;
const PREVIEW_HEIGHT: f32 = 132.0;
const GRID_GAP: f32 = 12.0;

pub(super) struct ThemePreview {
  id: String,
  name: String,
  palettes: Option<(Palette, Palette)>,
}

impl ThemePreview {
  pub(super) fn load_all() -> Vec<Self> {
    let mut themes = config::list_available_themes()
      .into_iter()
      .map(|id| {
        let file = config::load_theme_from_assets(&id);
        Self::from_file(id, file)
      })
      .collect::<Vec<_>>();
    Self::disambiguate_names(&mut themes);
    themes
  }

  fn disambiguate_names(themes: &mut [Self]) {
    let mut counts = HashMap::<String, usize>::new();
    for theme in themes.iter() {
      *counts.entry(theme.name.to_lowercase()).or_default() += 1;
    }
    for theme in themes {
      if counts[&theme.name.to_lowercase()] > 1 {
        theme.name = format!("{} ({})", theme.name, theme.id);
      }
    }
  }

  fn from_file(id: String, file: Option<ThemeFile>) -> Self {
    let (name, palettes) = match file {
      Some(file) => (
        file.name,
        Some((
          file.dark.to_palette(true),
          file.light.as_ref().unwrap_or(&file.dark).to_palette(false),
        )),
      ),
      None => (id.clone(), None),
    };
    Self { id, name, palettes }
  }

  fn palette(&self, is_dark: bool) -> Option<&Palette> {
    self
      .palettes
      .as_ref()
      .map(|(dark, light)| if is_dark { dark } else { light })
  }
}

fn gallery_columns(window_width: f32) -> u16 {
  // Sidebar, page/panel insets, borders, and a 12-pixel grid gap.
  (((window_width - SIDEBAR_WIDTH - 74.0 + GRID_GAP) / 192.0) as u16).clamp(1, 3)
}

fn preview_is_dark(mode: &str, appearance: WindowAppearance) -> bool {
  match mode {
    "light" => false,
    "dark" => true,
    _ => matches!(
      appearance,
      WindowAppearance::Dark | WindowAppearance::VibrantDark
    ),
  }
}

fn terminal_preview(id: &str, palette: &Palette, cx: &App) -> Div {
  let opaque = |color: gpui::Hsla| gpui::Hsla { a: 1.0, ..color };
  v_flex()
    .debug_selector({
      let id = id.to_string();
      move || format!("settings-theme-sample-{id}")
    })
    .w_full()
    .h(px(PREVIEW_HEIGHT))
    .flex_shrink_0()
    .overflow_hidden()
    .bg(opaque(palette.terminal_background))
    .text_color(opaque(palette.terminal_foreground))
    .child(
      h_flex()
        .w_full()
        .h(px(22.0))
        .flex_shrink_0()
        .px_2()
        .gap_1()
        .bg(opaque(palette.title_bar_background))
        .children(
          [
            palette.terminal_ansi_red,
            palette.terminal_ansi_yellow,
            palette.terminal_ansi_green,
          ]
          .map(|color| div().size(px(5.0)).rounded_full().bg(color)),
        )
        .child(div().ml_2().text_size(px(9.0)).child("kazeterm")),
    )
    .child(
      v_flex()
        .flex_1()
        .w_full()
        .min_w_0()
        .px_3()
        .py_2()
        .gap_1()
        .font_family(cx.theme().mono_font_family.clone())
        .text_size(px(10.0))
        .line_height(relative(1.3))
        .child(
          h_flex()
            .gap_1()
            .child(div().text_color(palette.terminal_ansi_green).child("$"))
            .child("cargo test"),
        )
        .child(
          h_flex()
            .gap_1()
            .child(div().text_color(palette.terminal_ansi_green).child("PASS"))
            .child(div().text_color(palette.text_muted).child("ui / themes")),
        )
        .child(
          h_flex()
            .gap_1()
            .child(div().text_color(palette.terminal_ansi_blue).child("src/"))
            .child(
              div()
                .text_color(palette.terminal_ansi_yellow)
                .child("main.rs"),
            ),
        )
        .child(
          h_flex().w_full().gap_1().mt_1().children(
            [
              palette.terminal_ansi_red,
              palette.terminal_ansi_green,
              palette.terminal_ansi_yellow,
              palette.terminal_ansi_blue,
              palette.terminal_ansi_magenta,
              palette.terminal_ansi_cyan,
            ]
            .map(|color| div().flex_1().h(px(4.0)).rounded_sm().bg(color)),
          ),
        ),
    )
}

impl SettingsPage {
  pub(super) fn render_theme_gallery(
    &self,
    ix: usize,
    window: &Window,
    cx: &mut Context<Self>,
  ) -> Div {
    let form = self
      .form
      .as_ref()
      .expect("theme fields require a loaded form");
    let selected = form.fields[ix].value(cx);
    let mode = FIELDS
      .iter()
      .position(|spec| spec.table == "colors" && spec.key == "theme_mode")
      .map(|ix| form.fields[ix].value(cx))
      .unwrap_or_default();
    let is_dark = preview_is_dark(&mode, window.appearance());
    let columns = gallery_columns(f32::from(window.viewport_size().width));
    let rows = self.themes.len().div_ceil(usize::from(columns));
    let grid_height = rows as f32 * CARD_HEIGHT + rows.saturating_sub(1) as f32 * GRID_GAP;
    v_flex()
      .debug_selector(|| "settings-field-colors-theme".into())
      .w_full()
      .min_w_0()
      .flex_shrink_0()
      .gap_3()
      .p_4()
      .border_b_1()
      .border_color(cx.theme().border.opacity(0.5))
      .child(div().font_weight(FontWeight::SEMIBOLD).child("Theme"))
      .child(form.fields[ix].render("Theme name", self.busy))
      .child(
        div()
          .debug_selector(|| "settings-theme-gallery".into())
          .grid()
          .grid_cols(columns)
          .w_full()
          .h(px(grid_height))
          .flex_shrink_0()
          .gap_3()
          .children(self.themes.iter().map(|preview| {
            let is_selected = preview.id == selected;
            let palette = preview.palette(is_dark);
            let id = preview.id.clone();
            let selector = id.clone();
            let selected_selector = id.clone();
            let theme = cx.theme();
            Button::new(SharedString::from(format!("settings-theme-{id}")))
              .debug_selector(move || format!("settings-theme-card-{selector}"))
              .accessibility_label(format!("{} theme", preview.name))
              .tooltip(preview.id.clone())
              .custom(
                ButtonCustomVariant::new(cx)
                  .color(theme.background)
                  .foreground(theme.foreground)
                  .hover(theme.secondary_hover)
                  .active(theme.secondary_active),
              )
              .selected(is_selected)
              .disabled(self.busy || palette.is_none())
              .w_full()
              .min_w_0()
              .h(px(CARD_HEIGHT))
              .p_0()
              .rounded(px(chrome::CONTROL_RADIUS))
              .border_2()
              .border_color(if is_selected {
                theme.primary
              } else {
                theme.border
              })
              .child(
                v_flex()
                  .w_full()
                  .h(px(CARD_HEIGHT - 4.0))
                  .min_w_0()
                  .overflow_hidden()
                  .when_some(palette, |card, palette| {
                    card.child(terminal_preview(&preview.id, palette, cx))
                  })
                  .when(palette.is_none(), |card| {
                    card.child(
                      div()
                        .w_full()
                        .h(px(PREVIEW_HEIGHT))
                        .p_3()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child("Preview unavailable"),
                    )
                  })
                  .child(
                    h_flex()
                      .flex_1()
                      .min_w_0()
                      .px_3()
                      .gap_2()
                      .child(
                        div()
                          .flex_1()
                          .min_w_0()
                          .truncate()
                          .text_sm()
                          .child(preview.name.clone()),
                      )
                      .when(is_selected, |label| {
                        label.child(
                          div()
                            .debug_selector(move || {
                              format!("settings-theme-selected-{selected_selector}")
                            })
                            .text_color(theme.primary)
                            .child(Icon::new(IconName::Check).small()),
                        )
                      }),
                  ),
              )
              .on_click(cx.listener(move |this, _, window, cx| {
                this.set_field(ix, id.clone(), window, cx);
              }))
          })),
      )
      .when(self.themes.is_empty(), |gallery| {
        gallery.child(
          div()
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .child("Reload settings to load theme previews."),
        )
      })
  }
}

#[cfg(test)]
mod tests {
  use super::{ThemePreview, gallery_columns, preview_is_dark};
  use crate::components::settings_page::{FIELDS, Section, SettingsPage};
  use gpui::{TestAppContext, WindowAppearance};

  fn scene_has_background(scene: &gpui::Scene, color: gpui::Hsla) -> bool {
    scene
      .quads
      .iter()
      .any(|quad| quad.background == color.into())
      || scene.paths.iter().any(|path| path.color == color.into())
      || scene.layers.frames.iter().any(|layer| {
        layer.background == color.to_rgb()
          || layer
            .content
            .scene()
            .is_some_and(|scene| scene_has_background(scene, color))
      })
  }

  #[test]
  fn theme_gallery_columns_and_mode_follow_available_space_and_draft() {
    for (width, columns) in [(420.0, 1), (600.0, 1), (800.0, 2), (1200.0, 3), (2400.0, 3)] {
      assert_eq!(gallery_columns(width), columns);
    }
    for appearance in [
      WindowAppearance::Light,
      WindowAppearance::VibrantLight,
      WindowAppearance::Dark,
      WindowAppearance::VibrantDark,
    ] {
      assert!(preview_is_dark("dark", appearance));
      assert!(!preview_is_dark("light", appearance));
      assert_eq!(
        preview_is_dark("system", appearance),
        matches!(
          appearance,
          WindowAppearance::Dark | WindowAppearance::VibrantDark
        )
      );
    }
    let file =
      config::parse_theme_content("name = 'Custom'\n[dark]\nbackground = '#123456'").unwrap();
    let expected = file.dark.to_palette(false).terminal_background;
    let preview = ThemePreview::from_file("custom".into(), Some(file));
    assert_eq!(
      preview.palette(false).unwrap().terminal_background,
      expected
    );
    assert_eq!(preview.name, "Custom");
    assert!(
      ThemePreview::from_file("broken".into(), None)
        .palette(true)
        .is_none()
    );
  }

  #[test]
  fn theme_gallery_disambiguates_names_without_merging_distinct_themes() {
    let builtin =
      config::parse_theme_content("name = 'Kanagawa'\n[dark]\nbackground = '#16161d'").unwrap();
    let custom =
      config::parse_theme_content("name = 'Kanagawa'\n[dark]\nbackground = '#8e8ee0'").unwrap();
    let mut variant = builtin.clone();
    variant.name = "KANAGAWA".into();
    let unique = config::parse_theme_content("name = 'One'\n[dark]").unwrap();
    let mut previews = vec![
      ThemePreview::from_file("kanagawa".into(), Some(builtin)),
      ThemePreview::from_file("two or three".into(), Some(custom)),
      ThemePreview::from_file("variant".into(), Some(variant)),
      ThemePreview::from_file("one".into(), Some(unique)),
      ThemePreview::from_file("broken".into(), None),
    ];
    let palettes = previews
      .iter()
      .map(|preview| {
        preview
          .palette(true)
          .map(|palette| palette.terminal_background)
      })
      .collect::<Vec<_>>();
    ThemePreview::disambiguate_names(&mut previews);
    assert_eq!(
      previews
        .iter()
        .map(|preview| (preview.id.as_str(), preview.name.as_str()))
        .collect::<Vec<_>>(),
      vec![
        ("kanagawa", "Kanagawa (kanagawa)"),
        ("two or three", "Kanagawa (two or three)"),
        ("variant", "KANAGAWA (variant)"),
        ("one", "One"),
        ("broken", "broken"),
      ],
    );
    assert_ne!(palettes[0], palettes[1]);
    for (preview, palette) in previews.iter().zip(palettes) {
      assert_eq!(
        preview
          .palette(true)
          .map(|palette| palette.terminal_background),
        palette
      );
    }
  }

  #[gpui::test]
  fn settings_theme_gallery_renders_all_themes_and_selects_only_the_draft(cx: &mut TestAppContext) {
    // Saving settings can change the process-global theme directory.
    let _guard = crate::components::main_window_e2e_tests::test_lock();
    crate::test_support::init_test_app(cx);
    let previews = ThemePreview::load_all();
    let ids = previews
      .iter()
      .map(|preview| preview.id.clone())
      .collect::<Vec<_>>();
    assert_eq!(ids, config::list_available_themes());
    assert!(ids.len() >= 7);
    // GPUI's test lookup requires static selectors; allocate one per catalog entry.
    let selectors = ids
      .iter()
      .map(|id| {
        let selector: &'static str =
          Box::leak(format!("settings-theme-card-{id}").into_boxed_str());
        selector
      })
      .collect::<Vec<_>>();
    let light = previews
      .iter()
      .find(|preview| preview.id == "dracula")
      .unwrap()
      .palette(false)
      .unwrap()
      .terminal_background;
    let window = cx.add_window(|window, cx| {
      let mut page = SettingsPage::new(window, cx);
      let config = cx.global::<config::Config>().clone();
      page.form = Some(page.make_form(&config, window, cx).unwrap());
      page.themes = previews;
      page.section = Section::Appearance;
      page
    });
    let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
    for width in [600.0, 1000.0, 1400.0] {
      cx.simulate_window_resize(window.into(), gpui::size(gpui::px(width), gpui::px(1000.0)));
      cx.run_until_parked();
      for &selector in &selectors {
        let bounds = visual.debug_bounds(selector).unwrap();
        assert!(bounds.left() >= gpui::px(200.0) && bounds.right() <= gpui::px(width));
        assert!(bounds.size.width >= gpui::px(160.0), "{width}: {bounds:?}");
      }
    }
    let card = visual.debug_bounds("settings-theme-card-dracula").unwrap();
    visual.simulate_click(card.center(), gpui::Modifiers::default());
    cx.run_until_parked();
    window
      .update(cx, |page, window, cx| {
        assert!(page.dirty);
        assert_eq!(page.build_config(cx).unwrap().colors.theme, "dracula");
        assert_ne!(cx.global::<config::Config>().colors.theme, "dracula");
        let mode = FIELDS
          .iter()
          .position(|spec| spec.key == "theme_mode")
          .unwrap();
        page.set_field(mode, "light".into(), window, cx);
      })
      .unwrap();
    cx.run_until_parked();
    assert!(
      visual
        .debug_bounds("settings-theme-selected-dracula")
        .is_some()
    );
    let sample_bounds = visual
      .debug_bounds("settings-theme-sample-dracula")
      .unwrap();
    let card_bounds = visual.debug_bounds("settings-theme-card-dracula").unwrap();
    let panel = visual.debug_bounds("settings-fields-panel").unwrap();
    assert!(
      panel.contains(&sample_bounds.center()),
      "panel {panel:?}; sample {sample_bounds:?}"
    );
    let viewport = visual.debug_bounds("settings-content").unwrap();
    assert!(
      viewport.contains(&sample_bounds.center()),
      "viewport {viewport:?}; sample {sample_bounds:?}"
    );
    let gallery_bounds = visual.debug_bounds("settings-theme-gallery").unwrap();
    let field_bounds = visual.debug_bounds("settings-field-colors-theme").unwrap();
    assert!(
      gallery_bounds.bottom() >= card_bounds.bottom(),
      "grid {gallery_bounds:?}; card {card_bounds:?}; field {field_bounds:?}"
    );
    assert!(
      field_bounds.bottom() >= card_bounds.bottom(),
      "grid {gallery_bounds:?}; card {card_bounds:?}; field {field_bounds:?}"
    );
    visual.update(|window, _| {
      assert!(
        // Rounded cards and scroll areas can both paint into nested scenes.
        scene_has_background(window.painted_scene(), gpui::Hsla { a: 1.0, ..light }),
        "Expected light preview {light:?}; sample {sample_bounds:?}; card {card_bounds:?}",
      );
    });
    window
      .update(cx, |page, _, cx| {
        page.busy = true;
        cx.notify();
      })
      .unwrap();
    cx.run_until_parked();
    let other = visual.debug_bounds("settings-theme-card-github").unwrap();
    visual.simulate_click(other.center(), gpui::Modifiers::default());
    cx.run_until_parked();
    window
      .update(cx, |page, _, cx| {
        assert_eq!(page.build_config(cx).unwrap().colors.theme, "dracula");
      })
      .unwrap();
  }
}
