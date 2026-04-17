use super::terminal_bounds::TerminalBounds;
use gpui::{Bounds, Hsla, Pixels, Point, Window, fill, point};
use terminal_kernel::index::Point as AlacPoint;

#[derive(Clone, Debug, Default)]
pub struct LayoutRect {
  point: AlacPoint<i32, i32>,
  num_of_cells: usize,
  pub(crate) color: Hsla,
}

impl LayoutRect {
  pub fn new(point: AlacPoint<i32, i32>, num_of_cells: usize, color: Hsla) -> LayoutRect {
    LayoutRect {
      point,
      num_of_cells,
      color,
    }
  }

  pub fn paint(&self, origin: Point<Pixels>, dimensions: &TerminalBounds, window: &mut Window) {
    let position = {
      let alac_point = self.point;
      point(
        (origin.x + alac_point.column as f32 * dimensions.cell_width).floor(),
        origin.y + alac_point.line as f32 * dimensions.line_height,
      )
    };
    let size = point(
      (dimensions.cell_width * self.num_of_cells as f32).ceil(),
      dimensions.line_height,
    )
    .into();

    window.paint_quad(fill(Bounds::new(position, size), self.color));
  }
}
