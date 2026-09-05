use super::{Config, FIELDS, FieldKind, PendingAction, Section, SettingsPage};
use gpui::{AppContext, EntityInputHandler, Focusable, TestAppContext, WindowHandle};

#[gpui::test]
fn settings_dropdown_popup_matches_control_width(cx: &mut TestAppContext) {
  use gpui::{InteractiveElement, ParentElement, Styled};
  use gpui_kit::component::menu::PopupMenuItem;

  struct DropdownTest;

  impl gpui::Render for DropdownTest {
    fn render(
      &mut self,
      window: &mut gpui::Window,
      cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
      gpui::div().size_full().p_4().child(
        super::settings_dropdown("test-dropdown", "never", false, window, cx, |menu, _, _| {
          menu.item(PopupMenuItem::element(|_, _| {
            gpui::div()
              .debug_selector(|| "settings-dropdown-option".into())
              .w_full()
              .child("always")
          }))
        })
        .debug_selector(|| "settings-dropdown-trigger".into()),
      )
    }
  }

  crate::test_support::init_test_app(cx);
  let window = cx.add_window(|window, cx| {
    let view = cx.new(|_| DropdownTest);
    gpui_kit::component::Root::new(view, window, cx)
  });
  let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
  for width in [600.0, 1400.0, 420.0] {
    cx.simulate_window_resize(window.into(), gpui::size(gpui::px(width), gpui::px(500.0)));
    cx.run_until_parked();
    let trigger = visual.debug_bounds("settings-dropdown-trigger").unwrap();
    visual.simulate_click(trigger.center(), gpui::Modifiers::default());
    cx.run_until_parked();
    let option = visual.debug_bounds("settings-dropdown-option").unwrap();
    assert!(
      option.size.height < gpui::px(40.0),
      "short menus should stay compact"
    );
    let inset = trigger.size.width - option.size.width;
    assert!(
      inset >= gpui::px(0.0) && inset <= gpui::px(32.0),
      "window {width}: trigger {trigger:?}, option {option:?}",
    );
    assert!(option.left() >= trigger.left());
    assert!(option.right() <= trigger.right() + gpui::px(2.0));
    cx.simulate_keystrokes(window.into(), "escape");
    cx.run_until_parked();
  }
}

#[gpui::test]
fn settings_dropdown_popup_scrolls_within_small_windows(cx: &mut TestAppContext) {
  use gpui::{InteractiveElement, ParentElement, Styled};
  use gpui_kit::component::menu::PopupMenuItem;
  use std::{cell::Cell, rc::Rc};

  struct DropdownTest {
    selected: Rc<Cell<Option<usize>>>,
    focus: gpui::FocusHandle,
  }

  impl gpui::Render for DropdownTest {
    fn render(
      &mut self,
      window: &mut gpui::Window,
      cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
      let selected = self.selected.clone();
      gpui::div()
        .track_focus(&self.focus)
        .size_full()
        .p_4()
        .flex()
        .flex_col()
        .justify_end()
        .child(
          super::settings_dropdown(
            "test-dropdown",
            "Select an option",
            false,
            window,
            cx,
            move |mut menu, _, _| {
              for ix in 0..40 {
                let selected = selected.clone();
                menu = menu.item(
                  PopupMenuItem::element(move |_, _| {
                    gpui::div()
                      .debug_selector(move || format!("dropdown-option-{ix}"))
                      .w_full()
                      .child(format!("Option {ix}"))
                  })
                  .on_click(move |_, _, _| selected.set(Some(ix))),
                );
              }
              menu
            },
          )
          .debug_selector(|| "dropdown-trigger".into()),
        )
    }
  }

  crate::test_support::init_test_app(cx);
  let selected = Rc::new(Cell::new(None));
  let focus = cx.update(|cx| cx.focus_handle());
  let window = cx.add_window(|window, cx| {
    window.focus(&focus, cx);
    let view = cx.new(|_| DropdownTest {
      selected: selected.clone(),
      focus: focus.clone(),
    });
    gpui_kit::component::Root::new(view, window, cx)
  });
  let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
  for (width, height) in [(420.0, 240.0), (600.0, 420.0), (1000.0, 1000.0)] {
    let viewport = gpui::Bounds::new(
      gpui::Point::default(),
      gpui::size(gpui::px(width), gpui::px(height)),
    );
    cx.simulate_window_resize(window.into(), viewport.size);
    cx.run_until_parked();
    let trigger = visual.debug_bounds("dropdown-trigger").unwrap();
    visual.simulate_click(trigger.center(), gpui::Modifiers::default());
    cx.run_until_parked();
    let first = visual.debug_bounds("dropdown-option-0").unwrap();
    assert!(viewport.contains(&first.origin), "{first:?}");
    assert!(viewport.contains(&first.bottom_right()), "{first:?}");
    assert!(
      visual.debug_bounds("dropdown-option-39").unwrap().bottom() > viewport.bottom(),
      "a long dropdown should require scrolling",
    );

    visual.simulate_event(gpui::ScrollWheelEvent {
      position: first.center(),
      delta: gpui::ScrollDelta::Pixels(gpui::point(gpui::px(0.0), gpui::px(-10000.0))),
      modifiers: gpui::Modifiers::default(),
      touch_phase: gpui::TouchPhase::Moved,
    });
    cx.run_until_parked();
    let last = visual.debug_bounds("dropdown-option-39").unwrap();
    assert!(viewport.contains(&last.origin), "{last:?}");
    assert!(viewport.contains(&last.bottom_right()), "{last:?}");
    assert!(
      last.bottom() - first.top() >= viewport.size.height - gpui::px(64.0),
      "a long dropdown should use the available window height",
    );
    visual.simulate_click(last.center(), gpui::Modifiers::default());
    cx.run_until_parked();
    assert_eq!(selected.replace(None), Some(39));

    visual.simulate_click(trigger.center(), gpui::Modifiers::default());
    cx.run_until_parked();
    assert!(visual.debug_bounds("dropdown-option-0").is_some());
    cx.simulate_window_resize(
      window.into(),
      gpui::size(gpui::px(width), gpui::px(height * 0.75)),
    );
    cx.run_until_parked();
    assert!(
      visual.debug_bounds("dropdown-option-0").is_none(),
      "resizing should dismiss the popup instead of retaining an oversized height",
    );
    assert_eq!(
      visual.update(|window, cx| window.focused(cx)),
      Some(focus.clone())
    );
  }
}

