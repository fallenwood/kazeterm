use std::ops::RangeInclusive;

use alacritty_terminal::index::Point;

#[derive(Debug)]
pub struct HoverTarget {
  pub tooltip: String,
  pub hovered_word: HoveredWord,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct HoveredWord {
  pub word: String,
  pub word_match: RangeInclusive<Point>,
  pub id: usize,
}
