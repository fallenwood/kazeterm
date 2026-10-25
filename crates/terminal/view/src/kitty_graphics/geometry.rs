//! Static placement geometry derived from Kitty's update_src_rect(),
//! update_dest_rect(), and grman_update_layers() at commit 617ddee75b02.

use gpui::{Bounds, Pixels, point, px, size};

use super::command::ImagePlacement;

pub(crate) struct PlacementGeometry {
  pub width_cells: u32,
  pub height_cells: u32,
  /// Both rectangles are relative to the placement's starting cell.
  pub bounds: Bounds<Pixels>,
  pub image_bounds: Bounds<Pixels>,
}

fn cell_offset(offset: u32, cell_size: f32) -> u32 {
  // The view can have fractional logical cell sizes. Retain the largest whole
  // pixel offset strictly inside the cell, matching size - 1 for integer sizes.
  offset.min((cell_size.max(1.0).ceil() - 1.0) as u32)
}

impl ImagePlacement {
  pub(crate) fn geometry(
    &self,
    image_size: (u32, u32),
    cell_size: (f32, f32),
  ) -> Option<PlacementGeometry> {
    let (image_width, image_height) = image_size;
    let (x, y, width, height) = self.crop;
    let x = x.min(image_width);
    let y = y.min(image_height);
    let width = if width == 0 { image_width } else { width }.min(image_width - x);
    let height = if height == 0 { image_height } else { height }.min(image_height - y);
    // An empty intersection is acknowledged but produces no draw command. In
    // particular, never divide by zero or send non-finite coordinates to GPUI.
    if width == 0 || height == 0 {
      return None;
    }

    // Use floating point cell metrics without rounding native images up to
    // whole cells. f64 intermediates avoid overflow from untrusted u32 sizes.
    let cw = f64::from(cell_size.0.max(1.0));
    let ch = f64::from(cell_size.1.max(1.0));
    let ox = f64::from(cell_offset(self.x_offset, cell_size.0));
    let oy = f64::from(cell_offset(self.y_offset, cell_size.1));
    let sw = f64::from(width);
    let sh = f64::from(height);
    let columns = f64::from(self.display_columns);
    let rows = f64::from(self.display_rows);

    let dest_width = if self.display_columns != 0 {
      columns * cw - ox
    } else if self.display_rows != 0 {
      (rows * ch - oy) * sw / sh
    } else {
      sw
    };
    let dest_height = if self.display_rows != 0 {
      rows * ch - oy
    } else {
      dest_width * sh / sw
    };

    // Preserve the reference's effective cell counts, independently of the
    // pixel destination. With one explicit axis update_dest_rect includes that
    // axis's offset in its auto-axis calculation, whereas rendering subtracts it.
    let width_cells = if self.display_columns != 0 {
      self.display_columns
    } else if self.display_rows != 0 {
      (((rows * ch + oy) * sw / sh) / cw).ceil() as u32
    } else {
      ((sw + ox) / cw).ceil() as u32
    };
    let height_cells = if self.display_rows != 0 {
      self.display_rows
    } else if self.display_columns != 0 {
      (((columns * cw + ox) * sh / sw) / ch).ceil() as u32
    } else {
      ((sh + oy) / ch).ceil() as u32
    };

    let sx = dest_width / sw;
    let sy = dest_height / sh;
    Some(PlacementGeometry {
      width_cells,
      height_cells,
      bounds: Bounds::new(
        point(px(ox as f32), px(oy as f32)),
        size(px(dest_width as f32), px(dest_height as f32)),
      ),
      image_bounds: Bounds::new(
        point(
          px((ox - f64::from(x) * sx) as f32),
          px((oy - f64::from(y) * sy) as f32),
        ),
        size(
          px((f64::from(image_width) * sx) as f32),
          px((f64::from(image_height) * sy) as f32),
        ),
      ),
    })
  }

