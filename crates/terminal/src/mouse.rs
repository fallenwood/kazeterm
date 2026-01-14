use std::cmp::{self, min};

use alacritty_terminal::{
  grid::Dimensions as _,
  index::{Column as GridCol, Line as GridLine, Point as AlacPoint, Side},
};
use gpui::{Pixels, Point, px};

use crate::TerminalBounds;

pub fn grid_point_and_side(
  pos: Point<Pixels>,
  cur_size: TerminalBounds,
  display_offset: usize,
) -> (AlacPoint, Side) {
  let mut col = GridCol((pos.x / cur_size.cell_width) as usize);
  let cell_x = cmp::max(px(0.), pos.x) % cur_size.cell_width;
  let half_cell_width = cur_size.cell_width / 2.0;
  let mut side = if cell_x > half_cell_width {
    Side::Right
  } else {
    Side::Left
  };

  if col > cur_size.last_column() {
    col = cur_size.last_column();
    side = Side::Right;
  }
  let col = min(col, cur_size.last_column());
  let mut line = (pos.y / cur_size.line_height) as i32;
  if line > cur_size.bottommost_line() {
    line = cur_size.bottommost_line().0;
    side = Side::Right;
  } else if line < 0 {
    side = Side::Left;
  }

  (
    AlacPoint::new(GridLine(line - display_offset as i32), col),
    side,
  )
}
