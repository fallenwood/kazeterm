//! Terminal tab bar component - custom implementation replacing gpui_component::tab
//! Based on zTerm's tab_bar.rs implementation

use gpui::prelude::*;
use gpui::*;
use gpui_component::h_flex;

/// Terminal tab bar component - a scrollable container for tab items
#[derive(IntoElement)]
pub struct TerminalTabBar {
  id: ElementId,
  scroll_handle: Option<ScrollHandle>,
  children: Vec<AnyElement>,
}

impl TerminalTabBar {
  /// Create a new tab bar with the given ID
  pub fn new(id: impl Into<ElementId>) -> Self {
    Self {
      id: id.into(),
      scroll_handle: None,
      children: vec![],
    }
  }

  /// Enable scroll tracking
  pub fn track_scroll(mut self, scroll_handle: &ScrollHandle) -> Self {
    self.scroll_handle = Some(scroll_handle.clone());
    self
  }

  /// Add child elements (tabs)
  pub fn children(mut self, children: impl IntoIterator<Item = impl IntoElement>) -> Self {
    self.children = children.into_iter().map(|c| c.into_any_element()).collect();
    self
  }
}

impl RenderOnce for TerminalTabBar {
  fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
    let scroll_handle = self.scroll_handle;
    let children = self.children;

    div()
      .id(self.id)
      .relative()
      .flex_1()
      .h_full()
      .min_w_0()
      .overflow_x_hidden()
      .child(
        h_flex()
          .id("tabs-container")
          .h_full()
          .gap_1()
          .overflow_x_scroll()
          .when_some(scroll_handle, |this, handle| this.track_scroll(&handle))
          .children(children),
      )
  }
}

/// A single terminal tab - wrapper for tab content with click handling
#[derive(IntoElement)]
pub struct TerminalTab {
  selected: bool,
  on_mouse_down: Option<Box<dyn Fn(&MouseDownEvent, &mut Window, &mut App) + 'static>>,
  child: Option<AnyElement>,
}

impl TerminalTab {
  /// Create a new terminal tab
  pub fn new() -> Self {
    Self {
      selected: false,
      on_mouse_down: None,
      child: None,
    }
  }

  /// Set whether this tab is selected
  pub fn selected(mut self, selected: bool) -> Self {
    self.selected = selected;
    self
  }

  /// Set the mouse down handler
  pub fn on_mouse_down(
    mut self,
    button: MouseButton,
    handler: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
  ) -> Self {
    if button == MouseButton::Left {
      self.on_mouse_down = Some(Box::new(handler));
    }
    self
  }

  /// Set the tab's child content
  pub fn child(mut self, child: impl IntoElement) -> Self {
    self.child = Some(child.into_any_element());
    self
  }
}

impl Default for TerminalTab {
  fn default() -> Self {
    Self::new()
  }
}

impl RenderOnce for TerminalTab {
  fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
    let on_mouse_down = self.on_mouse_down;
    let child = self.child;

    div()
      .flex()
      .flex_shrink_0()
      .items_center()
      .h_full()
      .cursor_pointer()
      .when_some(on_mouse_down, |this, handler| {
        this.on_mouse_down(MouseButton::Left, move |e, window, cx| {
          handler(e, window, cx);
        })
      })
      .when_some(child, |this, c| this.child(c))
  }
}
