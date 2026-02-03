use std::ops::RangeInclusive;

use crate::{
  background_region::BackgroundRegion,
  cursor_layout::CursorLayout,
  highlighted_range_line::{HighlightedRange, HighlightedRangeLine},
  scrollbar::{SCROLLBAR_WIDTH, ScrollbarState, paint_scrollbar},
  terminal_input_handler::TerminalInputHandler,
};
use alacritty_terminal::{
  grid::Dimensions,
  index::Point as AlacPoint,
  term::{TermMode, cell::Flags},
  vte::ansi::{Color, CursorShape as AlacCursorShape, NamedColor},
};
use gpui::{
  AbsoluteLength, App, Bounds, Context, Element, Entity, FocusHandle, Font, FontFeatures,
  FontStyle, FontWeight, HighlightStyle, Hsla, InteractiveElement, IntoElement, MouseButton,
  Pixels, Point, ShapedLine, StatefulInteractiveElement, StrikethroughStyle, TextRun, TextStyle,
  UnderlineStyle, WhiteSpace, Window, fill, point, px, relative,
};
use itertools::Itertools;
use themeing::{ActiveTheme as _, convert_color};

use super::batched_text_run::BatchedTextRun;
use super::layout_rect::LayoutRect;
use super::terminal_bounds::TerminalBounds;
use super::terminal_content::TerminalContent;
use super::terminal_view::TerminalView;
use super::{indexed_cell::IndexedCell, terminal::Terminal};
pub struct TerminalElement {
  terminal: Entity<Terminal>,
  terminal_view: Entity<TerminalView>,
  focus: FocusHandle,
  focused: bool,
  cursor_visible: bool,
  interactivity: gpui::Interactivity,
  // block_below_cursor: Option<Rc<BlockProperties>>,
}

impl TerminalElement {
  pub fn new(
    terminal: Entity<Terminal>,
    terminal_view: Entity<TerminalView>,
    focus: FocusHandle,
    focused: bool,
    cursor_visible: bool,
    interactivity: gpui::Interactivity,
  ) -> Self {
    Self {
      terminal,
      terminal_view,
      focus: focus.clone(),
      focused,
      cursor_visible,
      interactivity,
    }
    .track_focus(&focus)
  }

  pub fn layout_grid(
    grid: impl Iterator<Item = IndexedCell>,
    start_line_offset: i32,
    text_style: &TextStyle,
    hyperlink: Option<(HighlightStyle, &RangeInclusive<AlacPoint>)>,
    minimum_contrast: f32,
    cx: &App,
  ) -> (Vec<LayoutRect>, Vec<BatchedTextRun>) {
    let theme = cx.theme();

    // Pre-allocate with estimated capacity to reduce reallocations
    let estimated_cells = grid.size_hint().0;
    let estimated_runs = estimated_cells / 10; // Estimate ~10 cells per run
    let estimated_regions = estimated_cells / 20; // Estimate ~20 cells per background region

    let mut batched_runs = Vec::with_capacity(estimated_runs);

    // Collect background regions for efficient merging
    let mut background_regions: Vec<BackgroundRegion> = Vec::with_capacity(estimated_regions);
    let mut current_batch: Option<BatchedTextRun> = None;

    // First pass: collect all cells and their backgrounds
    let linegroups = grid.into_iter().chunk_by(|i| i.point.line);
    for (line_index, (_, line)) in linegroups.into_iter().enumerate() {
      let alac_line = start_line_offset + line_index as i32;

      // Flush any existing batch at line boundaries
      if let Some(batch) = current_batch.take() {
        batched_runs.push(batch);
      }

      let mut previous_cell_had_extras = false;

      for cell in line {
        let mut fg = cell.fg;
        let mut bg = cell.bg;
        if cell.flags.contains(Flags::INVERSE) {
          std::mem::swap(&mut fg, &mut bg);
        }

        // Collect background regions (skip default background)
        if !matches!(bg, Color::Named(NamedColor::Background)) {
          let color = convert_color(&bg, theme);
          let col = cell.point.column.0 as i32;

          // Try to extend the last region if it's on the same line with the same color
          if let Some(last_region) = background_regions.last_mut() {
            if last_region.color == color
              && last_region.start_line == alac_line
              && last_region.end_line == alac_line
              && last_region.end_col + 1 == col
            {
              last_region.end_col = col;
            } else {
              background_regions.push(BackgroundRegion::new(alac_line, col, color));
            }
          } else {
            background_regions.push(BackgroundRegion::new(alac_line, col, color));
          }
        }

        // Skip wide character spacers - they're just placeholders for the second cell of wide characters
        if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
          continue;
        }

        // Skip spaces that follow cells with extras (emoji variation sequences)
        if cell.c == ' ' && previous_cell_had_extras {
          previous_cell_had_extras = false;
          continue;
        }
        // Update tracking for next iteration
        previous_cell_had_extras = cell.extra.is_some();

        //Layout current cell text
        {
          if !is_blank(&cell) {
            let cell_style = TerminalElement::cell_style(
              &cell,
              fg,
              bg,
              theme,
              text_style,
              hyperlink,
              minimum_contrast,
            );

            let cell_point = AlacPoint::new(alac_line, cell.point.column.0 as i32);

            // Try to batch with existing run
            if let Some(ref mut batch) = current_batch {
              if batch.can_append(&cell_style)
                && batch.start_point.line == cell_point.line
                && batch.start_point.column + batch.cell_count as i32 == cell_point.column
              {
                batch.append_char(cell.c);
              } else {
                // Flush current batch and start new one
                let old_batch = current_batch.take().unwrap();
                batched_runs.push(old_batch);
                current_batch = Some(BatchedTextRun::new_from_char(
                  cell_point,
                  cell.c,
                  cell_style,
                  text_style.font_size,
                ));
              }
            } else {
              // Start new batch
              current_batch = Some(BatchedTextRun::new_from_char(
                cell_point,
                cell.c,
                cell_style,
                text_style.font_size,
              ));
            }
          };
        }
      }
    }

