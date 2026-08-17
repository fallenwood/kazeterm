use super::command::{ImagePlacement, VisiblePlacement};
use super::scroll_tracker::GraphicsScrollDirection;
use super::storage::KittyImageStorage;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PlacementGeometry {
  pub crop: (u32, u32, u32, u32),
  pub width_pixels: u32,
  pub height_pixels: u32,
  pub width_cells: u32,
  pub height_cells: u32,
}

pub(crate) fn resolve_geometry(
  image_width: u32,
  image_height: u32,
  crop: (u32, u32, u32, u32),
  display_columns: u32,
  display_rows: u32,
  x_offset: u32,
  y_offset: u32,
  cell_width: u32,
  cell_height: u32,
) -> Option<PlacementGeometry> {
  let (crop_x, crop_y, requested_width, requested_height) = crop;
  if crop_x >= image_width || crop_y >= image_height {
    return None;
  }
  let crop_width = if requested_width == 0 {
    image_width - crop_x
  } else {
    requested_width.min(image_width - crop_x)
  };
  let crop_height = if requested_height == 0 {
    image_height - crop_y
  } else {
    requested_height.min(image_height - crop_y)
  };
  if crop_width == 0
    || crop_height == 0
    || x_offset >= cell_width.max(1)
    || y_offset >= cell_height.max(1)
  {
    return None;
  }

  let cell_width = cell_width.max(1);
  let cell_height = cell_height.max(1);
  let requested_width_pixels = display_columns.checked_mul(cell_width);
  let requested_height_pixels = display_rows.checked_mul(cell_height);
  let (width_pixels, height_pixels) = match (display_columns, display_rows) {
    (0, 0) => (crop_width, crop_height),
    (_, 0) => {
      let width = requested_width_pixels?;
      let height = scale_dimension(crop_height, width, crop_width)?;
      (width, height)
    }
    (0, _) => {
      let height = requested_height_pixels?;
      let width = scale_dimension(crop_width, height, crop_height)?;
      (width, height)
    }
    (_, _) => (requested_width_pixels?, requested_height_pixels?),
  };
  let width_cells = div_ceil(width_pixels, cell_width);
  let height_cells = div_ceil(height_pixels, cell_height);

  Some(PlacementGeometry {
    crop: (crop_x, crop_y, crop_width, crop_height),
    width_pixels,
    height_pixels,
    width_cells,
    height_cells,
  })
}

fn scale_dimension(source: u32, target: u32, denominator: u32) -> Option<u32> {
  let scaled = (source as u64)
    .checked_mul(target as u64)?
    .checked_add(denominator as u64 - 1)?
    / denominator as u64;
  u32::try_from(scaled).ok().map(|value| value.max(1))
}

fn div_ceil(value: u32, divisor: u32) -> u32 {
  value / divisor + u32::from(!value.is_multiple_of(divisor))
}

/// Manages image placements in the terminal grid.
pub struct PlacementManager {
  placements: Vec<ImagePlacement>,
}

impl PlacementManager {
  pub fn new() -> Self {
    Self {
      placements: Vec::new(),
    }
  }

  /// Add a new image placement.
  pub fn add(&mut self, placement: ImagePlacement) {
    // Remove existing placement with same image_id + placement_id combo.
    if placement.placement_id != 0 {
      self.placements.retain(|p| {
        !(p.image_id == placement.image_id && p.placement_id == placement.placement_id)
      });
    }
    self.placements.push(placement);
  }

  /// Remove all placements for a given image ID.
  pub fn remove_by_image(&mut self, image_id: u32) {
    self.placements.retain(|p| p.image_id != image_id);
  }

  /// Remove a specific placement.
  pub fn remove_by_id(&mut self, image_id: u32, placement_id: Option<u32>) {
    self.placements.retain(|p| {
      if p.image_id != image_id {
        return true;
      }
      if let Some(pid) = placement_id {
        p.placement_id != pid
      } else {
        false
      }
    });
  }

  /// Remove all placements at a given grid position.
  pub fn remove_at_cursor(&mut self, line: i32, column: i32) {
    self.remove_at_cell(line, column);
  }

  pub fn remove_at_cell(&mut self, line: i32, column: i32) {
    self.placements.retain(|placement| {
      !intersects_cell(placement, line, column)
    });
  }

  pub fn remove_at_cell_and_z_index(&mut self, line: i32, column: i32, z_index: i32) {
    self.placements.retain(|placement| {
      !(placement.z_index == z_index && intersects_cell(placement, line, column))
    });
  }

  pub fn remove_at_column(&mut self, column: i32) {
    self.placements.retain(|placement| {
      let right = placement.column.saturating_add(placement.width_cells as i32);
      column < placement.column || column >= right
    });
  }

  pub fn remove_at_row(&mut self, line: i32) {
    self.placements.retain(|placement| {
      let bottom = placement.line.saturating_add(placement.height_cells as i32);
      line < placement.line || line >= bottom
    });
  }

