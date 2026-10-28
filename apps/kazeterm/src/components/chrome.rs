//! Shared presentation for the terminal workspace and Settings.

use gpui::{App, Div, ElementId, Hsla, Styled, div, px};
use gpui_kit::component::{
  ActiveTheme, Sizable,
  button::{Button, ButtonVariants},
  h_flex,
};

pub(super) const CONTROL_RADIUS: f32 = 6.0;

pub(super) fn page(cx: &App) -> Div {
  div()
    .flex()
    .flex_col()
    .size_full()
    .min_h_0()
    .bg(cx.theme().background)
    .text_color(cx.theme().foreground)
    .font_family(cx.theme().font_family.clone())
    .text_size(cx.theme().font_size)
}

pub(super) fn toolbar(cx: &App) -> Div {
  h_flex()
    .flex_shrink_0()
    .bg(cx.theme().title_bar)
    .border_b_1()
    .border_color(cx.theme().title_bar_border)
}

pub(super) fn toolbar_button(id: impl Into<ElementId>) -> Button {
  Button::new(id).ghost().small().rounded(px(CONTROL_RADIUS))
}

/// Tabs and settings categories use the same surface, text, and edge indicator.
/// The caller supplies the indicator to preserve grouped tabs' distinct color.
pub(super) fn navigation_item<E: Styled>(
  element: E,
  selected: bool,
  vertical: bool,
  indicator: Hsla,
  cx: &App,
) -> E {
  let theme = cx.theme();
  let element = element
    .bg(if selected {
      theme.tab_active
    } else {
      theme.tab
    })
    .text_color(if selected {
      theme.foreground
    } else {
      theme.muted_foreground
    })
    .border_color(if selected {
      indicator
    } else {
      theme.transparent
    });
  if vertical {
    element.border_l_2().rounded(px(CONTROL_RADIUS))
  } else {
    element.border_b_2().rounded_t(px(CONTROL_RADIUS))
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use gpui::TestAppContext;

  #[gpui::test]
  fn chrome_uses_interface_font_and_shared_navigation_tokens(cx: &mut TestAppContext) {
    crate::test_support::init_test_app(cx);
    cx.update(|cx| {
      let theme = cx.theme().clone();
      let mut root = page(cx);
      assert_eq!(
        root.style().text.font_family,
        Some(theme.font_family.clone())
      );
      assert_eq!(root.style().text.font_size, Some(theme.font_size.into()));
      assert_eq!(root.style().background, Some(theme.background.into()));
      let mut bar = toolbar(cx);
      assert_eq!(bar.style().background, Some(theme.title_bar.into()));
      assert_eq!(bar.style().border_color, Some(theme.title_bar_border));

      for selected in [false, true] {
        for vertical in [false, true] {
          // Grouped tabs can use a separate indicator without changing the surface.
          for indicator in [theme.primary, theme.foreground] {
            let mut item = navigation_item(div(), selected, vertical, indicator, cx);
            let style = item.style();
            assert_eq!(
              style.background,
              Some(
                if selected {
                  theme.tab_active
                } else {
                  theme.tab
                }
                .into()
              ),
            );
            assert_eq!(
              style.border_color,
              Some(if selected {
                indicator
              } else {
                theme.transparent
              }),
            );
            assert_eq!(
              style.text.color,
              Some(if selected {
                theme.foreground
              } else {
                theme.muted_foreground
              }),
            );
            let edge = if vertical {
              style.border_widths.left
            } else {
              style.border_widths.bottom
            };
            assert_eq!(edge, Some(px(2.0).into()));
          }
        }
      }
    });
  }
}
