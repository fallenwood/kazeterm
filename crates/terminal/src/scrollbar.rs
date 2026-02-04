use gpui::{BorderStyle, Bounds, Hsla, Pixels, Point, Window, fill, px};

/// Scrollbar width in pixels
pub const SCROLLBAR_WIDTH: f32 = 12.0;

/// Minimum thumb height in pixels
const MIN_THUMB_HEIGHT: f32 = 20.0;

/// Scrollbar state for rendering
#[derive(Clone, Debug)]
pub struct ScrollbarState {
  /// Total number of lines in the terminal (visible + history)
  pub total_lines: usize,
  /// Number of visible lines
  pub visible_lines: usize,
  /// Current scroll offset (0 = bottom, history_size = top)
  pub display_offset: usize,
  /// History size (max scroll offset)
  pub history_size: usize,
}

impl ScrollbarState {
  pub fn new(
    total_lines: usize,
    visible_lines: usize,
    display_offset: usize,
    history_size: usize,
  ) -> Self {
    Self {
      total_lines,
      visible_lines,
      display_offset,
      history_size,
    }
  }

  /// Returns true if the scrollbar should be shown (there's content to scroll)
  pub fn should_show(&self) -> bool {
    self.history_size > 0
  }

  /// Calculate the thumb position and size as ratios (0.0 to 1.0)
  pub fn thumb_metrics(&self) -> (f32, f32) {
    if self.total_lines == 0 || self.history_size == 0 {
      return (0.0, 1.0);
    }

    let total = self.total_lines as f32;
    let visible = self.visible_lines as f32;

    // Thumb size is the ratio of visible lines to total lines
    let thumb_size = (visible / total).min(1.0);

    // Thumb position: display_offset is 0 at bottom, history_size at top
    // We need to invert it for the scrollbar (0 at top, 1 at bottom)
    let scroll_ratio = if self.history_size > 0 {
      1.0 - (self.display_offset as f32 / self.history_size as f32)
    } else {
      1.0
    };

    // Position should be adjusted so the thumb stays within bounds
    let thumb_top = scroll_ratio * (1.0 - thumb_size);

    (thumb_top, thumb_size)
  }

  /// Convert a Y position (0.0 to 1.0) to a scroll offset
  pub fn position_to_offset(&self, position_ratio: f32) -> usize {
    if self.history_size == 0 {
      return 0;
    }

    // Invert the position (top = history_size, bottom = 0)
    let inverted = 1.0 - position_ratio;
    (inverted * self.history_size as f32).round() as usize
  }

  /// Check if a position ratio (0.0 to 1.0) is within the thumb area
  /// Returns true if the click is on the thumb, false if on the track
  pub fn is_on_thumb(&self, position_ratio: f32, track_height: Pixels) -> bool {
    let (thumb_top_ratio, thumb_size_ratio) = self.thumb_metrics();

    // Calculate actual thumb bounds considering minimum height
    let thumb_height = (track_height * thumb_size_ratio).max(px(MIN_THUMB_HEIGHT));
    let thumb_top = track_height * thumb_top_ratio;
    // Adjust thumb position if it would overflow
    let thumb_top = thumb_top.min(track_height - thumb_height);

    let click_y = track_height * position_ratio;
    click_y >= thumb_top && click_y <= thumb_top + thumb_height
  }
}

/// Paint a scrollbar into the given bounds
pub fn paint_scrollbar(
  bounds: Bounds<Pixels>,
  state: &ScrollbarState,
  track_color: Hsla,
  thumb_color: Hsla,
  hovered: bool,
  window: &mut Window,
) {
  // Paint track background
  let track_fill = if hovered {
    track_color.opacity(0.3)
  } else {
    track_color.opacity(0.1)
  };
  window.paint_quad(fill(bounds, track_fill));

  // Calculate thumb dimensions
  let (thumb_top_ratio, thumb_size_ratio) = state.thumb_metrics();
  let track_height = bounds.size.height;
  let thumb_height = (track_height * thumb_size_ratio).max(px(MIN_THUMB_HEIGHT));
  let thumb_top = bounds.origin.y + track_height * thumb_top_ratio;

  // Adjust thumb position if it would overflow
  let thumb_top = thumb_top.min(bounds.origin.y + track_height - thumb_height);

  let thumb_bounds = Bounds {
    origin: Point {
      x: bounds.origin.x + px(2.0),
      y: thumb_top,
    },
    size: gpui::Size {
      width: bounds.size.width - px(4.0),
      height: thumb_height,
    },
  };

  // Paint thumb with rounded corners
  let thumb_fill = if hovered {
    thumb_color.opacity(0.7)
  } else {
    thumb_color.opacity(0.4)
  };

  window.paint_quad(gpui::quad(
    thumb_bounds,
    px(3.0), // corner radius
    thumb_fill,
    gpui::Edges::default(),
    Hsla::transparent_black(),
    BorderStyle::default(),
  ));
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_scrollbar_metrics() {
    // At bottom of scroll (display_offset = 0)
    let state = ScrollbarState::new(100, 20, 0, 80);
    let (top, size) = state.thumb_metrics();
    assert!((size - 0.2).abs() < 0.01); // 20/100 = 0.2
    assert!((top - 0.8).abs() < 0.01); // thumb at bottom

    // At top of scroll (display_offset = history_size)
    let state = ScrollbarState::new(100, 20, 80, 80);
    let (top, _size) = state.thumb_metrics();
    assert!(top < 0.01); // thumb at top

    // Middle of scroll
    let state = ScrollbarState::new(100, 20, 40, 80);
    let (top, _size) = state.thumb_metrics();
    assert!((top - 0.4).abs() < 0.01); // thumb in middle
  }

  #[test]
  fn test_position_to_offset() {
    let state = ScrollbarState::new(100, 20, 0, 80);

    // Click at top should scroll to top (max offset)
    assert_eq!(state.position_to_offset(0.0), 80);

    // Click at bottom should scroll to bottom (offset 0)
    assert_eq!(state.position_to_offset(1.0), 0);

    // Click in middle
    assert_eq!(state.position_to_offset(0.5), 40);
  }

  #[test]
  fn test_is_on_thumb() {
    // Scrollbar at bottom (display_offset = 0), thumb is at the bottom
    let state = ScrollbarState::new(100, 20, 0, 80);
    let track_height = px(500.0);

    // Thumb is at 80% down (top = 0.8, size = 0.2)
    // Click at 85% should be on thumb
    assert!(state.is_on_thumb(0.85, track_height));
    // Click at 95% should be on thumb
    assert!(state.is_on_thumb(0.95, track_height));
    // Click at 50% should NOT be on thumb (above it)
    assert!(!state.is_on_thumb(0.5, track_height));
    // Click at 10% should NOT be on thumb
    assert!(!state.is_on_thumb(0.1, track_height));

    // Scrollbar at top (display_offset = history_size), thumb is at the top
    let state = ScrollbarState::new(100, 20, 80, 80);
    // Thumb is at top (top = 0, size = 0.2)
    // Click at 10% should be on thumb
    assert!(state.is_on_thumb(0.1, track_height));
    // Click at 50% should NOT be on thumb
    assert!(!state.is_on_thumb(0.5, track_height));
  }
}