  pub fn remove_by_z_index(&mut self, z_index: i32) {
    self.placements.retain(|placement| placement.z_index != z_index);
  }

  pub fn remove_by_image_range(&mut self, first: u32, last: u32) {
    self
      .placements
      .retain(|placement| placement.image_id < first || placement.image_id > last);
  }

  pub fn remove_visible(&mut self, viewport_top: i32, viewport_lines: u32) {
    let viewport_bottom = viewport_top.saturating_add(viewport_lines as i32);
    self.placements.retain(|placement| {
      let bottom = placement.line.saturating_add(placement.height_cells as i32);
      bottom <= viewport_top || placement.line >= viewport_bottom
    });
  }

  pub fn image_ids(&self) -> Vec<u32> {
    let mut ids = self
      .placements
      .iter()
      .map(|placement| placement.image_id)
      .collect::<Vec<_>>();
    ids.sort_unstable();
    ids.dedup();
    ids
  }

  pub fn has_image(&self, image_id: u32) -> bool {
    self
      .placements
      .iter()
      .any(|placement| placement.image_id == image_id)
  }

  pub fn retain_lines_at_or_after(&mut self, first_line: i32) {
    self.placements.retain(|placement| {
      placement.line.saturating_add(placement.height_cells as i32) > first_line
    });
  }

  pub fn scroll_region(
    &mut self,
    direction: GraphicsScrollDirection,
    top: i32,
    bottom: i32,
    lines: u32,
  ) {
    let lines = (lines as i32).min(bottom.saturating_sub(top));
    if lines <= 0 {
      return;
    }

    self.placements.retain_mut(|placement| {
      if placement.line < top || placement.line >= bottom {
        return true;
      }

      match direction {
        GraphicsScrollDirection::Up => {
          if placement.line < top.saturating_add(lines) {
            return false;
          }
          placement.line = placement.line.saturating_sub(lines);
        }
        GraphicsScrollDirection::Down => {
          if placement.line >= bottom.saturating_sub(lines) {
            return false;
          }
          placement.line = placement.line.saturating_add(lines);
        }
      }
      true
    });
  }

  /// Remove all placements.
  pub fn clear(&mut self) {
    self.placements.clear();
  }

  /// Remove placements for images that no longer exist in storage.
  pub fn gc(&mut self, storage: &KittyImageStorage) {
    self
      .placements
      .retain(|p| storage.peek(p.image_id).is_some());
  }

  /// Get all placements visible in the current viewport.
  ///
  /// `viewport_top` is the absolute line number of the top visible row.
  /// `viewport_lines` is the number of visible rows.
  pub fn visible_placements(
    &self,
    storage: &KittyImageStorage,
    viewport_top: i32,
    viewport_lines: u32,
  ) -> Vec<VisiblePlacement> {
    let viewport_bottom = viewport_top + viewport_lines as i32;

    let mut visible = self
      .placements
      .iter()
      .filter_map(|p| {
        // Check if any part of the image overlaps the viewport.
        let img_bottom = p.line + p.height_cells as i32;
        if img_bottom <= viewport_top || p.line >= viewport_bottom {
          return None;
        }

        let stored = storage.peek(p.image_id)?;

        Some(VisiblePlacement {
          image_id: p.image_id,
          render_image: stored.render_image.clone(),
          source_width: p.source_width,
          source_height: p.source_height,
          viewport_line: p.line - viewport_top,
          column: p.column,
          width_cells: p.width_cells,
          height_cells: p.height_cells,
          width_pixels: p.width_pixels,
          height_pixels: p.height_pixels,
          crop: p.crop,
          z_index: p.z_index,
          x_offset: p.x_offset,
          y_offset: p.y_offset,
        })
      })
      .collect::<Vec<_>>();
    visible.sort_by_key(|placement| (placement.z_index, placement.image_id));
    visible
  }

  pub fn placement_count(&self) -> usize {
    self.placements.len()
  }
}

fn intersects_cell(placement: &ImagePlacement, line: i32, column: i32) -> bool {
  let right = placement.column.saturating_add(placement.width_cells as i32);
  let bottom = placement.line.saturating_add(placement.height_cells as i32);
  column >= placement.column && column < right && line >= placement.line && line < bottom
}

#[cfg(test)]
mod tests {
  use super::*;

  fn make_placement(image_id: u32, line: i32, col: i32) -> ImagePlacement {
    ImagePlacement {
      image_id,
      placement_id: 0,
      source_width: 100,
      source_height: 50,
      line,
      column: col,
      width_cells: 10,
      height_cells: 5,
      width_pixels: 100,
      height_pixels: 50,
      crop: (0, 0, 0, 0),
      z_index: 0,
      x_offset: 0,
      y_offset: 0,
    }
  }

