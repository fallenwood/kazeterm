use std::ops::Range;

pub struct ImeState {
  pub marked_text: String,
  pub marked_range_utf16: Option<Range<usize>>,
}
