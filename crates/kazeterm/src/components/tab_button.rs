use std::rc::Rc;

use gpui::{
  App, ClickEvent, ElementId, InteractiveElement, IntoElement, KeyboardClickEvent, MouseButton,
  ParentElement, RenderOnce, StatefulInteractiveElement, Styled, Window, div,
  prelude::FluentBuilder,
};
use themeing::SettingsStore;

#[derive(IntoElement)]
pub struct TabButton {
  #[allow(unused)]
  id: ElementId,
  index: usize,
  visible: bool,
  on_click: Option<Rc<dyn Fn(&TabButtonClickEvent, &mut Window, &mut App)>>,
}

#[derive(Clone, Debug)]
pub struct TabButtonClickEvent {
  pub index: usize,
  _inner: ClickEvent,
}

impl Default for TabButtonClickEvent {
  fn default() -> Self {
    Self {
      index: 0,
      _inner: ClickEvent::Keyboard(KeyboardClickEvent::default()),
    }
  }
}

impl TabButton {
  pub fn new(id: impl Into<ElementId>, index: usize) -> Self {
    Self {
      id: id.into(),
      index,
      visible: true,
      on_click: None,
    }
  }

  /// Set visibility of the close button (for hover-only behavior)
  pub fn visible(mut self, visible: bool) -> Self {
    self.visible = visible;
    self
  }

  pub fn on_click(
    mut self,
    handler: impl Fn(&TabButtonClickEvent, &mut Window, &mut App) + 'static,
  ) -> Self {
    self.on_click = Some(Rc::new(handler));
    self
  }
}

impl RenderOnce for TabButton {
  fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
    let setting_store = cx.global::<SettingsStore>();
    let theme = setting_store.theme();
    let colors = theme.colors();
    let hover_bg = colors.element_hover;
    let active_bg = colors.element_active;
    let text_color = colors.text_muted;

    // Simple close button - no nested interactive elements
    div()
      .id(self.id.clone())
      .size_4()
      .flex_shrink_0()
      .rounded_sm()
      .flex()
      .items_center()
      .justify_center()
      .cursor_pointer()
      .when(self.visible, |this| {
        this
          .hover(move |style| style.bg(hover_bg))
          .active(move |style| style.bg(active_bg))
          .child(div().text_color(text_color).text_sm().child("×"))
      })
      .on_mouse_down(
        MouseButton::Left,
        |_: &gpui::MouseDownEvent, _: &mut Window, cx: &mut App| {
          cx.stop_propagation();
        },
      )
      .on_click({
        let on_click = self.on_click.clone();
        let index = self.index;
        move |ev: &ClickEvent, window: &mut Window, cx: &mut App| {
          cx.stop_propagation();

          let ev = TabButtonClickEvent {
            index,
            _inner: ev.clone(),
          };

          if let Some(callback) = &on_click {
            callback(&ev, window, cx);
          }
        }
      })
  }
}