struct SettingsFiles(std::path::PathBuf);

impl SettingsFiles {
  fn new() -> Self {
    let unique = std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .unwrap()
      .as_nanos();
    let directory = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
      .join(format!(".settings-test-{}-{unique}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    std::fs::write(
      directory.join("kazeterm.toml"),
      format!(
        "version = \"{}\"\nimports = [\"overlay.toml\"]\n[animation]\nenabled = false\n",
        config::CURRENT_CONFIG_VERSION,
      ),
    )
    .unwrap();
    std::fs::write(
      directory.join("overlay.toml"),
      "[appearance]\nbackground_opacity = 0.6\n",
    )
    .unwrap();
    Self(directory)
  }

  fn load(&self) -> config::ConfigFile {
    config::ConfigFile::load_from_path(self.0.join("kazeterm.toml")).unwrap()
  }
}

impl Drop for SettingsFiles {
  fn drop(&mut self) {
    std::fs::remove_file(self.0.join("kazeterm.toml")).unwrap();
    std::fs::remove_file(self.0.join("overlay.toml")).unwrap();
    std::fs::remove_dir(&self.0).unwrap();
  }
}

#[gpui::test]
fn settings_save_hot_reloads_open_windows_without_restarting_terminals(cx: &mut TestAppContext) {
  use crate::components::MainWindow;
  use crate::components::main_window_e2e_tests::{install_fake_factory, test_lock};
  use crate::components::terminal_window::clear_terminal_session_factory_for_testing;

  let _guard = test_lock();
  let files = SettingsFiles::new();
  crate::test_support::init_test_app(cx);
  let calls = install_fake_factory();
  let windows = [
    cx.add_window(|window, cx| MainWindow::new(window, cx)),
    cx.add_window(|window, cx| MainWindow::new(window, cx)),
  ];
  let terminals = windows.map(|handle| {
    handle
      .update(cx, |main, window, cx| {
        crate::window_manager::register_window(window.window_handle(), &cx.entity(), cx);
        main.active_terminal().unwrap().entity_id()
      })
      .unwrap()
  });
  let page = windows[0]
    .update(cx, |main, window, cx| {
      let page = cx.new(|cx| {
        let mut page = SettingsPage::new(window, cx);
        page.install_file(files.load(), window, cx);
        for (table, key, value) in [
          ("font", "size", "24"),
          ("font", "ui_size", "20"),
          ("colors", "theme", "nord"),
          ("tab", "vertical", "true"),
          ("appearance", "background_opacity", "0.8"),
        ] {
          let ix = FIELDS
            .iter()
            .position(|spec| spec.table == table && spec.key == key)
            .unwrap();
          page.set_field(ix, value.into(), window, cx);
        }
        page
      });
      main.attach_settings_page(page.clone(), window, cx);
      assert_eq!(cx.global::<Config>().font.size, 18.0);
      page
    })
    .unwrap();
  let factory_count = calls.lock().unwrap().programs.len();
  let save = windows[0]
    .update(cx, |_, window, cx| {
      page.update(cx, |page, cx| {
        page.save(window, cx);
        std::mem::replace(&mut page.task, gpui::Task::ready(()))
      })
    })
    .unwrap();
  cx.executor().allow_parking();
  cx.foreground_executor().block_test(save);
  cx.run_until_parked();

  cx.update(|cx| {
    let config = cx.global::<Config>();
    assert_eq!(config.font.size, 24.0);
    assert_eq!(config.font.ui_size, 20.0);
    assert!(config.tab.vertical);
    assert_eq!(config.appearance.background_opacity, 0.6);
    assert_eq!(cx.global::<themeing::SettingsStore>().theme().id, "nord");
    let theme = cx.global::<gpui_kit::component::Theme>();
    assert_eq!(theme.font_size, gpui::px(20.0));
    assert_eq!(theme.mono_font_size, gpui::px(24.0));
    let page = page.read(cx);
    assert!(!page.busy);
    assert!(!page.dirty);
    assert!(page.error.is_none(), "{:?}", page.error);
    assert!(page.status.starts_with("Saved and applied."));
  });
  for (handle, terminal) in windows.into_iter().zip(terminals) {
    handle
      .update(cx, |main, _, _| {
        assert_eq!(main.active_terminal().unwrap().entity_id(), terminal);
        assert!(main.vertical_tabbar_render_width > gpui::px(0.0));
      })
      .unwrap();
  }
  windows[0]
    .update(cx, |main, _, _| assert!(main.settings_page.is_some()))
    .unwrap();
  assert_eq!(calls.lock().unwrap().programs.len(), factory_count);
  let saved = files.load();
  assert_eq!(saved.config().font.size, 24.0);
  assert_eq!(saved.config().appearance.background_opacity, 0.8);
  clear_terminal_session_factory_for_testing();
}

#[gpui::test]
fn settings_search_and_save_controls_remain_inside_small_windows(cx: &mut TestAppContext) {
  let window = page_with_defaults(cx);
  for (width, height) in [(800.0, 600.0), (600.0, 420.0)] {
    cx.simulate_window_resize(window.into(), gpui::size(gpui::px(width), gpui::px(height)));
    for pending in [None, Some(PendingAction::Close)] {
      window
        .update(cx, |page, _, cx| {
          page.pending = pending;
          cx.notify();
        })
        .unwrap();
      cx.run_until_parked();
      let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
      for selector in ["settings-search", "settings-footer"] {
        let bounds = visual
          .debug_bounds(selector)
          .expect("control should render");
        assert!(bounds.origin.y >= gpui::px(0.0), "{selector}: {bounds:?}");
        assert!(
          bounds.bottom() <= gpui::px(height),
          "{selector}: {bounds:?}"
        );
        assert!(
          bounds.size.height >= gpui::px(30.0),
          "{selector}: {bounds:?}"
        );
      }
    }
  }
}

fn page_with_defaults(cx: &mut TestAppContext) -> WindowHandle<SettingsPage> {
  crate::test_support::init_test_app(cx);
  cx.add_window(|window, cx| {
    let mut page = SettingsPage::new(window, cx);
    let config = cx.global::<Config>().clone();
    page.form = Some(page.make_form(&config, window, cx).unwrap());
    page
  })
}

#[gpui::test]
fn settings_numeric_inputs_reject_invalid_edits_and_allow_partial_numbers(cx: &mut TestAppContext) {
  let window = page_with_defaults(cx);
  window
    .update(cx, |page, window, cx| {
      for (spec, input) in FIELDS.iter().zip(&page.form.as_ref().unwrap().fields) {
        if !matches!(spec.kind, FieldKind::Number { .. }) {
          continue;
        }
        input.state.update(cx, |state, cx| {
          let original = state.value().to_string();
          for invalid in ["a", "12px", "1e3", "-1", "1,2", "1.2.3"] {
            state.replace_text_in_range(Some(0..original.len()), invalid, window, cx);
            assert_eq!(state.value().as_str(), original, "{}: {invalid}", spec.key);
          }
          state.replace_and_mark_text_in_range(Some(0..original.len()), "abc", None, window, cx);
          assert_eq!(state.value().as_str(), original, "{}", spec.key);
        });
      }
    })
    .unwrap();
  cx.run_until_parked();
  window
    .update(cx, |page, window, cx| {
      assert!(!page.dirty, "rejected input must not change the draft");
      for (spec, input) in FIELDS.iter().zip(&page.form.as_ref().unwrap().fields) {
        let FieldKind::Number { integer, .. } = spec.kind else {
          continue;
        };
        input.state.update(cx, |state, cx| {
          let accepted: &[&str] = if integer {
            &["", "7", "007"]
          } else {
            &["", ".", ".5", "7.", "7.5"]
          };
          for text in accepted {
            let len = state.value().len();
            state.replace_text_in_range(Some(0..len), text, window, cx);
            assert_eq!(state.value().as_str(), *text, "{}", spec.key);
          }
          let before = state.value().to_string();
          let end = before.len();
          state.replace_text_in_range(Some(end..end), ".", window, cx);
          assert_eq!(state.value().as_str(), before, "{}", spec.key);
          state.replace_text_in_range(Some(0..end), "12", window, cx);
          state.replace_text_in_range(Some(1..2), "3", window, cx);
          assert_eq!(state.value().as_str(), "13", "{}", spec.key);
        });
      }
    })
    .unwrap();
}

#[gpui::test]
fn settings_numeric_inputs_filter_clipboard_paste(cx: &mut TestAppContext) {
  let window = page_with_defaults(cx);
  let input = window
    .update(cx, |page, window, cx| {
      page.section = Section::Font;
      let ix = FIELDS
        .iter()
        .position(|spec| spec.table == "font" && spec.key == "size")
        .unwrap();
      let input = page.form.as_ref().unwrap().fields[ix].state.clone();
      window.focus(&input.focus_handle(cx), cx);
      cx.notify();
      input
    })
    .unwrap();
  cx.run_until_parked();
  for (paste, expected) in [("24px", "18"), ("24.5", "24.5"), ("24.5.6", "24.5")] {
    window
      .update(cx, |_, window, cx| {
        input.update(cx, |state, cx| state.select_all(window, cx));
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(paste.into()));
        window.dispatch_action(Box::new(gpui_kit::component::input::Paste), cx);
      })
      .unwrap();
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(input.read(cx).value().as_str(), expected));
  }
}