    // Flush any remaining batch
    if let Some(batch) = current_batch {
      batched_runs.push(batch);
    }

    // Second pass: merge background regions and convert to layout rects
    let merged_regions = merge_background_regions(background_regions);
    let mut rects = Vec::with_capacity(merged_regions.len() * 2); // Estimate 2 rects per merged region

    // Convert merged regions to layout rects
    // Since LayoutRect only supports single-line rectangles, we need to split multi-line regions
    for region in merged_regions {
      for line in region.start_line..=region.end_line {
        rects.push(LayoutRect::new(
          AlacPoint::new(line, region.start_col),
          (region.end_col - region.start_col + 1) as usize,
          region.color,
        ));
      }
    }

    (rects, batched_runs)
  }

  /// Converts the Alacritty cell styles to GPUI text styles and background color.
  fn cell_style(
    indexed: &IndexedCell,
    fg: alacritty_terminal::vte::ansi::Color,
    bg: alacritty_terminal::vte::ansi::Color,
    colors: &themeing::Theme,
    text_style: &TextStyle,
    hyperlink: Option<(HighlightStyle, &RangeInclusive<AlacPoint>)>,
    minimum_contrast: f32,
  ) -> TextRun {
    let flags = indexed.cell.flags;
    let mut fg = convert_color(&fg, colors);
    let bg = convert_color(&bg, colors);

    // // Only apply contrast adjustment to non-decorative characters
    if !is_decorative_character(indexed.c) {
      fg = crate::apca_contrast::ensure_minimum_contrast(fg, bg, minimum_contrast);
    }

    // Ghostty uses (175/255) as the multiplier (~0.69), Alacritty uses 0.66, Kitty
    // uses 0.75. We're using 0.7 because it's pretty well in the middle of that.
    if flags.intersects(Flags::DIM) {
      fg.a *= 0.7;
    }

    let underline = (flags.intersects(Flags::ALL_UNDERLINES) || indexed.cell.hyperlink().is_some())
      .then(|| UnderlineStyle {
        color: Some(fg),
        thickness: Pixels::from(1.0),
        wavy: flags.contains(Flags::UNDERCURL),
      });

    let strikethrough = flags
      .intersects(Flags::STRIKEOUT)
      .then(|| StrikethroughStyle {
        color: Some(fg),
        thickness: Pixels::from(1.0),
      });

    let weight = if flags.intersects(Flags::BOLD) {
      FontWeight::BOLD
    } else {
      text_style.font_weight
    };

    let style = if flags.intersects(Flags::ITALIC) {
      FontStyle::Italic
    } else {
      FontStyle::Normal
    };

    let mut result = TextRun {
      len: indexed.c.len_utf8(),
      color: fg,
      background_color: None,
      font: Font {
        weight,
        style,
        ..text_style.font()
      },
      underline,
      strikethrough,
    };

    if let Some((style, range)) = hyperlink
      && range.contains(&indexed.point)
    {
      if let Some(underline) = style.underline {
        result.underline = Some(underline);
      }

      if let Some(color) = style.color {
        result.color = color;
      }
    }

    result
  }

  fn shape_cursor(
    cursor_point: DisplayCursor,
    size: TerminalBounds,
    text_fragment: &ShapedLine,
  ) -> Option<(Point<Pixels>, Pixels)> {
    if cursor_point.line() < size.total_lines() as i32 {
      let cursor_width = if text_fragment.width == Pixels::ZERO {
        size.cell_width()
      } else {
        text_fragment.width
      };

      // Cursor should always surround as much of the text as possible,
      // hence when on pixel boundaries round the origin down and the width up
      Some((
        point(
          (cursor_point.col() as f32 * size.cell_width()).floor(),
          (cursor_point.line() as f32 * size.line_height()).floor(),
        ),
        cursor_width.ceil(),
      ))
    } else {
      None
    }
  }

  fn generic_button_handler<E>(
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

  fn register_mouse_listeners(
    &mut self,
    mode: TermMode,
    hitbox: &gpui::Hitbox,
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
        window.focus(&focus);

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
        // if phase != DispatchPhase::Bubble {
        //     return;
        // }

        if e.pressed_button.is_some() && !cx.has_active_drag() && focus.is_focused(window) {
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
      }
    });

    self.interactivity.on_mouse_up(
      MouseButton::Left,
      TerminalElement::generic_button_handler(
        terminal.clone(),
        focus.clone(),
        false,
        move |terminal, e, cx| {
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
              terminal_view.scroll_wheel(e, cx);
            }
          })
          .ok();
      }
    });

    // Mouse mode handlers:
    // All mouse modes need the extra click handlers
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
      // Non-mouse-mode: right-click for copy/paste
      self.interactivity.on_mouse_down(MouseButton::Right, {
        let terminal = terminal.clone();
        move |_e, _window, cx| {
          let has_selection = terminal.read(cx).last_content.selection.is_some();
          if has_selection {
            // Has selection - copy and clear selection
            terminal.update(cx, |term, cx| {
              term.copy_and_clear_selection(cx);
            });
          } else {
            // No selection - paste
            if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
              terminal.update(cx, |term, _cx| {
                term.input(text.into_bytes());
              });
            }
          }
        }
      });
    }
  }
}