  pub(crate) fn update_cell_coverage(&mut self, image_size: (u32, u32), cell_size: (f32, f32)) {
    // Like grman_resize, shrinking a cell also clamps the retained offsets.
    self.x_offset = cell_offset(self.x_offset, cell_size.0);
    self.y_offset = cell_offset(self.y_offset, cell_size.1);
    (self.width_cells, self.height_cells) = self
      .geometry(image_size, cell_size)
      .map(|g| (g.width_cells, g.height_cells))
      .unwrap_or((0, 0));
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn placement() -> ImagePlacement {
    ImagePlacement {
      image_id: 1,
      placement_id: 0,
      line: 0,
      column: 0,
      display_columns: 0,
      display_rows: 0,
      width_cells: 0,
      height_cells: 0,
      crop: (0, 0, 0, 0),
      z_index: 0,
      x_offset: 0,
      y_offset: 0,
    }
  }

  fn assert_rect(rect: Bounds<Pixels>, expected: [f32; 4]) {
    let actual = [
      f32::from(rect.origin.x),
      f32::from(rect.origin.y),
      f32::from(rect.size.width),
      f32::from(rect.size.height),
    ];
    for (actual, expected) in actual.into_iter().zip(expected) {
      assert!((actual - expected).abs() < 0.001, "{actual} != {expected}");
    }
  }

  #[test]
  fn native_and_single_axis_sizes_preserve_pixels_and_aspect_ratio() {
    let mut p = placement();
    for (cols, rows, expected, coverage) in [
      (0, 0, [0., 0., 31., 17.], (4, 1)),
      (3, 0, [0., 0., 30., 30. * 17. / 31.], (3, 1)),
      (0, 2, [0., 0., 40. * 31. / 17., 40.], (8, 2)),
      // The pinned implementation uses the explicit rectangle, not letterboxing.
      (3, 2, [0., 0., 30., 40.], (3, 2)),
    ] {
      p.display_columns = cols;
      p.display_rows = rows;
      let g = p.geometry((31, 17), (10., 20.)).unwrap();
      assert_rect(g.bounds, expected);
      assert_eq!((g.width_cells, g.height_cells), coverage);
    }
    let g = placement().geometry((31, 17), (7.5, 15.5)).unwrap();
    assert_rect(g.bounds, [0., 0., 31., 17.]);
    assert_eq!((g.width_cells, g.height_cells), (5, 2));
  }

  #[test]
  fn crop_intersection_and_full_image_transform_do_not_stretch_the_uncropped_image() {
    let mut p = placement();
    p.crop = (2, 1, 3, 5);
    p.x_offset = 3;
    p.y_offset = 1;
    p.display_columns = 4;
    p.display_rows = 1;
    let g = p.geometry((10, 20), (10., 20.)).unwrap();
    // Derived from Kitty's test_image_put (source crop and cell offsets).
    assert_rect(g.bounds, [3., 1., 37., 19.]);
    assert_rect(
      g.image_bounds,
      [3. - 2. * 37. / 3., 1. - 19. / 5., 370. / 3., 76.],
    );

    p.display_columns = 0;
    p.display_rows = 0;
    p.crop = (8, 17, u32::MAX, u32::MAX);
    let g = p.geometry((10, 20), (10., 20.)).unwrap();
    assert_rect(g.bounds, [3., 1., 2., 3.]);
    assert_rect(g.image_bounds, [-5., -16., 10., 20.]);
    p.crop = (8, 17, 0, 0); // Default extents mean the remaining source area.
    assert_rect(
      p.geometry((10, 20), (10., 20.)).unwrap().bounds,
      [3., 1., 2., 3.],
    );
    for crop in [(10, 0, 1, 1), (0, 20, 0, 0), (u32::MAX, u32::MAX, 1, 1)] {
      p.crop = crop;
      assert!(p.geometry((10, 20), (10., 20.)).is_none());
    }
  }

  #[test]
  fn offsets_cell_coverage_resize_and_large_controls_are_bounded() {
    let mut p = placement();
    p.x_offset = 5;
    p.y_offset = 5;
    p.update_cell_coverage((10, 20), (10., 20.));
    assert_eq!((p.width_cells, p.height_cells), (2, 2));
    assert_rect(
      p.geometry((10, 20), (10., 20.)).unwrap().bounds,
      [5., 5., 10., 20.],
    );
    p.update_cell_coverage((10, 20), (4., 4.));
    assert_eq!((p.x_offset, p.y_offset), (3, 3));
    assert_eq!((p.width_cells, p.height_cells), (4, 6));
    p.update_cell_coverage((10, 20), (10., 20.));
    assert_eq!((p.x_offset, p.y_offset), (3, 3));

    p.display_columns = 3;
    let g = p.geometry((10, 20), (10., 20.)).unwrap();
    assert_eq!((g.width_cells, g.height_cells), (3, 4));
    assert_rect(g.bounds, [3., 3., 27., 54.]);
    p.display_columns = 0;
    p.display_rows = 2;
    let g = p.geometry((10, 20), (10., 20.)).unwrap();
    assert_eq!((g.width_cells, g.height_cells), (3, 2));
    assert_rect(g.bounds, [3., 3., 18.5, 37.]);

    p.x_offset = u32::MAX;
    p.y_offset = u32::MAX;
    let g = p.geometry((10, 20), (7.5, 15.5)).unwrap();
    assert_eq!(g.bounds.origin, point(px(7.), px(15.)));
    p.display_columns = u32::MAX;
    p.display_rows = u32::MAX;
    let g = p.geometry((1, 1), (10., 20.)).unwrap();
    assert_rect(
      Bounds::new(g.bounds.origin, size(px(1.), px(1.))),
      [9., 19., 1., 1.],
    );
    assert!(f32::from(g.image_bounds.size.width).is_finite());
    assert!(f32::from(g.image_bounds.size.height).is_finite());
    assert_eq!((g.width_cells, g.height_cells), (u32::MAX, u32::MAX));
  }
}