#[gpui::test]
fn settings_draft_isolated_and_validates_across_categories(cx: &mut TestAppContext) {
  let window = page_with_defaults(cx);
  cx.run_until_parked();
  window
    .update(cx, |page, window, cx| {
      let ix = FIELDS
        .iter()
        .position(|spec| spec.table == "font" && spec.key == "size")
        .unwrap();
      page.set_field(ix, "24".into(), window, cx);
      assert!(page.dirty);
      assert_eq!(page.build_config(cx).unwrap().font.size, 24.0);
      assert_ne!(cx.global::<Config>().font.size, 24.0);
      page.section = Section::Profiles;
      assert_eq!(page.form.as_ref().unwrap().fields[ix].value(cx), "24");
      page.set_field(ix, "0".into(), window, cx);
      assert!(page.build_config(cx).is_err());
      page.request_close(window, cx);
      assert_eq!(page.pending, Some(PendingAction::Close));
      page.save(window, cx);
      assert!(page.error.is_some());
      assert_eq!(page.pending, Some(PendingAction::Close));
      page.pending = None;
      assert!(page.dirty);
    })
    .unwrap();
}

#[gpui::test]
fn settings_preserve_unedited_text_and_clamped_values(cx: &mut TestAppContext) {
  let window = page_with_defaults(cx);
  window
    .update(cx, |page, window, cx| {
      let mut config = cx.global::<Config>().clone();
      config.terminal.working_directory = Some(" directory with spaces ".into());
      config.terminal.scrollback_lines = 200_000;
      config
        .terminal
        .env
        .insert("PRESERVE ".into(), " value ".into());
      page.form = Some(page.make_form(&config, window, cx).unwrap());
      let ix = FIELDS
        .iter()
        .position(|spec| spec.table == "font" && spec.key == "size")
        .unwrap();
      page.set_field(ix, "24".into(), window, cx);
      let edited = page.build_config(cx).unwrap();
      assert_eq!(
        edited.terminal.working_directory,
        config.terminal.working_directory
      );
      assert_eq!(edited.terminal.scrollback_lines, 200_000);
      assert_eq!(edited.terminal.env, config.terminal.env);
    })
    .unwrap();
}

