use std::ops::RangeInclusive;

use alacritty_terminal::{
  index::Point as AlacPoint,
  vte::ansi::{Color, NamedColor},
};
use gpui::{
  App, Font, FontStyle, FontWeight, HighlightStyle, Pixels, StrikethroughStyle, TextRun, TextStyle,
  UnderlineStyle,
};
use itertools::Itertools;
use themeing::{ActiveTheme as _, convert_color};

use crate::{background_region::BackgroundRegion, indexed_cell::IndexedCell};

use super::BatchedTextRun;
use super::LayoutRect;
use super::TerminalElement;
use super::helpers::{is_blank, is_decorative_character, merge_background_regions};

impl TerminalElement {
  pub fn layout_grid(
    grid: impl Iterator<Item = IndexedCell>,
    start_line_offset: i32,
    text_style: &TextStyle,
    hyperlink: Option<(HighlightStyle, &RangeInclusive<AlacPoint>)>,
    minimum_contrast: f32,
    bold_as_bright: bool,
    cx: &App,
  ) -> (Vec<LayoutRect>, Vec<BatchedTextRun>) {
    let theme = cx.theme();

    let estimated_cells = grid.size_hint().0;
    let estimated_runs = estimated_cells / 10;
    let estimated_regions = estimated_cells / 20;

    let mut batched_runs = Vec::with_capacity(estimated_runs);
    let mut background_regions: Vec<BackgroundRegion> = Vec::with_capacity(estimated_regions);
    let mut current_batch: Option<BatchedTextRun> = None;

    let linegroups = grid.into_iter().chunk_by(|i| i.point.line);
    for (line_index, (_, line)) in linegroups.into_iter().enumerate() {
      let alac_line = start_line_offset + line_index as i32;

      if let Some(batch) = current_batch.take() {
        batched_runs.push(batch);
      }

      let mut previous_cell_had_extras = false;

      for cell in line {
        let mut fg = cell.fg;
        let mut bg = cell.bg;
        if cell
          .flags
          .contains(alacritty_terminal::term::cell::Flags::INVERSE)
        {
          std::mem::swap(&mut fg, &mut bg);
        }

        // Bold-as-bright: promote standard named colors to their bright variant
        if bold_as_bright
          && cell
            .flags
            .intersects(alacritty_terminal::term::cell::Flags::BOLD)
        {
          fg = to_bright_named(fg);
        }

        if !matches!(bg, Color::Named(NamedColor::Background)) {
          let color = convert_color(&bg, theme);
          let col = cell.point.column.0 as i32;

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

        if cell
          .flags
          .contains(alacritty_terminal::term::cell::Flags::WIDE_CHAR_SPACER)
        {
          continue;
        }

        if cell.c == ' ' && previous_cell_had_extras {
          previous_cell_had_extras = false;
          continue;
        }
        previous_cell_had_extras = cell.extra.is_some();

        {
          if !is_blank(&cell) {
            let cell_style = Self::cell_style(
              &cell,
              fg,
              bg,
              theme,
              text_style,
              hyperlink,
              minimum_contrast,
            );

            let cell_point = AlacPoint::new(alac_line, cell.point.column.0 as i32);

            if let Some(ref mut batch) = current_batch {
              if batch.can_append(&cell_style)
                && batch.start_point.line == cell_point.line
                && batch.start_point.column + batch.cell_count as i32 == cell_point.column
              {
                batch.append_char(cell.c);
              } else {
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

    if let Some(batch) = current_batch {
      batched_runs.push(batch);
    }

    let merged_regions = merge_background_regions(background_regions);
    let mut rects = Vec::with_capacity(merged_regions.len() * 2);

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

    if !is_decorative_character(indexed.c) {
      fg = crate::apca_contrast::ensure_minimum_contrast(fg, bg, minimum_contrast);
    }

    if flags.intersects(alacritty_terminal::term::cell::Flags::DIM) {
      fg.a *= 0.7;
    }

    let underline = (flags.intersects(alacritty_terminal::term::cell::Flags::ALL_UNDERLINES)
      || indexed.cell.hyperlink().is_some())
    .then(|| UnderlineStyle {
      color: Some(fg),
      thickness: Pixels::from(1.0),
      wavy: flags.contains(alacritty_terminal::term::cell::Flags::UNDERCURL),
    });

    let strikethrough = flags
      .intersects(alacritty_terminal::term::cell::Flags::STRIKEOUT)
      .then(|| StrikethroughStyle {
        color: Some(fg),
        thickness: Pixels::from(1.0),
      });

    let weight = if flags.intersects(alacritty_terminal::term::cell::Flags::BOLD) {
      FontWeight::BOLD
    } else {
      text_style.font_weight
    };

    let style = if flags.intersects(alacritty_terminal::term::cell::Flags::ITALIC) {
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
}

/// Promote a standard named ANSI color (0–7) to its bright variant (8–15).
/// Non-standard named colors, indexed colors, and true colors are returned unchanged.
fn to_bright_named(color: Color) -> Color {
  match color {
    Color::Named(n) => Color::Named(match n {
      NamedColor::Black => NamedColor::BrightBlack,
      NamedColor::Red => NamedColor::BrightRed,
      NamedColor::Green => NamedColor::BrightGreen,
      NamedColor::Yellow => NamedColor::BrightYellow,
      NamedColor::Blue => NamedColor::BrightBlue,
      NamedColor::Magenta => NamedColor::BrightMagenta,
      NamedColor::Cyan => NamedColor::BrightCyan,
      NamedColor::White => NamedColor::BrightWhite,
      other => other,
    }),
    other => other,
  }
}