impl InteractiveElement for TerminalElement {
  fn interactivity(&mut self) -> &mut gpui::Interactivity {
    &mut self.interactivity
  }
}

impl StatefulInteractiveElement for TerminalElement {}

/// The information generated during layout that is necessary for painting.
pub struct LayoutState {
  hitbox: gpui::Hitbox,
  batched_text_runs: Vec<BatchedTextRun>,
  rects: Vec<LayoutRect>,
  relative_highlighted_ranges: Vec<(RangeInclusive<AlacPoint>, Hsla)>,
  cursor: Option<CursorLayout>,
  background_color: Hsla,
  dimensions: TerminalBounds,
  mode: TermMode,
  display_offset: usize,
  // hyperlink_tooltip: Option<AnyElement>,
  gutter: Pixels,
  // block_below_cursor_element: Option<AnyElement>,
  base_text_style: TextStyle,
  scrollbar_state: Option<ScrollbarState>,
  scrollbar_bounds: Option<gpui::Bounds<Pixels>>,
}

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
        let line_height_multiplier = 1.1667_f32;
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
          font_size: font_size.into(),
          font_style: FontStyle::Normal,
          line_height: relative(line_height_multiplier).into(),
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
          // Reserve space for gutter and scrollbar
          size.width -= gutter + scrollbar_width;

          // https://github.com/zed-industries/zed/issues/2750
          // if the terminal is one column wide, rendering 🦀
          // causes alacritty to misbehave.
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

        //             let (last_hovered_word, hover_tooltip) =
        self.terminal.update(cx, |terminal, cx| {
          terminal.set_size(dimensions);
          terminal.sync(window, cx);

          //                     if window.modifiers().secondary()
          //                         && bounds.contains(&window.mouse_position())
          //                         && self.terminal_view.read(cx).hover.is_some()
          //                     {
          //                         let registered_hover = self.terminal_view.read(cx).hover.as_ref();
          //                         if terminal.last_content.last_hovered_word.as_ref()
          //                             == registered_hover.map(|hover| &hover.hovered_word)
          //                         {
          //                             (
          //                                 terminal.last_content.last_hovered_word.clone(),
          //                                 registered_hover.map(|hover| hover.tooltip.clone()),
          //                             )
          //                         } else {
          //                             (None, None)
          //                         }
          //                     } else {
          //                         (None, None)
          //                     }
        });

        //             let scroll_top = self.terminal_view.read(cx).scroll_top;
        //             let hyperlink_tooltip = hover_tooltip.map(|hover_tooltip| {
        //                 let offset = bounds.origin + point(gutter, px(0.)) - point(px(0.), scroll_top);
        //                 let mut element = div()
        //                     .size_full()
        //                     .id("terminal-element")
        //                     .tooltip(Tooltip::text(hover_tooltip))
        //                     .into_any_element();
        //                 element.prepaint_as_root(offset, bounds.size.into(), window, cx);
        //                 element
        //             });

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

        // Create link style for hovered links
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

        // Add search matches to highlighted ranges
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

        // Layout cursor. Rectangle is used for IME, so we should lay it out even
        // if we don't end up showing it.
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

        // Calculate scrollbar state
        let visible_lines = dimensions.screen_lines();
        let total_lines = visible_lines + history_size;
        let scrollbar_state =
          ScrollbarState::new(total_lines, visible_lines, display_offset, history_size);

        // Calculate scrollbar bounds (on the right edge of the terminal)
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
          background_color: background_color,
          dimensions,
          rects,
          relative_highlighted_ranges,
          mode,
          display_offset,
          // hyperlink_tooltip,
          gutter,
          // block_below_cursor_element,
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

      self.register_mouse_listeners(layout.mode, &layout.hitbox, window);
      if window.modifiers().secondary()
        && bounds.contains(&window.mouse_position())
        && self.terminal_view.read(cx).hover.is_some()
      {
        window.set_cursor_style(gpui::CursorStyle::PointingHand, &layout.hitbox);
      } else {
        window.set_cursor_style(gpui::CursorStyle::IBeam, &layout.hitbox);
      }

      let original_cursor = layout.cursor.take();
      //         let hyperlink_tooltip = layout.hyperlink_tooltip.take();
      //         let block_below_cursor_element = layout.block_below_cursor_element.take();
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
                corner_radius: corner_radius,
              };
              hr.paint(true, bounds, window);
            }
          }

          // Paint batched text runs instead of individual cells
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

          if self.cursor_visible // && marked_text_cloned.is_none()
                        && let Some(mut cursor) = original_cursor
          {
            cursor.paint(origin, window, cx);
          }

          //                 if let Some(mut element) = block_below_cursor_element {
          //                     element.paint(window, cx);
          //                 }

          //                 if let Some(mut element) = hyperlink_tooltip {
          //                     element.paint(window, cx);
          //                 }
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

        // Add scrollbar click handler
        let scrollbar_bounds = *scrollbar_bounds;
        let scrollbar_state = scrollbar_state.clone();
        let terminal = self.terminal.clone();
        window.on_mouse_event(move |e: &gpui::MouseDownEvent, _phase, _window, cx| {
          if e.button == MouseButton::Left && scrollbar_bounds.contains(&e.position) {
            // Calculate the position ratio within the scrollbar
            let relative_y = e.position.y - scrollbar_bounds.origin.y;
            // Pixels / Pixels gives a scalar f32
            let position_ratio = (relative_y / scrollbar_bounds.size.height) as f32;
            let new_offset = scrollbar_state.position_to_offset(position_ratio);

            terminal.update(cx, |terminal, cx| {
              terminal.scroll(alacritty_terminal::grid::Scroll::Delta(
                new_offset as i32 - terminal.last_content.display_offset as i32,
              ));
              cx.notify();
            });
          }
        });
      }
    });
  }
}

