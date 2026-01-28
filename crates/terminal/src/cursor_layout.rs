use alacritty_terminal::vte::ansi::CursorShape;
use gpui::{
  AnyElement, App, BorderStyle, Bounds, Hsla, Pixels, ShapedLine, Window, fill, outline, px, size,
};

pub struct CursorLayout {
  origin: gpui::Point<Pixels>,
  block_width: Pixels,
  line_height: Pixels,
  color: Hsla,
  shape: CursorShape,
  block_text: Option<ShapedLine>,
  cursor_name: Option<AnyElement>,
}

impl CursorLayout {
  pub fn new(
    origin: gpui::Point<Pixels>,
    block_width: Pixels,
    line_height: Pixels,
    color: Hsla,
    shape: CursorShape,
    block_text: Option<ShapedLine>,
  ) -> CursorLayout {
    CursorLayout {
      origin,
      block_width,
      line_height,
      color,
      shape,
      block_text,
      cursor_name: None,
    }
  }

  pub fn bounding_rect(&self, origin: gpui::Point<Pixels>) -> Bounds<Pixels> {
    Bounds {
      origin: self.origin + origin,
      size: size(self.block_width, self.line_height),
    }
  }

  fn bounds(&self, origin: gpui::Point<Pixels>) -> Bounds<Pixels> {
    match self.shape {
      CursorShape::Beam => Bounds {
        origin: self.origin + origin,
        size: size(px(2.0), self.line_height),
      },
      CursorShape::Block | CursorShape::HollowBlock => Bounds {
        origin: self.origin + origin,
        size: size(self.block_width, self.line_height),
      },
      CursorShape::Underline => Bounds {
        origin: self.origin + origin + gpui::Point::new(Pixels::ZERO, self.line_height - px(2.0)),
        size: size(self.block_width, px(2.0)),
      },
      CursorShape::Hidden => unreachable!(),
    }
  }

  pub fn paint(&mut self, origin: gpui::Point<Pixels>, window: &mut Window, cx: &mut App) {
    let bounds = self.bounds(origin);

    // Draw background or border quad
    let cursor = if matches!(self.shape, CursorShape::HollowBlock) {
      outline(bounds, self.color, BorderStyle::Solid)
    } else {
      fill(bounds, self.color)
    };

    if let Some(name) = &mut self.cursor_name {
      name.paint(window, cx);
    }

    window.paint_quad(cursor);

    if let Some(block_text) = &self.block_text {
      block_text
        .paint(self.origin + origin, self.line_height, window, cx)
        .unwrap();
    }
  }
}
