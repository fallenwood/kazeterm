use std::ops::Deref;

use terminal_kernel::{index::Point as AlacPoint, term::cell::Cell};

#[derive(Debug, Clone)]
pub struct IndexedCell {
  pub point: AlacPoint,
  pub cell: Cell,
}

impl Deref for IndexedCell {
  type Target = Cell;

  #[inline]
  fn deref(&self) -> &Cell {
    &self.cell
  }
}
