use gpui::*;
use gpui_kit::component::{
  ActiveTheme,
  button::{Button, ButtonVariants},
  input::{Input, InputEvent, InputState},
};
use themeing::SettingsStore;

#[derive(Clone)]
pub enum GroupRenameEvent {
  Cancel,
  Save {
    group_id: String,
    name: Option<String>,
  },
}

pub struct GroupRenameDialog {
  group_id: String,
  input: Entity<InputState>,
  _subscription: Subscription,
}

impl EventEmitter<GroupRenameEvent> for GroupRenameDialog {}

impl GroupRenameDialog {
  pub fn new(group_id: String, name: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
    let input = cx.new(|cx| InputState::new(window, cx).default_value(name));
    let subscription = cx.subscribe_in(&input, window, |dialog, _, event, _window, cx| {
      if matches!(event, InputEvent::PressEnter { .. }) {
        dialog.save(cx);
      }
    });
    window.focus(&input.focus_handle(cx), cx);
    Self {
      group_id,
      input,
      _subscription: subscription,
    }
  }

  fn save(&self, cx: &mut Context<Self>) {
    let value = self.input.read(cx).value().trim().to_string();
    cx.emit(GroupRenameEvent::Save {
      group_id: self.group_id.clone(),
      name: (!value.is_empty()).then_some(value),
    });
  }
}

impl Render for GroupRenameDialog {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let theme = cx.theme();
    let overlay = cx
      .global::<SettingsStore>()
      .theme()
      .colors()
      .overlay_background;
    div()
      .absolute()
      .inset_0()
      .flex()
      .items_center()
      .justify_center()
      .bg(overlay)
      .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
      .child(
        div()
          .w(px(350.0))
          .p_4()
          .bg(theme.popover)
          .text_color(theme.popover_foreground)
          .rounded_md()
          .shadow_lg()
          .border_1()
          .border_color(theme.border)
          .on_key_down(cx.listener(|_this, event: &KeyDownEvent, _, cx| {
            if event.keystroke.key == "escape" {
              cx.emit(GroupRenameEvent::Cancel);
            }
          }))
          .child(
            div()
              .flex()
              .flex_col()
              .gap_3()
              .child("Rename Group")
              .child(Input::new(&self.input).w_full().cursor_text())
              .child(
                div()
                  .text_sm()
                  .text_color(theme.muted_foreground)
                  .child("Leave blank for an automatic name"),
              )
              .child(
                gpui_kit::component::h_flex()
                  .gap_2()
                  .justify_end()
                  .child(
                    Button::new("cancel-group-rename")
                      .ghost()
                      .label("Cancel")
                      .on_click(cx.listener(|_, _, _, cx| cx.emit(GroupRenameEvent::Cancel))),
                  )
                  .child(
                    Button::new("save-group-rename")
                      .primary()
                      .label("Save")
                      .on_click(cx.listener(|this, _, _, cx| this.save(cx))),
                  ),
              ),
          ),
      )
  }
}

#[derive(Clone, Copy)]
pub enum GroupDeleteEvent {
  Cancel,
  Ungroup,
  CloseTabs,
}

pub struct GroupDeleteDialog {
  focus: FocusHandle,
  count: usize,
}

impl EventEmitter<GroupDeleteEvent> for GroupDeleteDialog {}

impl GroupDeleteDialog {
  pub fn new(count: usize, window: &mut Window, cx: &mut Context<Self>) -> Self {
    let focus = cx.focus_handle();
    window.focus(&focus, cx);
    Self { focus, count }
  }
}

impl Render for GroupDeleteDialog {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let theme = cx.theme();
    let overlay = cx
      .global::<SettingsStore>()
      .theme()
      .colors()
      .overlay_background;
    div()
      .absolute()
      .inset_0()
      .flex()
      .items_center()
      .justify_center()
      .bg(overlay)
      .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
      .on_key_down(cx.listener(
        |_, event: &KeyDownEvent, _, cx| match event.keystroke.key.as_str() {
          "escape" => cx.emit(GroupDeleteEvent::Cancel),
          "enter" => cx.emit(GroupDeleteEvent::Ungroup),
          _ => {}
        },
      ))
      .child(
        div()
          .track_focus(&self.focus)
          .w(px(430.0))
          .p_4()
          .bg(theme.popover)
          .text_color(theme.popover_foreground)
          .rounded_md()
          .shadow_lg()
          .border_1()
          .border_color(theme.border)
          .child(
            div()
              .flex()
              .flex_col()
              .gap_3()
              .child("Delete Group?")
              .child(format!(
                "Remove all {} tabs from the group, or close them and end their terminal sessions?",
                self.count
              ))
              .child(
                gpui_kit::component::h_flex()
                  .gap_2()
                  .justify_end()
                  .child(
                    Button::new("cancel-group-delete")
                      .ghost()
                      .label("Cancel")
                      .on_click(cx.listener(|_, _, _, cx| cx.emit(GroupDeleteEvent::Cancel))),
                  )
                  .child(
                    Button::new("ungroup-all")
                      .primary()
                      .label("Ungroup All")
                      .on_click(cx.listener(|_, _, _, cx| cx.emit(GroupDeleteEvent::Ungroup))),
                  )
                  .child(
                    Button::new("close-group-tabs")
                      .danger()
                      .label(format!("Close {} Tabs", self.count))
                      .on_click(cx.listener(|_, _, _, cx| cx.emit(GroupDeleteEvent::CloseTabs))),
                  ),
              ),
          ),
      )
  }
}
