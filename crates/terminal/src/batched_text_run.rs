use gpui::{AbsoluteLength, TextRun, Window};

use super::terminal_bounds::TerminalBounds;
use alacritty_terminal::index::Point as AlacPoint;

#[derive(Debug)]
pub struct BatchedTextRun {
  pub start_point: AlacPoint<i32, i32>,
  pub text: String,
  pub cell_count: usize,
  pub style: TextRun,
  pub font_size: AbsoluteLength,
}

impl BatchedTextRun {
  pub(crate) fn new_from_char(
    start_point: AlacPoint<i32, i32>,
    c: char,
    style: TextRun,
    font_size: AbsoluteLength,
  ) -> Self {
    let mut text = String::with_capacity(100); // Pre-allocate for typical line length
    text.push(c);
    BatchedTextRun {
      start_point,
      text,
      cell_count: 1,
      style,
      font_size,
    }
  }

  pub(crate) fn can_append(&self, other_style: &gpui::TextRun) -> bool {
    self.style.font == other_style.font
      && self.style.color == other_style.color
      && self.style.background_color == other_style.background_color
      && self.style.underline == other_style.underline
      && self.style.strikethrough == other_style.strikethrough
  }

  pub(crate) fn append_char(&mut self, c: char) {
    self.text.push(c);
    self.cell_count += 1;
    self.style.len += c.len_utf8();
  }

  pub fn paint(
    &self,
    origin: gpui::Point<gpui::Pixels>,
    dimensions: &TerminalBounds,
    window: &mut Window,
    cx: &mut gpui::App,
  ) {
    let pos = gpui::Point::new(
      origin.x + self.start_point.column as f32 * dimensions.cell_width,
      origin.y + self.start_point.line as f32 * dimensions.line_height,
    );

    let _ = window
      .text_system()
      .shape_line(
        self.text.clone().into(),
        self.font_size.to_pixels(window.rem_size()),
        std::slice::from_ref(&self.style),
        Some(dimensions.cell_width),
      )
      .paint(pos, dimensions.line_height, window, cx);
  }
}
