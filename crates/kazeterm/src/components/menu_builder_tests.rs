use std::{cell::Cell, rc::Rc};

use gpui::{
  AppContext, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render, Styled,
  TestAppContext, Window, div, px,
};
use gpui_kit::component::{
  IconName, Root,
  button::Button,
  menu::{DropdownMenu, PopupMenuItem},
};

use crate::components::{
  MainWindow,
  main_window_e2e_tests::{install_fake_factory, test_lock},
  split_pane_context_menu::build_terminal_context_menu,
  terminal_window::clear_terminal_session_factory_for_testing,
};

#[derive(Clone, Copy, Debug)]
enum MenuKind {
  NewTab,
  Tab,
  Terminal,
}

struct MenuTest {
  main: Entity<MainWindow>,
  kind: MenuKind,
  selected: Rc<Cell<bool>>,
  focus: gpui::FocusHandle,
}

fn probe(id: &'static str, selected: Rc<Cell<bool>>) -> PopupMenuItem {
  PopupMenuItem::element(move |_, _| div().debug_selector(move || id.into()).w_full().child(id))
    .on_click(move |_, _, _| selected.set(true))
}

impl Render for MenuTest {
  fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
    let main = self.main.clone();
    let kind = self.kind;
    let selected = self.selected.clone();
    div()
      .track_focus(&self.focus)
      .size_full()
      .p_4()
      .flex()
      .flex_col()
      .items_end()
      .justify_end()
      .child(
        div()
          .debug_selector(|| "menu-trigger".into())
          .w(px(100.0))
          .child(Button::new("menu").label("Menu").w_full().dropdown_menu(
            move |menu, window, cx| {
              let menu = menu.item(probe("menu-first", selected.clone()));
              let menu = match kind {
                MenuKind::NewTab => {
                  let hosts = (0..40).map(|ix| format!("host-{ix}")).collect::<Vec<_>>();
                  super::build_new_tab_menu(menu, main.clone(), &[], &[], &hosts, &[], window, cx)
                }
                MenuKind::Tab => super::build_tab_context_menu(
                  menu,
                  main.clone(),
                  0,
                  0,
                  false,
                  true,
                  true,
                  false,
                  false,
                  "Move Left",
                  IconName::ArrowLeft,
                  "Move Right",
                  IconName::ArrowRight,
                  false,
                  false,
                  window,
                  cx,
                ),
                MenuKind::Terminal => {
                  let terminal = main.read(cx).active_terminal().unwrap();
                  build_terminal_context_menu(menu, &terminal, &main, window, cx)
                }
              };
              if matches!(kind, MenuKind::Terminal) {
                let selected = selected.clone();
                menu.submenu("Overflow options", window, cx, move |menu, window, cx| {
                  let mut menu = super::scrollable_menu(menu, window, cx)
                    .item(probe("submenu-first", selected.clone()));
                  for ix in 0..40 {
                    menu = menu.item(PopupMenuItem::new(format!("Option {ix}")));
                  }
                  menu.item(probe("submenu-last", selected.clone()))
                })
              } else {
                menu.item(probe("menu-last", selected.clone()))
              }
            },
          )),
      )
  }
}

#[gpui::test]
fn application_popup_menus_scroll_without_clipping_submenus(cx: &mut TestAppContext) {
  let _guard = test_lock();
  crate::test_support::init_test_app(cx);
  install_fake_factory();

  for kind in [MenuKind::NewTab, MenuKind::Tab, MenuKind::Terminal] {
    let selected = Rc::new(Cell::new(false));
    let focus = cx.update(|cx| cx.focus_handle());
    let window = cx.add_window(|window, cx| {
      let main = cx.new(|cx| MainWindow::new(window, cx));
      window.focus(&focus, cx);
      let view = cx.new(|_| MenuTest {
        main,
        kind,
        selected: selected.clone(),
        focus: focus.clone(),
      });
      Root::new(view, window, cx)
    });
    let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
    for (width, height) in [(420.0, 300.0), (800.0, 500.0), (1000.0, 1000.0)] {
      let viewport = gpui::Bounds::new(gpui::Point::default(), gpui::size(px(width), px(height)));
      cx.simulate_window_resize(window.into(), viewport.size);
      cx.run_until_parked();
      let trigger = visual.debug_bounds("menu-trigger").unwrap();
      visual.simulate_click(trigger.center(), gpui::Modifiers::default());
      cx.run_until_parked();
      let first = visual.debug_bounds("menu-first").unwrap();
      assert!(viewport.contains(&first.origin), "{kind:?}: {first:?}");
      assert!(
        viewport.contains(&first.bottom_right()),
        "{kind:?}: {first:?}"
      );
      if matches!(kind, MenuKind::Tab) && height == 1000.0 {
        let last = visual.debug_bounds("menu-last").unwrap();
        assert!(
          viewport.contains(&last.bottom_right()),
          "a menu that fits the window should not need scrolling: {last:?}",
        );
      }
      visual.simulate_mouse_move(first.center(), None, gpui::Modifiers::default());
      cx.run_until_parked();
      cx.simulate_keystrokes(window.into(), "up");
      cx.run_until_parked();
      let (last_selector, content_top) = if matches!(kind, MenuKind::Terminal) {
        let first = visual.debug_bounds("submenu-first").unwrap();
        assert!(viewport.contains(&first.origin), "{first:?}");
        assert!(viewport.contains(&first.bottom_right()), "{first:?}");
        visual.simulate_event(gpui::ScrollWheelEvent {
          position: first.center(),
          delta: gpui::ScrollDelta::Pixels(gpui::point(px(0.0), px(-10000.0))),
          modifiers: gpui::Modifiers::default(),
          touch_phase: gpui::TouchPhase::Moved,
        });
        cx.run_until_parked();
        ("submenu-last", first.top())
      } else {
        ("menu-last", first.top())
      };
      let last = visual.debug_bounds(last_selector).unwrap();
      assert!(viewport.contains(&last.origin), "{kind:?}: {last:?}");
      assert!(
        viewport.contains(&last.bottom_right()),
        "{kind:?}: {last:?}"
      );
      if !matches!(kind, MenuKind::Tab) {
        assert!(
          last.bottom() - content_top >= viewport.size.height - px(64.0),
          "{kind:?}: a long menu should use the available window height",
        );
      }
      visual.simulate_click(last.center(), gpui::Modifiers::default());
      cx.run_until_parked();
      assert!(
        selected.replace(false),
        "{kind:?}: last item was not clickable"
      );

      visual.simulate_click(trigger.center(), gpui::Modifiers::default());
      cx.run_until_parked();
      let first = visual.debug_bounds("menu-first").unwrap();
      if matches!(kind, MenuKind::Terminal) {
        visual.simulate_mouse_move(first.center(), None, gpui::Modifiers::default());
        cx.run_until_parked();
        cx.simulate_keystrokes(window.into(), "up");
        cx.run_until_parked();
        let submenu = visual.debug_bounds("submenu-first").unwrap();
        cx.simulate_keystrokes(
          window.into(),
          if submenu.left() < first.left() {
            "left"
          } else {
            "right"
          },
        );
        cx.run_until_parked();
      }
      cx.simulate_window_resize(window.into(), gpui::size(px(width), px(height * 0.75)));
      cx.run_until_parked();
      assert!(visual.debug_bounds("menu-first").is_none(), "{kind:?}");
      assert!(visual.debug_bounds("submenu-first").is_none(), "{kind:?}");
      assert_eq!(
        visual.update(|window, cx| window.focused(cx)),
        Some(focus.clone())
      );
    }
  }
  clear_terminal_session_factory_for_testing();
}