impl IntoElement for TerminalElement {
  type Element = Self;

  fn into_element(self) -> Self::Element {
    self
  }
}

/// Merge background regions to minimize the number of rectangles
fn merge_background_regions(regions: Vec<BackgroundRegion>) -> Vec<BackgroundRegion> {
  if regions.is_empty() {
    return regions;
  }

  let mut merged = regions;
  let mut changed = true;

  // Keep merging until no more merges are possible
  while changed {
    changed = false;
    let mut i = 0;

    while i < merged.len() {
      let mut j = i + 1;
      while j < merged.len() {
        if merged[i].can_merge_with(&merged[j]) {
          let other = merged.remove(j);
          merged[i].merge_with(&other);
          changed = true;
        } else {
          j += 1;
        }
      }
      i += 1;
    }
  }

  merged
}

pub fn is_blank(cell: &IndexedCell) -> bool {
  if cell.c != ' ' {
    return false;
  }

  if cell.bg != Color::Named(NamedColor::Background) {
    return false;
  }

  if cell.hyperlink().is_some() {
    return false;
  }

  if cell
    .flags
    .intersects(Flags::ALL_UNDERLINES | Flags::INVERSE | Flags::STRIKEOUT)
  {
    return false;
  }

  true
}

/// Helper struct for converting data between Alacritty's cursor points, and displayed cursor points.
struct DisplayCursor {
  line: i32,
  col: usize,
}

