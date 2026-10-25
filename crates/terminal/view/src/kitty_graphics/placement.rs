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
    // Replacement is scoped to internal storage identity, never to the shared
    // client ID zero of anonymous images.
    if placement.placement_id != 0 {
      self.placements.retain(|p| {
        !(p.internal_image_id == placement.internal_image_id
          && p.placement_id == placement.placement_id)
      });
    }
    self.placements.push(placement);
  }

  /// Remove all placements for a given image ID.
  pub fn remove_by_image(&mut self, image_id: u32) {
    self
      .placements
      .retain(|p| image_id == 0 || p.image_id != image_id);
  }

  pub(crate) fn remove_by_internal_image(&mut self, internal_id: u64) {
    self
      .placements
      .retain(|p| p.internal_image_id != internal_id);
  }

  /// Remove a specific placement.
  pub fn remove_by_id(&mut self, image_id: u32, placement_id: Option<u32>) {
    self.placements.retain(|p| {
      if image_id == 0 || p.image_id != image_id {
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
      .retain(|p| !placement_contains(p, line as i64, column as i64));
  }

  /// Remove matching placements and return affected internal image identities.
  pub fn remove_matching(&mut self, mut matches: impl FnMut(&ImagePlacement) -> bool) -> Vec<u64> {
    let mut affected = Vec::new();
    self.placements.retain(|p| {
      if matches(p) {
        affected.push(p.internal_image_id);
        false
      } else {
        true
      }
    });
    affected
  }

  pub fn has_image(&self, image_id: u32) -> bool {
    image_id != 0 && self.placements.iter().any(|p| p.image_id == image_id)
  }

  pub(crate) fn has_internal_image(&self, internal_id: u64) -> bool {
    self
      .placements
      .iter()
      .any(|p| p.internal_image_id == internal_id)
  }

  /// Remove all placements.
  pub fn clear(&mut self) {
    self.placements.clear();
  }

  /// Remove placements for images that no longer exist in storage.
  pub fn gc(&mut self, storage: &KittyImageStorage) {
    self
      .placements
      .retain(|p| storage.peek_by_internal(p.internal_image_id).is_some());
  }

  /// Recompute automatic cell coverage after font/cell metrics change.
  pub fn resize(&mut self, storage: &KittyImageStorage, cell_size: (f32, f32)) {
    for p in &mut self.placements {
      if let Some(image) = storage.peek_by_internal(p.internal_image_id) {
        p.update_cell_coverage((image.width, image.height), cell_size);
      }
    }
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
    cell_size: (f32, f32),
  ) -> Vec<VisiblePlacement> {
    let viewport_height = gpui::px(viewport_lines as f32 * cell_size.1);
    let mut visible: Vec<_> = self
      .placements
      .iter()
      .filter_map(|p| {
        let stored = storage.peek_by_internal(p.internal_image_id)?;
        let geometry = p.geometry((stored.width, stored.height), cell_size)?;
        let origin = gpui::point(
          gpui::px(p.column as f32 * cell_size.0),
          gpui::px((i64::from(p.line) - i64::from(viewport_top)) as f32 * cell_size.1),
        );
        let bounds = gpui::Bounds::new(geometry.bounds.origin + origin, geometry.bounds.size);
        if bounds.bottom() <= gpui::px(0.) || bounds.top() >= viewport_height {
          return None;
        }

        Some(VisiblePlacement {
          image_id: p.image_id,
          internal_image_id: stored.internal_id,
          render_image: stored.render_image.clone(),
          bounds,
          image_bounds: gpui::Bounds::new(
            geometry.image_bounds.origin + origin,
            geometry.image_bounds.size,
          ),
          z_index: p.z_index,
        })
      })
      .collect();
    visible.sort_by_key(|p| (p.z_index, p.internal_image_id));
    visible
  }

  pub fn placement_count(&self) -> usize {
    self.placements.len()
  }
}

pub fn placement_contains(p: &ImagePlacement, line: i64, column: i64) -> bool {
  line >= i64::from(p.line)
    && line < i64::from(p.line) + i64::from(p.height_cells)
    && column >= i64::from(p.column)
    && column < i64::from(p.column) + i64::from(p.width_cells)
}

#[cfg(test)]
mod tests {
  use super::*;

  fn make_placement(image_id: u32, line: i32, col: i32) -> ImagePlacement {
    ImagePlacement {
      image_id,
      internal_image_id: u64::from(image_id),
      placement_id: 0,
      line,
      column: col,
      display_columns: 10,
      display_rows: 5,
      width_cells: 10,
      height_cells: 5,
      crop: (0, 0, 0, 0),
      z_index: 0,
      x_offset: 0,
      y_offset: 0,
    }
  }

  #[test]
  fn visible_graphics_keep_crop_transform_and_sort_by_kitty_internal_image_order() {
    use super::super::command::{GraphicsLayer, KittyCommand};

    let mut storage = KittyImageStorage::new();
    let mut mgr = PlacementManager::new();
    for (id, z_index) in [(9, 0), (2, 0), (7, -1), (3, i32::MIN)] {
      storage
        .store(&KittyCommand {
          image_id: id,
          source_width: 20,
          source_height: 40,
          payload: vec![0; 20 * 40 * 4],
          ..KittyCommand::default()
        })
        .unwrap();
      let mut p = make_placement(id, -1, 1);
      p.internal_image_id = storage.peek(id).unwrap().internal_id;
      p.display_columns = 0;
      p.display_rows = 0;
      p.crop = (2, 10, 8, 25);
      p.z_index = z_index;
      p.update_cell_coverage((20, 40), (10., 20.));
      mgr.add(p);
    }
    let mut below = make_placement(2, 2, 0); // Starts below the viewport.
    below.internal_image_id = storage.peek(2).unwrap().internal_id;
    mgr.add(below);
    mgr.add(make_placement(404, 0, 0)); // Missing image data.
    let mut empty = make_placement(2, 0, 0);
    empty.internal_image_id = storage.peek(2).unwrap().internal_id;
    empty.crop = (20, 0, 0, 0);
    mgr.add(empty);

    let visible = mgr.visible_placements(&storage, 0, 2, (10., 20.));
    assert_eq!(
      visible.iter().map(|p| p.image_id).collect::<Vec<_>>(),
      [3, 7, 9, 2]
    );
    let p = &visible[0];
    assert_eq!(
      p.bounds,
      gpui::Bounds::new(
        gpui::point(gpui::px(10.), gpui::px(-20.)),
        gpui::size(gpui::px(8.), gpui::px(25.))
      )
    );
    assert_eq!(
      p.image_bounds,
      gpui::Bounds::new(
        gpui::point(gpui::px(8.), gpui::px(-30.)),
        gpui::size(gpui::px(20.), gpui::px(40.))
      )
    );
    assert!(std::sync::Arc::ptr_eq(
      &p.render_image,
      &storage.peek(3).unwrap().render_image
    ));

    for (z, expected) in [
      (i32::MIN, GraphicsLayer::BelowBackground),
      (i32::MIN / 2 - 1, GraphicsLayer::BelowBackground),
      (i32::MIN / 2, GraphicsLayer::BelowText),
      (-1, GraphicsLayer::BelowText),
      (0, GraphicsLayer::AboveText),
      (i32::MAX, GraphicsLayer::AboveText),
    ] {
      assert_eq!(GraphicsLayer::for_z_index(z), expected);
    }
  }

  #[test]
  fn resizing_graphics_updates_deletion_coverage_without_rescaling_native_pixels() {
    use super::super::command::KittyCommand;

    let mut storage = KittyImageStorage::new();
    storage
      .store(&KittyCommand {
        image_id: 1,
        source_width: 20,
        source_height: 40,
        payload: vec![0; 20 * 40 * 4],
        ..KittyCommand::default()
      })
      .unwrap();
    let mut mgr = PlacementManager::new();
    let mut p = make_placement(1, 0, 0);
    p.display_columns = 0;
    p.display_rows = 0;
    p.update_cell_coverage((20, 40), (10., 20.));
    mgr.add(p);
    assert!(!placement_contains(&mgr.placements[0], 3, 3));
    mgr.resize(&storage, (5., 10.));
    assert!(placement_contains(&mgr.placements[0], 3, 3));
    let visible = mgr.visible_placements(&storage, 0, 10, (5., 10.));
    assert_eq!(
      visible[0].bounds.size,
      gpui::size(gpui::px(20.), gpui::px(40.))
    );
    mgr.remove_at_cursor(3, 3);
    assert_eq!(mgr.placement_count(), 0);
    assert_eq!(storage.image_count(), 1);
  }

  #[test]
  fn reusing_client_ids_does_not_revive_stale_placements() {
    use super::super::command::KittyCommand;
    let mut storage = KittyImageStorage::new();
    let command = KittyCommand {
      image_id: 7,
      source_width: 1,
      source_height: 1,
      payload: vec![0; 4],
      ..KittyCommand::default()
    };
    storage.store(&command).unwrap();
    let old_identity = storage.peek(7).unwrap().internal_id;
    let mut mgr = PlacementManager::new();
    let mut p = make_placement(7, 0, 0);
    p.internal_image_id = old_identity;
    mgr.add(p);
    storage.remove(7);
    storage.store(&command).unwrap();
    assert_ne!(storage.peek(7).unwrap().internal_id, old_identity);
    assert!(
      mgr
        .visible_placements(&storage, 0, 24, (10., 20.))
        .is_empty()
    );
    mgr.gc(&storage);
    assert_eq!(mgr.placement_count(), 0);
    assert!(storage.peek(7).is_some());
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