#[gpui::test]
fn settings_categories_render_and_close_returns_to_same_terminal(cx: &mut TestAppContext) {
  use crate::components::MainWindow;
  use crate::components::main_window_e2e_tests::{install_fake_factory, test_lock};
  use crate::components::terminal_window::clear_terminal_session_factory_for_testing;

  let _guard = test_lock();
  crate::test_support::init_test_app(cx);
  let calls = install_fake_factory();
  let window = cx.add_window(|window, cx| MainWindow::new(window, cx));
  cx.simulate_window_resize(window.into(), gpui::size(gpui::px(800.0), gpui::px(600.0)));
  cx.run_until_parked();
  let (terminal, count) = window
    .update(cx, |main, window, cx| {
      let terminal = main.active_terminal().unwrap();
      let page = cx.new(|cx| {
        let mut page = SettingsPage::new(window, cx);
        let config = cx.global::<Config>().clone();
        page.form = Some(page.make_form(&config, window, cx).unwrap());
        page
      });
      main.attach_settings_page(page, window, cx);
      (terminal, main.items.len())
    })
    .unwrap();

  for section in Section::ALL {
    window
      .update(cx, |main, _, cx| {
        main.settings_page.as_ref().unwrap().update(cx, |page, cx| {
          page.section = section;
          cx.notify();
        });
      })
      .unwrap();
    cx.run_until_parked();
    let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
    let footer = visual.debug_bounds("settings-footer").unwrap();
    assert!(
      footer.bottom() <= gpui::px(600.0),
      "{section:?}: {footer:?}"
    );
  }
  let factory_count = calls.lock().unwrap().programs.len();
  let terminal_shortcuts = cx.update(|cx| {
    let config = cx.global::<Config>();
    format!(
      "{} {}",
      config.keybindings.new_tab.first().unwrap(),
      config.keybindings.split_horizontal.first().unwrap()
    )
  });
  cx.simulate_keystrokes(window.into(), &terminal_shortcuts);
  cx.run_until_parked();
  window
    .update(cx, |main, window, cx| {
      assert_eq!(main.items.len(), count);
      main.focus_active_terminal(window, cx);
      assert!(!terminal.focus_handle(cx).is_focused(window));
    })
    .unwrap();
  cx.simulate_keystrokes(window.into(), "escape");
  cx.run_until_parked();
  window
    .update(cx, |main, window, cx| {
      assert!(main.settings_page.is_none());
      assert_eq!(main.items.len(), count);
      assert_eq!(
        main.active_terminal().unwrap().entity_id(),
        terminal.entity_id()
      );
      assert!(terminal.focus_handle(cx).is_focused(window));
    })
    .unwrap();
  assert_eq!(calls.lock().unwrap().programs.len(), factory_count);
  clear_terminal_session_factory_for_testing();
}