impl DisplayCursor {
  fn from(cursor_point: AlacPoint, display_offset: usize) -> Self {
    Self {
      line: cursor_point.line.0 + display_offset as i32,
      col: cursor_point.column.0,
    }
  }

  pub fn line(&self) -> i32 {
    self.line
  }

  pub fn col(&self) -> usize {
    self.col
  }
}

fn to_highlighted_range_lines(
  range: &RangeInclusive<AlacPoint>,
  layout: &LayoutState,
  origin: Point<Pixels>,
) -> Option<(Pixels, Vec<HighlightedRangeLine>)> {
  // Step 1. Normalize the points to be viewport relative.
  // When display_offset = 1, here's how the grid is arranged:
  //-2,0 -2,1...
  //--- Viewport top
  //-1,0 -1,1...
  //--------- Terminal Top
  // 0,0  0,1...
  // 1,0  1,1...
  //--- Viewport Bottom
  // 2,0  2,1...
  //--------- Terminal Bottom

  // Normalize to viewport relative, from terminal relative.
  // lines are i32s, which are negative above the top left corner of the terminal
  // If the user has scrolled, we use the display_offset to tell us which offset
  // of the grid data we should be looking at. But for the rendering step, we don't
  // want negatives. We want things relative to the 'viewport' (the area of the grid
  // which is currently shown according to the display offset)
  let unclamped_start = AlacPoint::new(
    range.start().line + layout.display_offset,
    range.start().column,
  );
  let unclamped_end = AlacPoint::new(range.end().line + layout.display_offset, range.end().column);

  // Step 2. Clamp range to viewport, and return None if it doesn't overlap
  if unclamped_end.line.0 < 0 || unclamped_start.line.0 > layout.dimensions.num_lines() as i32 {
    return None;
  }

  let clamped_start_line = unclamped_start.line.0.max(0) as usize;
  let clamped_end_line = unclamped_end
    .line
    .0
    .min(layout.dimensions.num_lines() as i32) as usize;
  //Convert the start of the range to pixels
  let start_y = origin.y + clamped_start_line as f32 * layout.dimensions.line_height;

  // Step 3. Expand ranges that cross lines into a collection of single-line ranges.
  //  (also convert to pixels)
  let mut highlighted_range_lines = Vec::new();
  for line in clamped_start_line..=clamped_end_line {
    let mut line_start = 0;
    let mut line_end = layout.dimensions.columns();

    if line == clamped_start_line {
      line_start = unclamped_start.column.0;
    }
    if line == clamped_end_line {
      line_end = unclamped_end.column.0 + 1; // +1 for inclusive
    }

    highlighted_range_lines.push(HighlightedRangeLine {
      start_x: origin.x + line_start as f32 * layout.dimensions.cell_width,
      end_x: origin.x + line_end as f32 * layout.dimensions.cell_width,
    });
  }

  Some((start_y, highlighted_range_lines))
}

fn is_decorative_character(ch: char) -> bool {
  matches!(
      ch as u32,
      // Unicode Box Drawing and Block Elements
      0x2500..=0x257F // Box Drawing (└ ┐ ─ │ etc.)
      | 0x2580..=0x259F // Block Elements (▀ ▄ █ ░ ▒ ▓ etc.)
      | 0x25A0..=0x25FF // Geometric Shapes (■ ▶ ● etc. - includes triangular/circular separators)

      // Private Use Area - Powerline separator symbols only
      | 0xE0B0..=0xE0B7 // Powerline separators: triangles (E0B0-E0B3) and half circles (E0B4-E0B7)
      | 0xE0B8..=0xE0BF // Powerline separators: corner triangles
      | 0xE0C0..=0xE0CA // Powerline separators: flames (E0C0-E0C3), pixelated (E0C4-E0C7), and ice (E0C8 & E0CA)
      | 0xE0CC..=0xE0D1 // Powerline separators: honeycombs (E0CC-E0CD) and lego (E0CE-E0D1)
      | 0xE0D2..=0xE0D7 // Powerline separators: trapezoid (E0D2 & E0D4) and inverted triangles (E0D6-E0D7)
  )
}
