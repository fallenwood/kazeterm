use gpui::{App, Context, Entity, FocusHandle, MouseButton};

use crate::terminal::Terminal;

use super::{TerminalElement, is_mouse_from_touch};

use gpui::{Bounds, Pixels, Window};
use terminal_kernel::term::TermMode;

impl TerminalElement {
  pub(super) fn generic_button_handler<E>(
    connection: Entity<Terminal>,
    focus_handle: FocusHandle,
    steal_focus: bool,
    f: impl Fn(&mut Terminal, &E, &mut Context<Terminal>),
  ) -> impl Fn(&E, &mut Window, &mut App) {
    move |event, window, cx| {
      if steal_focus {
        window.focus(&focus_handle);
      } else if !focus_handle.is_focused(window) {
        return;
      }
      connection.update(cx, |terminal, cx| {
        f(terminal, event, cx);
        cx.notify();
      })
    }
  }

  pub(super) fn register_mouse_listeners(
    &mut self,
    mode: TermMode,
    hitbox: &gpui::Hitbox,
    scrollbar_bounds: Option<Bounds<Pixels>>,
    minimap_bounds: Option<Bounds<Pixels>>,
    right_click_context_menu: bool,
    window: &mut Window,
  ) {
    let focus = self.focus.clone();
    let terminal = self.terminal.clone();
    let terminal_view = self.terminal_view.clone();

    self.interactivity.on_mouse_down(MouseButton::Left, {
      let terminal = terminal.clone();
      let focus = focus.clone();
      let terminal_view = terminal_view.clone();

      move |e, window, cx| {
        if let Some(sb_bounds) = scrollbar_bounds
          && sb_bounds.contains(&e.position)
        {
          return;
        }
        if let Some(mm_bounds) = minimap_bounds
          && mm_bounds.contains(&e.position)
        {
          return;
        }

        window.focus(&focus);

        if is_mouse_from_touch() {
          terminal.update(cx, |terminal, _| {
            terminal.begin_touch(e.position);
          });
          terminal_view.update(cx, |view, cx| {
            view.start_long_press_timer(window, cx);
          });
          return;
        }

        let scroll_top = terminal_view.read(cx).scroll_top;
        terminal.update(cx, |terminal, cx| {
          let mut adjusted_event = e.clone();
          if scroll_top > Pixels::ZERO {
            adjusted_event.position.y += scroll_top;
          }
          terminal.mouse_down(&adjusted_event, cx);
          cx.notify();
        })
      }
    });

    window.on_mouse_event({
      let terminal = self.terminal.clone();
      let hitbox = hitbox.clone();
      let focus = focus.clone();
      let terminal_view = terminal_view;
      move |e: &gpui::MouseMoveEvent, _phase, window, cx| {
        if e.pressed_button.is_some() && !cx.has_active_drag() && focus.is_focused(window) {
          // Skip terminal mouse handling while scrollbar is being dragged
          if terminal_view.read(cx).scrollbar_drag_state.is_some() {
            return;
          }

          if terminal.read(cx).is_touch_active() {
            terminal.update(cx, |terminal, cx| {
              terminal.touch_move(e.position);
              cx.notify();
            });
            return;
          }

          // Skip if mouse is over scrollbar or minimap
          if let Some(sb_bounds) = scrollbar_bounds
            && sb_bounds.contains(&e.position)
          {
            return;
          }
          if let Some(mm_bounds) = minimap_bounds
            && mm_bounds.contains(&e.position)
          {
            return;
          }

          let hovered = hitbox.is_hovered(window);

          let scroll_top = terminal_view.read(cx).scroll_top;
          terminal.update(cx, |terminal, cx| {
            if terminal.selection_started() || hovered {
              let mut adjusted_event = e.clone();
              if scroll_top > Pixels::ZERO {
                adjusted_event.position.y += scroll_top;
              }
              terminal.mouse_drag(&adjusted_event, hitbox.bounds, cx);
              cx.notify();
            }
          })
        }

        if hitbox.is_hovered(window) {
          terminal.update(cx, |terminal, cx| {
            terminal.mouse_move(e, cx);
          })
        }

        // Track hover state for split pane border highlighting
        let hovered_now = hitbox.is_hovered(window);
        let was_hovered = terminal_view.read(cx).is_hovered;
        if hovered_now != was_hovered {
          let focus_terminal_on_hover = cx
            .try_global::<config::Config>()
            .map(|config| config.terminal.focus_terminal_on_hover)
            .unwrap_or(true);

          if hovered_now && focus_terminal_on_hover && !focus.is_focused(window) {
            window.focus(&focus);
          }

          terminal_view.update(cx, |view, cx| {
            view.is_hovered = hovered_now;
            cx.notify();
          });
        }
      }
    });

    self.interactivity.on_mouse_up(
      MouseButton::Left,
      TerminalElement::generic_button_handler(
        terminal.clone(),
        focus.clone(),
        false,
        move |terminal, e, cx| {
          if terminal.is_touch_active() {
            terminal.end_touch();
            return;
          }
          terminal.mouse_up(e, cx);
        },
      ),
    );

    self.interactivity.on_mouse_down(
      MouseButton::Middle,
      TerminalElement::generic_button_handler(
        terminal.clone(),
        focus.clone(),
        true,
        move |terminal, e, cx| {
          terminal.mouse_down(e, cx);
        },
      ),
    );

    self.interactivity.on_scroll_wheel({
      let terminal_view = self.terminal_view.downgrade();
      move |e, window, cx| {
        terminal_view
          .update(cx, |terminal_view, cx| {
            if terminal_view.focus_handle.is_focused(window) {
              terminal_view.scroll_wheel(e, window, cx);
            }
          })
          .ok();
      }
    });

    if mode.intersects(TermMode::MOUSE_MODE) {
      self.interactivity.on_mouse_down(
        MouseButton::Right,
        TerminalElement::generic_button_handler(
          terminal.clone(),
          focus.clone(),
          true,
          move |terminal, e, cx| {
            terminal.mouse_down(e, cx);
          },
        ),
      );

      self.interactivity.on_mouse_up(
        MouseButton::Right,
        TerminalElement::generic_button_handler(
          terminal.clone(),
          focus.clone(),
          false,
          move |terminal, e, cx| {
            terminal.mouse_up(e, cx);
          },
        ),
      );

      self.interactivity.on_mouse_up(
        MouseButton::Middle,
        TerminalElement::generic_button_handler(terminal, focus, false, move |terminal, e, cx| {
          terminal.mouse_up(e, cx);
        }),
      );
    } else {
      if !right_click_context_menu {
        self.interactivity.on_mouse_down(MouseButton::Right, {
          let terminal = terminal.clone();
          move |e, _window, cx| {
            if is_mouse_from_touch() {
              terminal.update(cx, |term, cx| {
                term.start_touch_selection(e.position);
                cx.notify();
              });
              return;
            }
            let has_selection = terminal.read(cx).last_content.selection.is_some();
            if has_selection {
              terminal.update(cx, |term, cx| {
                term.copy_and_clear_selection(cx);
              });
            } else if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
              terminal.update(cx, |term, _cx| {
                term.input(text.into_bytes());
              });
            }
          }
        });

        self.interactivity.on_mouse_up(MouseButton::Right, {
          let terminal = terminal.clone();
          let focus = focus.clone();
          move |_e, window, cx| {
            if !focus.is_focused(window) {
              return;
            }
            if terminal.read(cx).is_touch_active() {
              terminal.update(cx, |term, _| {
                term.end_touch();
              });
            }
          }
        });
      }
    }
  }
}
