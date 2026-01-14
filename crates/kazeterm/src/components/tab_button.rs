use std::rc::Rc;

use gpui::{
  App, ClickEvent, ElementId, InteractiveElement, IntoElement, KeyboardClickEvent,
  MouseButton, RenderOnce, Window,
};
use gpui_component::{Sizable as _, button::{Button, ButtonVariants}};

#[derive(IntoElement)]
pub struct TabButton {
  #[allow(unused)]
  id: ElementId,
  index: usize,
  on_click: Option<Rc<dyn Fn(&TabButtonClickEvent, &mut Window, &mut App)>>,
  button: Button,
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
    let button = Button::new("close")
      .ghost()
      .xsmall()
      .label("×");

    Self {
      id: id.into(),
      index,
      on_click: None,
      button,
    }
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
  fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
    self.button
      .on_mouse_down(MouseButton::Left, |_: &gpui::MouseDownEvent, _: &mut Window, cx: &mut App| {
        cx.stop_propagation();
      })
      .on_click(move |ev: &ClickEvent, window: &mut Window, cx: &mut App| {
        cx.stop_propagation();

        let ev = TabButtonClickEvent {
          index: self.index,
          _inner: ev.clone(),
        };

        if let Some(callback) = &self.on_click {
          callback(&ev, window, cx);
        }
      })
  }
}
