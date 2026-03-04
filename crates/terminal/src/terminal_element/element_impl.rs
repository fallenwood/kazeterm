use alacritty_terminal::{
  grid::Dimensions,
  vte::ansi::CursorShape as AlacCursorShape,
};
use gpui::{
  AbsoluteLength, App, Bounds, Element, FontFeatures, FontStyle, FontWeight, HighlightStyle,
  MouseButton, Pixels, Point, TextRun, TextStyle, UnderlineStyle, WhiteSpace,
  Window, fill, px, relative,
};
use themeing::ActiveTheme as _;

use crate::{
  cursor_layout::CursorLayout,
  highlighted_range_line::HighlightedRange,
  scrollbar::{MIN_THUMB_HEIGHT, SCROLLBAR_WIDTH, ScrollbarState, paint_scrollbar},
  terminal_input_handler::TerminalInputHandler,
};

use super::TerminalBounds;
use super::TerminalContent;
use super::helpers::{DisplayCursor, to_highlighted_range_lines};
use super::{LayoutState, TerminalElement};

impl Element for TerminalElement {
  type RequestLayoutState = ();
  type PrepaintState = LayoutState;

  fn id(&self) -> Option<gpui::ElementId> {
    self.interactivity.element_id.clone()
  }

  fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
    None
  }

  fn request_layout(
    &mut self,
    global_id: Option<&gpui::GlobalElementId>,
    inspector_id: Option<&gpui::InspectorElementId>,
    window: &mut Window,
    cx: &mut gpui::App,
  ) -> (gpui::LayoutId, Self::RequestLayoutState) {
    self.interactivity.occlude_mouse();
    let height: gpui::Length = relative(1.).into();

    let layout_id = self.interactivity.request_layout(
      global_id,
      inspector_id,
      window,
      cx,
      |mut style, window, cx| {
        style.size.width = relative(1.).into();
        style.size.height = height;
        window.request_layout(style, None, cx)
      },
    );
    (layout_id, ())
  }

  fn prepaint(
    &mut self,
    global_id: Option<&gpui::GlobalElementId>,
    inspector_id: Option<&gpui::InspectorElementId>,
    bounds: gpui::Bounds<gpui::Pixels>,
    _: &mut Self::RequestLayoutState,
    window: &mut Window,
    cx: &mut gpui::App,
  ) -> Self::PrepaintState {
    self.interactivity.prepaint(
      global_id,
      inspector_id,
      bounds,
      bounds.size,
      window,
      cx,
      |_, _, hitbox, window, cx: &mut App| {
        let hitbox = hitbox.unwrap();
        let config = cx.global::<::config::Config>();
        let zoom_state = cx.global::<themeing::ZoomState>();

        let font_family = gpui::SharedString::from(config.font_family.clone());
        let line_height_multiplier = 1.18_f32;
        let effective_font_size = zoom_state.effective_font_size(config.font_size);
        let font_size = AbsoluteLength::from(Pixels::from(effective_font_size));
        let font_weight = FontWeight::NORMAL;
        let font_features = FontFeatures::default();

        let minimum_contrast = 45.0;

        let theme = cx.theme().clone();

        let text_style = TextStyle {
          font_family,
          font_features,
          font_weight,
          font_fallbacks: None,
          font_size,
          font_style: FontStyle::Normal,
          line_height: relative(line_height_multiplier),
          background_color: Some(theme.colors().terminal_ansi_background),
          white_space: WhiteSpace::Normal,
          color: theme.colors().terminal_foreground,
          ..Default::default()
        };

        let text_system = cx.text_system();
        let gutter;
        let scrollbar_width = px(SCROLLBAR_WIDTH);
        let (dimensions, _line_height_px) = {
          let rem_size = window.rem_size();
          let font_pixels = text_style.font_size.to_pixels(rem_size);
          let line_height = font_pixels * line_height_multiplier;
          let font_id = cx.text_system().resolve_font(&text_style.font());

          let cell_width = text_system
            .advance(font_id, font_pixels, 'm')
            .unwrap()
            .width;
          gutter = cell_width;

          let mut size = bounds.size;
          size.width -= gutter + scrollbar_width;

          if size.width < cell_width * 2.0 {
            size.width = cell_width * 2.0;
          }

          let mut origin = bounds.origin;
          origin.x += gutter;

          (
            TerminalBounds::new(line_height, cell_width, Bounds { origin, size }),
            line_height,
          )
        };

        let background_color = theme.colors().terminal_ansi_background;

        self.terminal.update(cx, |terminal, cx| {
          terminal.set_size(dimensions);
          terminal.sync(window, cx);
        });

        let TerminalContent {
          cells,
          mode,
          display_offset,
          cursor_char,
          selection,
          cursor,
          search_matches,
          current_search_match_index,
          last_hovered_word,
          history_size,
          ..
        } = &self.terminal.read(cx).last_content;

        let mode = *mode;
        let display_offset = *display_offset;
        let history_size = *history_size;
        let current_match_idx = *current_search_match_index;

        let link_style = HighlightStyle {
          color: Some(theme.colors().link_text_hover),
          font_weight: Some(font_weight),
          font_style: None,
          background_color: None,
          underline: Some(UnderlineStyle {
            thickness: px(1.0),
            color: Some(theme.colors().link_text_hover),
            wavy: false,
          }),
          strikethrough: None,
          fade_out: None,
        };

        let mut relative_highlighted_ranges = Vec::new();
        for (idx, search_match) in search_matches.iter().enumerate() {
          let color = if idx + 1 == current_match_idx {
            theme.colors().search_match_background
          } else {
            theme.colors().search_highlight_background
          };
          relative_highlighted_ranges.push((search_match.clone(), color));
        }

        if let Some(selection) = selection {
          let selection_color = cx.theme().colors().element_selection_background;
          relative_highlighted_ranges.push((selection.start..=selection.end, selection_color));
        }

        let (rects, batched_text_runs) = TerminalElement::layout_grid(
          cells.iter().cloned(),
          0,
          &text_style,
          last_hovered_word
            .as_ref()
            .map(|last_hovered_word| (link_style, &last_hovered_word.word_match)),
          minimum_contrast,
          cx,
        );

        let cursor = if let AlacCursorShape::Hidden = cursor.shape {
          None
        } else {
          let cursor_point = DisplayCursor::from(cursor.point, display_offset);
          let cursor_color = theme.colors().terminal_cursor;
          let cursor_text: gpui::ShapedLine = {
            let str_trxt = cursor_char.to_string();
            let len = str_trxt.len();
            window.text_system().shape_line(
              str_trxt.into(),
              text_style.font_size.to_pixels(window.rem_size()),
              &[TextRun {
                len,
                font: text_style.font(),
                color: theme.colors().terminal_ansi_background,
                background_color: None,
                underline: Default::default(),
                strikethrough: None,
              }],
              None,
            )
          };

          let focused = self.focused;
          TerminalElement::shape_cursor(cursor_point, dimensions, &cursor_text).map(
            move |(cursor_position, block_width)| {
              let (shape, text) = match cursor.shape {
                AlacCursorShape::Block if !focused => (AlacCursorShape::HollowBlock, None),
                AlacCursorShape::Block => (AlacCursorShape::Block, Some(cursor_text)),
                AlacCursorShape::Underline => (AlacCursorShape::Underline, None),
                AlacCursorShape::Beam => (AlacCursorShape::Beam, None),
                AlacCursorShape::HollowBlock => (AlacCursorShape::HollowBlock, None),
                AlacCursorShape::Hidden => unreachable!(),
              };

              CursorLayout::new(
                cursor_position,
                block_width,
                dimensions.line_height,
                cursor_color,
                shape,
                text,
              )
            },
          )
        };

        let visible_lines = dimensions.screen_lines();
        let total_lines = visible_lines + history_size;
        let scrollbar_state =
          ScrollbarState::new(total_lines, visible_lines, display_offset, history_size);

        let scrollbar_bounds = if scrollbar_state.should_show() {
          Some(Bounds {
            origin: Point {
              x: bounds.origin.x + bounds.size.width - scrollbar_width,
              y: bounds.origin.y,
            },
            size: gpui::Size {
              width: scrollbar_width,
              height: bounds.size.height,
            },
          })
        } else {
          None
        };

        LayoutState {
          hitbox,
          batched_text_runs,
          cursor,
          background_color,
          dimensions,
          rects,
          relative_highlighted_ranges,
          mode,
          display_offset,
          gutter,
          base_text_style: text_style,
          scrollbar_state: if scrollbar_state.should_show() {
            Some(scrollbar_state)
          } else {
            None
          },
          scrollbar_bounds,
        }
      },
    )
  }

  fn paint(
    &mut self,
    global_id: Option<&gpui::GlobalElementId>,
    inspector_id: Option<&gpui::InspectorElementId>,
    bounds: gpui::Bounds<gpui::Pixels>,
    _: &mut Self::RequestLayoutState,
    layout: &mut Self::PrepaintState,
    window: &mut Window,
    cx: &mut gpui::App,
  ) {
    window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
      let scroll_top = self.terminal_view.read(cx).scroll_top;

      window.paint_quad(fill(bounds, layout.background_color));
      let origin =
        bounds.origin + Point::new(layout.gutter, px(0.)) - Point::new(px(0.), scroll_top);

      let marked_text_cloned: Option<String> = {
        let ime_state = &self.terminal_view.read(cx).ime_state;
        ime_state.as_ref().map(|state| state.marked_text.clone())
      };

      let terminal_input_handler = TerminalInputHandler {
        terminal: self.terminal.clone(),
        terminal_view: self.terminal_view.clone(),
        cursor_bounds: layout
          .cursor
          .as_ref()
          .map(|cursor| cursor.bounding_rect(origin)),
      };

      self.register_mouse_listeners(layout.mode, &layout.hitbox, layout.scrollbar_bounds, window);
      if window.modifiers().secondary()
        && bounds.contains(&window.mouse_position())
        && self.terminal_view.read(cx).hover.is_some()
      {
        window.set_cursor_style(gpui::CursorStyle::PointingHand, &layout.hitbox);
      } else {
        window.set_cursor_style(gpui::CursorStyle::IBeam, &layout.hitbox);
      }

      let original_cursor = layout.cursor.take();
      self.interactivity.paint(
        global_id,
        inspector_id,
        bounds,
        Some(&layout.hitbox),
        window,
        cx,
        |_, window, cx| {
          window.handle_input(&self.focus, terminal_input_handler, cx);

          for rect in &layout.rects {
            rect.paint(origin, &layout.dimensions, window);
          }

          for (relative_highlighted_range, color) in layout.relative_highlighted_ranges.iter() {
            if let Some((start_y, highlighted_range_lines)) =
              to_highlighted_range_lines(relative_highlighted_range, layout, origin)
            {
              let corner_radius = 0.15 * layout.dimensions.line_height;
              let hr = HighlightedRange {
                start_y,
                line_height: layout.dimensions.line_height,
                lines: highlighted_range_lines,
                color: *color,
                corner_radius,
              };
              hr.paint(true, bounds, window);
            }
          }

          for batch in &layout.batched_text_runs {
            batch.paint(origin, &layout.dimensions, window, cx);
          }

          if let Some(text_to_mark) = &marked_text_cloned
            && !text_to_mark.is_empty()
            && let Some(cursor_layout) = &original_cursor
          {
            let ime_position = cursor_layout.bounding_rect(origin).origin;
            let mut ime_style = layout.base_text_style.clone();
            ime_style.underline = Some(UnderlineStyle {
              color: Some(ime_style.color),
              thickness: px(1.0),
              wavy: false,
            });

            let shaped_line = window.text_system().shape_line(
              text_to_mark.clone().into(),
              ime_style.font_size.to_pixels(window.rem_size()),
              &[TextRun {
                len: text_to_mark.len(),
                font: ime_style.font(),
                color: ime_style.color,
                background_color: None,
                underline: ime_style.underline,
                strikethrough: None,
              }],
              None,
            );

            shaped_line
              .paint(ime_position, layout.dimensions.line_height, window, cx)
              .unwrap_or_default();
          }

          if self.cursor_visible
            && let Some(mut cursor) = original_cursor
          {
            cursor.paint(origin, window, cx);
          }
        },
      );

      // Paint scrollbar outside the main terminal content area
      if let (Some(scrollbar_state), Some(scrollbar_bounds)) =
        (&layout.scrollbar_state, &layout.scrollbar_bounds)
      {
        let theme = cx.theme();
        let track_color = theme.colors().scrollbar_track_background;
        let thumb_color = theme.colors().scrollbar_thumb_background;
        let hovered = scrollbar_bounds.contains(&window.mouse_position());
        paint_scrollbar(
          *scrollbar_bounds,
          scrollbar_state,
          track_color,
          thumb_color,
          hovered,
          window,
        );

        let scrollbar_bounds = *scrollbar_bounds;
        let scrollbar_state_for_down = scrollbar_state.clone();
        let terminal_for_down = self.terminal.clone();
        let terminal_view_for_down = self.terminal_view.clone();
        window.on_mouse_event(move |e: &gpui::MouseDownEvent, _phase, _window, cx| {
          if e.button == MouseButton::Left && scrollbar_bounds.contains(&e.position) {
            let relative_y = e.position.y - scrollbar_bounds.origin.y;
            let position_ratio = relative_y / scrollbar_bounds.size.height;

            if scrollbar_state_for_down.is_on_thumb(position_ratio, scrollbar_bounds.size.height) {
              let (thumb_top_px, _) =
                scrollbar_state_for_down.thumb_pixel_bounds(scrollbar_bounds.size.height);
              let click_offset_from_thumb_px: f32 = (relative_y - thumb_top_px).into();
              let mouse_y: f32 = relative_y.into();
              terminal_view_for_down.update(cx, |view, cx| {
                view.scrollbar_drag_state = Some((click_offset_from_thumb_px, mouse_y));
                cx.notify();
              });
            } else {
              let new_offset = scrollbar_state_for_down.position_to_offset(position_ratio);
              terminal_for_down.update(cx, |terminal, cx| {
                terminal.scroll(alacritty_terminal::grid::Scroll::Delta(
                  new_offset as i32 - terminal.last_content.display_offset as i32,
                ));
                cx.notify();
              });
            }
          }
        });

        const MIN_DRAG_DELTA_PX: f32 = 3.0;

        let scrollbar_bounds_for_move = scrollbar_bounds;
        let terminal_for_move = self.terminal.clone();
        let terminal_view_for_move = self.terminal_view.clone();
        window.on_mouse_event(move |e: &gpui::MouseMoveEvent, _phase, _window, cx| {
          let drag_state = terminal_view_for_move.read(cx).scrollbar_drag_state;

          if let Some((click_offset_from_thumb_px, last_mouse_y)) = drag_state
            && e.pressed_button == Some(MouseButton::Left) {
              let relative_y = e.position.y - scrollbar_bounds_for_move.origin.y;
              let current_mouse_y: f32 = relative_y.into();

              if (current_mouse_y - last_mouse_y).abs() < MIN_DRAG_DELTA_PX {
                return;
              }

              let thumb_top_px = px(current_mouse_y - click_offset_from_thumb_px);
              let track_height = scrollbar_bounds_for_move.size.height;

              let history_size = terminal_for_move.read(cx).last_content.history_size;

              if history_size > 0 {
                let terminal_content = &terminal_for_move.read(cx).last_content;
                let visible_lines = terminal_content.terminal_bounds.num_lines();
                let total_lines = visible_lines + history_size;

                let thumb_size_ratio = visible_lines as f32 / total_lines as f32;
                let thumb_height = (track_height * thumb_size_ratio).max(px(MIN_THUMB_HEIGHT));
                let scrollable_height = track_height - thumb_height;

                if scrollable_height > px(0.0) {
                  let thumb_top_clamped = thumb_top_px.clamp(px(0.0), scrollable_height);
                  let normalized: f32 = thumb_top_clamped / scrollable_height;

                  let new_offset = ((1.0 - normalized) * history_size as f32) as usize;

                  let current_offset = terminal_content.display_offset;
                  if new_offset != current_offset {
                    terminal_for_move.update(cx, |terminal, cx| {
                      terminal.scroll(alacritty_terminal::grid::Scroll::Delta(
                        new_offset as i32 - terminal.last_content.display_offset as i32,
                      ));
                      cx.notify();
                    });
                  }
                }

                terminal_view_for_move.update(cx, |view, _| {
                  if let Some((offset, _)) = view.scrollbar_drag_state {
                    view.scrollbar_drag_state = Some((offset, current_mouse_y));
                  }
                });
              }
            }
        });

        let terminal_view_for_up = self.terminal_view.clone();
        window.on_mouse_event(move |e: &gpui::MouseUpEvent, _phase, _window, cx| {
          if e.button == MouseButton::Left {
            let is_dragging = terminal_view_for_up.read(cx).scrollbar_drag_state.is_some();
            if is_dragging {
              terminal_view_for_up.update(cx, |view, cx| {
                view.scrollbar_drag_state = None;
                cx.notify();
              });
            }
          }
        });
      }
    });
  }
}