#[gpui::test]
fn settings_draft_survives_last_background_terminal_exit(cx: &mut TestAppContext) {
  use crate::components::MainWindow;
  use crate::components::main_window_e2e_tests::{install_fake_factory, test_lock};
  use crate::components::terminal_window::clear_terminal_session_factory_for_testing;

  let _guard = test_lock();
  crate::test_support::init_test_app(cx);
  let calls = install_fake_factory();
  let window = cx.add_window(|window, cx| MainWindow::new(window, cx));
  cx.run_until_parked();
  window
    .update(cx, |main, window, cx| {
      let page = cx.new(|cx| {
        let mut page = SettingsPage::new(window, cx);
        let config = cx.global::<Config>().clone();
        page.form = Some(page.make_form(&config, window, cx).unwrap());
        page.dirty = true;
        page
      });
      main.attach_settings_page(page, window, cx);
      let index = main.items[0].index;
      main.remove_tab_by(index, window, cx);
    })
    .unwrap();
  cx.run_until_parked();
  let before = calls.lock().unwrap().programs.len();
  window
    .update(cx, |main, window, cx| {
      assert!(main.items.is_empty());
      let page = main.settings_page.as_ref().unwrap();
      assert!(page.read(cx).dirty);
      page.update(cx, |page, cx| {
        page.request_close(window, cx);
        assert_eq!(page.pending, Some(PendingAction::Close));
        page.perform(PendingAction::Close, window, cx);
      });
    })
    .unwrap();
  cx.run_until_parked();
  window
    .update(cx, |main, _, _| {
      assert!(main.settings_page.is_none());
      assert_eq!(main.items.len(), 1);
    })
    .unwrap();
  assert_eq!(calls.lock().unwrap().programs.len(), before + 1);
  clear_terminal_session_factory_for_testing();
}
