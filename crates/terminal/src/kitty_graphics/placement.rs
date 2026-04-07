use super::command::{ImagePlacement, VisiblePlacement};
use super::storage::KittyImageStorage;

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
    self
      .placements
      .retain(|p| !(p.line == line && p.column == column));
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

    self
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
          render_image: stored.render_image.clone(),
          viewport_line: p.line - viewport_top,
          column: p.column,
          width_cells: p.width_cells,
          height_cells: p.height_cells,
          z_index: p.z_index,
          x_offset: p.x_offset,
          y_offset: p.y_offset,
        })
      })
      .collect()
  }

  pub fn placement_count(&self) -> usize {
    self.placements.len()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn make_placement(image_id: u32, line: i32, col: i32) -> ImagePlacement {
    ImagePlacement {
      image_id,
      placement_id: 0,
      line,
      column: col,
      width_cells: 10,
      height_cells: 5,
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
  fn test_clear() {
    let mut mgr = PlacementManager::new();
    mgr.add(make_placement(1, 0, 0));
    mgr.add(make_placement(2, 0, 0));
    mgr.clear();
    assert_eq!(mgr.placement_count(), 0);
  }
}