  #[test]
  fn test_add_and_count() {
    let mut mgr = PlacementManager::new();
    mgr.add(make_placement(1, 0, 0));
    mgr.add(make_placement(2, 10, 5));
    assert_eq!(mgr.placement_count(), 2);
  }

  #[test]
  fn test_remove_by_image() {
    let mut mgr = PlacementManager::new();
    mgr.add(make_placement(1, 0, 0));
    mgr.add(make_placement(1, 10, 0));
    mgr.add(make_placement(2, 20, 0));
    mgr.remove_by_image(1);
    assert_eq!(mgr.placement_count(), 1);
  }

  #[test]
  fn test_remove_at_cursor() {
    let mut mgr = PlacementManager::new();
    mgr.add(make_placement(1, 5, 3));
    mgr.add(make_placement(2, 5, 3));
    mgr.add(make_placement(3, 10, 0));
    mgr.remove_at_cursor(5, 3);
    assert_eq!(mgr.placement_count(), 1);
  }

  #[test]
  fn removal_uses_full_placement_rectangle() {
    let mut mgr = PlacementManager::new();
    mgr.add(make_placement(1, 5, 3));
    mgr.add(make_placement(2, 20, 20));

    mgr.remove_at_cell(7, 8);

    assert_eq!(mgr.image_ids(), [2]);
  }

  #[test]
  fn removes_only_matching_z_index_at_cell() {
    let mut first = make_placement(1, 5, 3);
    first.z_index = -1;
    let mut second = make_placement(2, 5, 3);
    second.z_index = 2;
    let mut mgr = PlacementManager::new();
    mgr.add(first);
    mgr.add(second);

    mgr.remove_at_cell_and_z_index(6, 4, -1);

    assert_eq!(mgr.image_ids(), [2]);
  }

  #[test]
  fn native_size_preserves_pixels_and_rounds_occupied_cells() {
    let geometry = resolve_geometry(21, 17, (0, 0, 0, 0), 0, 0, 3, 2, 10, 8).unwrap();

    assert_eq!(geometry.width_pixels, 21);
    assert_eq!(geometry.height_pixels, 17);
    assert_eq!((geometry.width_cells, geometry.height_cells), (3, 3));
  }

  #[test]
  fn one_sided_size_preserves_crop_aspect_ratio() {
    let geometry = resolve_geometry(100, 80, (10, 20, 40, 20), 4, 0, 0, 0, 10, 10).unwrap();

    assert_eq!(geometry.crop, (10, 20, 40, 20));
    assert_eq!((geometry.width_pixels, geometry.height_pixels), (40, 20));
    assert_eq!((geometry.width_cells, geometry.height_cells), (4, 2));
  }

  #[test]
  fn crop_is_intersected_with_source_image() {
    let geometry = resolve_geometry(10, 10, (8, 0, 3, 10), 0, 0, 0, 0, 8, 16).unwrap();

    assert_eq!(geometry.crop, (8, 0, 2, 10));
  }

  #[test]
  fn pixel_offsets_do_not_expand_placement_rectangle() {
    let geometry = resolve_geometry(16, 16, (0, 0, 0, 0), 2, 1, 7, 15, 8, 16).unwrap();

    assert_eq!((geometry.width_cells, geometry.height_cells), (2, 1));
  }

  #[test]
  fn rejects_pixel_offset_outside_first_cell() {
    assert!(resolve_geometry(10, 10, (0, 0, 0, 0), 0, 0, 8, 0, 8, 16).is_none());
  }

  #[test]
  fn test_clear() {
    let mut mgr = PlacementManager::new();
    mgr.add(make_placement(1, 0, 0));
    mgr.add(make_placement(2, 0, 0));
    mgr.clear();
    assert_eq!(mgr.placement_count(), 0);
  }

  #[test]
  fn scrolling_up_moves_anchors_and_drops_scrolled_out_rows() {
    let mut mgr = PlacementManager::new();
    mgr.add(make_placement(1, 2, 0));
    mgr.add(make_placement(2, 4, 0));
    mgr.add(make_placement(3, 8, 0));

    mgr.scroll_region(GraphicsScrollDirection::Up, 2, 8, 2);

    assert_eq!(mgr.image_ids(), [2, 3]);
    assert_eq!(mgr.placements[0].line, 2);
    assert_eq!(mgr.placements[1].line, 8);
  }

  #[test]
  fn scrolling_down_moves_anchors_and_drops_bottom_rows() {
    let mut mgr = PlacementManager::new();
    mgr.add(make_placement(1, 1, 0));
    mgr.add(make_placement(2, 3, 0));
    mgr.add(make_placement(3, 6, 0));

    mgr.scroll_region(GraphicsScrollDirection::Down, 2, 7, 2);

    assert_eq!(mgr.image_ids(), [1, 2]);
    assert_eq!(mgr.placements[0].line, 1);
    assert_eq!(mgr.placements[1].line, 5);
  }
}
