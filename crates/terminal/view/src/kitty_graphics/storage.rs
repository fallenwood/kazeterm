use std::{collections::HashMap, io::Cursor, sync::Arc};

use flate2::{Decompress, FlushDecompress, Status};
use gpui::RenderImage;
use image::{ImageBuffer, ImageFormat, ImageReader, Limits, Rgba};
use tracing::warn;

use super::command::{KittyCommand, KittyFormat, MAX_IMAGE_BYTES, StoredImage};

#[derive(Clone)]
pub struct PreparedImage {
  pub render_image: Arc<RenderImage>,
  pub width: u32,
  pub height: u32,
  pub memory_bytes: usize,
}

/// Image storage with LRU eviction.
pub struct KittyImageStorage {
  images: HashMap<u32, StoredImage>,
  /// Image numbers resolve to the newest surviving transmission, not LRU order.
  image_numbers: HashMap<u32, Vec<u32>>,
  /// Access order for LRU eviction (most recently used at the back).
  access_order: Vec<u32>,
  /// Total memory used by stored images.
  total_memory: usize,
  /// Maximum memory before eviction.
  max_memory: usize,
  /// Next auto-assigned client/storage image ID.
  next_id: u32,
  /// Last internal creation identity; independent of LRU and client-ID reuse.
  last_internal_id: u64,
}

impl KittyImageStorage {
  pub fn new() -> Self {
    Self {
      images: HashMap::new(),
      image_numbers: HashMap::new(),
      access_order: Vec::new(),
      total_memory: 0,
      max_memory: MAX_IMAGE_BYTES,
      next_id: 1,
      last_internal_id: 0,
    }
  }

  /// Store a decoded image from a completed Kitty command.
  /// Returns the assigned image ID, or an error message.
  pub fn store(&mut self, cmd: &KittyCommand) -> Result<u32, String> {
    let mut command = cmd.clone();
    let prepared = prepare_image(&mut command)?;
    self.store_prepared(cmd.image_id, cmd.image_number, prepared)
  }

  pub fn store_prepared(
    &mut self,
    requested_image_id: u32,
    image_number: u32,
    prepared: PreparedImage,
  ) -> Result<u32, String> {
    if prepared.memory_bytes > self.max_memory {
      return Err(format!(
        "Decoded image uses {} bytes, exceeding the {} byte storage limit",
        prepared.memory_bytes, self.max_memory
      ));
    }

    let image_id = if requested_image_id == 0 {
      self.allocate_id()
    } else {
      requested_image_id
    };

    let internal_id = if let Some(image) = self.images.get(&image_id) {
      // Kitty's find_or_create_image retains identity on explicit replacement.
      image.internal_id
    } else {
      self.last_internal_id = self
        .last_internal_id
        .checked_add(1)
        .ok_or_else(|| "Image identity space exhausted".to_string())?;
      self.last_internal_id
    };

    // Replacing an image must release its old allocation before quota checks,
    // otherwise an unrelated image can be evicted unnecessarily.
    self.remove(image_id);

    // Evict old images if needed.
    while self.total_memory + prepared.memory_bytes > self.max_memory
      && !self.access_order.is_empty()
    {
      self.evict_oldest();
    }

    self.images.insert(
      image_id,
      StoredImage {
        id: image_id,
        internal_id,
        render_image: prepared.render_image,
        width: prepared.width,
        height: prepared.height,
        memory_bytes: prepared.memory_bytes,
      },
    );
    self.access_order.push(image_id);
    if image_number != 0 {
      self
        .image_numbers
        .entry(image_number)
        .or_default()
        .push(image_id);
    }
    self.total_memory += prepared.memory_bytes;

    Ok(image_id)
  }

  pub fn newest_for_number(&self, image_number: u32) -> Option<u32> {
    self.image_numbers.get(&image_number)?.last().copied()
  }

  pub fn image_ids(&self) -> impl Iterator<Item = u32> + '_ {
    self.images.keys().copied()
  }

  /// Get a stored image, updating access order for LRU.
  pub fn get(&mut self, image_id: u32) -> Option<&StoredImage> {
    if self.images.contains_key(&image_id) {
      self.touch(image_id);
      self.images.get(&image_id)
    } else {
      None
    }
  }

  /// Get without updating LRU (for rendering).
  pub fn peek(&self, image_id: u32) -> Option<&StoredImage> {
    self.images.get(&image_id)
  }

  /// Delete an image by ID.
  pub fn remove(&mut self, image_id: u32) -> bool {
    if let Some(img) = self.images.remove(&image_id) {
      self.total_memory = self.total_memory.saturating_sub(img.memory_bytes);
      self.access_order.retain(|&id| id != image_id);
      self.image_numbers.retain(|_, ids| {
        ids.retain(|&id| id != image_id);
        !ids.is_empty()
      });
      true
    } else {
      false
    }
  }

  /// Delete all images.
  pub fn clear(&mut self) {
    self.images.clear();
    self.image_numbers.clear();
    self.access_order.clear();
    self.total_memory = 0;
  }

  pub fn image_count(&self) -> usize {
    self.images.len()
  }

  fn allocate_id(&mut self) -> u32 {
    while self.images.contains_key(&self.next_id) {
      self.next_id = self.next_id.wrapping_add(1).max(1);
    }
    let id = self.next_id;
    self.next_id = self.next_id.wrapping_add(1).max(1);
    id
  }

  fn touch(&mut self, image_id: u32) {
    self.access_order.retain(|&id| id != image_id);
    self.access_order.push(image_id);
  }

  fn evict_oldest(&mut self) {
    if let Some(oldest_id) = self.access_order.first().copied() {
      self.remove(oldest_id);
    }
  }
}

/// Decode raw image data from a Kitty command payload into BGRA pixels.
///
/// GPUI's `paint_image` expects BGRA format, so all decode paths convert to BGRA.
pub fn prepare_image(cmd: &mut KittyCommand) -> Result<PreparedImage, String> {
  let (width, height, rgba_data) = decode_image_data(cmd)?;
  let memory_bytes = image_byte_len(width, height, 4)?;
  let img_buf: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_raw(width, height, rgba_data)
    .ok_or_else(|| "Failed to create image buffer".to_string())?;
  let frame = image::Frame::new(img_buf);

  Ok(PreparedImage {
    render_image: Arc::new(RenderImage::new(vec![frame])),
    width,
    height,
    memory_bytes,
  })
}

/// Validate a query using the same bounded decoder as uploads, without creating
/// a render image or touching storage, image IDs, placements, or LRU state.
pub fn validate_image(cmd: &mut KittyCommand) -> Result<(), String> {
  decode_image_data(cmd).map(|_| ())
}

fn decode_image_data(cmd: &mut KittyCommand) -> Result<(u32, u32, Vec<u8>), String> {
  let expected = match cmd.format {
    KittyFormat::Png => cmd.data_size as usize,
    KittyFormat::Rgb | KittyFormat::Rgba => {
      if cmd.source_width == 0 || cmd.source_height == 0 {
        return Err("EINVAL:Raw pixel data requires nonzero s and v".into());
      }
      // Check the final four-byte pixel allocation before decompression or RGB conversion.
      image_byte_len(cmd.source_width, cmd.source_height, 4)?;
      image_byte_len(
        cmd.source_width,
        cmd.source_height,
        if cmd.format == KittyFormat::Rgb { 3 } else { 4 },
      )?
    }
  };
  if cmd.compressed {
    if expected == 0 || expected > MAX_IMAGE_BYTES {
      return Err("EINVAL:Compressed PNG requires a bounded, nonzero S size".into());
    }
    cmd.payload = decompress_zlib(&cmd.payload, expected)?;
    cmd.compressed = false;
  }
  let (w, h, mut data) = match cmd.format {
    KittyFormat::Png => {
      let decoded = decode_png(&cmd.payload);
      cmd.payload.clear();
      decoded?
    }
    KittyFormat::Rgba => {
      let w = cmd.source_width;
      let h = cmd.source_height;
      if w == 0 || h == 0 {
        return Err("RGBA format requires s= and v= (width/height)".to_string());
      }
      let expected = image_byte_len(w, h, 4)?;
      if cmd.payload.len() != expected {
        return Err(format!(
          "ENODATA:RGBA payload size mismatch: expected {} got {}",
          expected,
          cmd.payload.len()
        ));
      }
      (w, h, std::mem::take(&mut cmd.payload))
    }
    KittyFormat::Rgb => {
      let w = cmd.source_width;
      let h = cmd.source_height;
      if w == 0 || h == 0 {
        return Err("RGB format requires s= and v= (width/height)".to_string());
      }
      let expected = image_byte_len(w, h, 3)?;
      if cmd.payload.len() != expected {
        return Err(format!(
          "ENODATA:RGB payload size mismatch: expected {} got {}",
          expected,
          cmd.payload.len()
        ));
      }
      // Convert RGB to RGBA.
      let mut rgba = Vec::with_capacity(image_byte_len(w, h, 4)?);
      for chunk in std::mem::take(&mut cmd.payload).chunks_exact(3) {
        rgba.push(chunk[0]);
        rgba.push(chunk[1]);
        rgba.push(chunk[2]);
        rgba.push(255);
      }
      (w, h, rgba)
    }
  };

  // Convert RGBA to BGRA (swap R and B channels) for GPUI rendering.
  for chunk in data.chunks_exact_mut(4) {
    chunk.swap(0, 2);
  }

  Ok((w, h, data))
}

fn image_byte_len(width: u32, height: u32, bytes_per_pixel: usize) -> Result<usize, String> {
  let size = (width as usize)
    .checked_mul(height as usize)
    .and_then(|pixels| pixels.checked_mul(bytes_per_pixel))
    .ok_or_else(|| "E2BIG:Image dimensions overflow addressable memory".to_string())?;
  if size > MAX_IMAGE_BYTES {
    return Err("E2BIG:Decoded image exceeds the memory quota".into());
  }
  Ok(size)
}

fn decompress_zlib(data: &[u8], expected: usize) -> Result<Vec<u8>, String> {
  if expected > MAX_IMAGE_BYTES {
    return Err("E2BIG:Decompressed image exceeds the memory quota".into());
  }
  let mut decoded = Vec::with_capacity(expected + 1);
  let mut decoder = Decompress::new(true);
  let status = decoder
    .decompress_vec(data, &mut decoded, FlushDecompress::Finish)
    .map_err(|_| "EINVAL:Invalid zlib image data")?;
  if status != Status::StreamEnd
    || decoded.len() != expected
    || decoder.total_in() != data.len() as u64
  {
    return Err("EINVAL:Truncated, oversized or incorrectly sized zlib image data".into());
  }
  Ok(decoded)
}

fn decode_png(data: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
  let mut reader = ImageReader::with_format(Cursor::new(data), ImageFormat::Png);
  let mut limits = Limits::default();
  limits.max_alloc = Some(MAX_IMAGE_BYTES as u64);
  reader.limits(limits);
  let img = reader.decode().map_err(|e| {
    warn!("Failed to decode PNG image: {e}");
    format!("EINVAL:Failed to decode PNG image: {e}")
  })?;
  image_byte_len(img.width(), img.height(), 4)?;

  let rgba = img.to_rgba8();
  let (w, h) = rgba.dimensions();
  Ok((w, h, rgba.into_raw()))
}

#[cfg(test)]
mod tests {
  use super::*;

  fn make_test_command(image_id: u32, width: u32, height: u32) -> KittyCommand {
    let mut rgba_data = vec![0u8; (width as usize) * (height as usize) * 4];
    // Fill with red pixels.
    for chunk in rgba_data.chunks_exact_mut(4) {
      chunk[0] = 255;
      chunk[3] = 255;
    }
    KittyCommand {
      image_id,
      format: KittyFormat::Rgba,
      source_width: width,
      source_height: height,
      payload: rgba_data,
      ..Default::default()
    }
  }

  #[test]
  fn test_store_and_get() {
    let mut storage = KittyImageStorage::new();
    let cmd = make_test_command(1, 10, 10);
    let id = storage.store(&cmd).unwrap();
    assert_eq!(id, 1);
    assert!(storage.get(1).is_some());
    assert_eq!(storage.image_count(), 1);
  }

  #[test]
  fn test_auto_id() {
    let mut storage = KittyImageStorage::new();
    let cmd = make_test_command(0, 2, 2);
    let id = storage.store(&cmd).unwrap();
    assert!(id > 0);
  }

  #[test]
  fn test_remove() {
    let mut storage = KittyImageStorage::new();
    let cmd = make_test_command(5, 2, 2);
    storage.store(&cmd).unwrap();
    assert!(storage.remove(5));
    assert!(storage.peek(5).is_none());
    assert_eq!(storage.image_count(), 0);
  }

  #[test]
  fn test_clear() {
    let mut storage = KittyImageStorage::new();
    for i in 1..=5 {
      let cmd = make_test_command(i, 2, 2);
      storage.store(&cmd).unwrap();
    }
    assert_eq!(storage.image_count(), 5);
    storage.clear();
    assert_eq!(storage.image_count(), 0);
  }

  #[test]
  fn auto_ids_do_not_replace_explicit_ids() {
    let mut storage = KittyImageStorage::new();
    storage.store(&make_test_command(1, 1, 1)).unwrap();
    let id = storage.store(&make_test_command(0, 1, 1)).unwrap();
    assert_ne!(id, 1);
    assert_eq!(storage.image_count(), 2);
    storage.next_id = u32::MAX;
    assert_eq!(
      storage.store(&make_test_command(0, 1, 1)).unwrap(),
      u32::MAX
    );
    let wrapped = storage.store(&make_test_command(0, 1, 1)).unwrap();
    assert_ne!(wrapped, 1);
    assert_ne!(wrapped, id);
    assert_eq!(storage.image_count(), 4);
  }

  #[test]
  fn graphics_internal_identity_survives_replacement_and_lru_but_not_deletion() {
    let mut storage = KittyImageStorage::new();
    for id in [9, 2] {
      storage.store(&make_test_command(id, 1, 1)).unwrap();
    }
    let first = storage.peek(9).unwrap().internal_id;
    let second = storage.peek(2).unwrap().internal_id;
    assert!(first < second);
    storage.get(9);
    storage.store(&make_test_command(9, 1, 1)).unwrap();
    assert_eq!(storage.peek(9).unwrap().internal_id, first);
    storage.remove(9);
    storage.store(&make_test_command(9, 1, 1)).unwrap();
    assert!(storage.peek(9).unwrap().internal_id > second);

    storage.last_internal_id = u64::MAX;
    storage.store(&make_test_command(9, 1, 1)).unwrap(); // Replacement still works.
    assert!(storage.store(&make_test_command(7, 1, 1)).is_err());
    assert!(storage.peek(2).is_some()); // Failure did not evict existing data.
  }

  #[test]
  fn replacement_does_not_evict_unrelated_images() {
    let mut storage = KittyImageStorage::new();
    storage.max_memory = 8;
    for id in [1, 2, 2] {
      storage.store(&make_test_command(id, 1, 1)).unwrap();
    }
    assert!(storage.peek(1).is_some());
    assert!(storage.peek(2).is_some());
    assert_eq!(storage.total_memory, 8);
  }

  fn compress(data: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(data).unwrap();
    encoder.finish().unwrap()
  }

  #[test]
  fn zlib_raw_and_png_transfers_decode_to_the_same_bgra_pixels() {
    let rgba = vec![255, 0, 0, 128];
    let image = image::DynamicImage::ImageRgba8(ImageBuffer::from_raw(1, 1, rgba.clone()).unwrap());
    let mut png = Cursor::new(Vec::new());
    image.write_to(&mut png, ImageFormat::Png).unwrap();
    for (format, payload, expected_alpha) in [
      (KittyFormat::Rgb, vec![255, 0, 0], 255),
      (KittyFormat::Rgba, rgba, 128),
      (KittyFormat::Png, png.into_inner(), 128),
    ] {
      for compressed in [false, true] {
        let mut cmd = KittyCommand {
          format,
          source_width: 1,
          source_height: 1,
          compressed,
          data_size: payload.len() as u32,
          payload: if compressed {
            compress(&payload)
          } else {
            payload.clone()
          },
          ..Default::default()
        };
        let (width, height, pixels) = decode_image_data(&mut cmd).unwrap();
        assert_eq!((width, height), (1, 1));
        assert_eq!(pixels, [0, 0, 255, expected_alpha]);
        assert!(cmd.payload.is_empty());
      }
    }
  }

  #[test]
  fn decoder_rejects_corrupt_compressed_and_oversized_image_data() {
    let compressed = compress(&[255, 0, 0, 255]);
    assert!(decompress_zlib(&compressed, 3).is_err());
    assert!(decompress_zlib(&compressed, 5).is_err());
    assert!(decompress_zlib(&compressed[..compressed.len() - 1], 4).is_err());
    assert!(decompress_zlib(b"invalid zlib", 4).is_err());
    let mut trailing = compressed.clone();
    trailing.push(0);
    assert!(decompress_zlib(&trailing, 4).is_err());
    assert!(decompress_zlib(&compressed, MAX_IMAGE_BYTES + 1).is_err());
    for (width, height) in [(u32::MAX, u32::MAX), (100_000, 100_000), (0, 1)] {
      let mut cmd = make_test_command(1, 1, 1);
      cmd.source_width = width;
      cmd.source_height = height;
      assert!(prepare_image(&mut cmd).is_err());
    }
    let mut cmd = KittyCommand {
      format: KittyFormat::Png,
      compressed: true,
      payload: compressed,
      ..Default::default()
    };
    assert!(prepare_image(&mut cmd).is_err()); // missing S

    // f=100 must not silently accept a different image format.
    let mut bmp = Cursor::new(Vec::new());
    image::DynamicImage::new_rgba8(1, 1)
      .write_to(&mut bmp, ImageFormat::Bmp)
      .unwrap();
    cmd.compressed = false;
    cmd.payload = bmp.into_inner();
    assert!(prepare_image(&mut cmd).is_err());
  }

  #[test]
  fn image_numbers_resolve_by_creation_order_and_survive_lru_changes() {
    let mut storage = KittyImageStorage::new();
    let mut cmd = make_test_command(0, 1, 1);
    cmd.image_number = 9;
    let first = storage.store(&cmd).unwrap();
    let second = storage.store(&cmd).unwrap();
    assert_ne!(first, second);
    storage.get(first); // LRU access must not make this the newest transmission.
    assert_eq!(storage.newest_for_number(9), Some(second));
    storage.remove(second);
    assert_eq!(storage.newest_for_number(9), Some(first));
    storage.max_memory = 4;
    storage.store(&make_test_command(7, 1, 1)).unwrap();
    assert_eq!(storage.newest_for_number(9), None);
    assert!(storage.image_numbers.is_empty());
  }

  #[test]
  fn test_lru_eviction() {
    let mut storage = KittyImageStorage::new();
    storage.max_memory = 10 * 10 * 4; // Only room for one 10x10 RGBA image (400 bytes).

    // Store image 1: 10x10 = 400 bytes — fills the cache.
    let cmd1 = make_test_command(1, 10, 10);
    storage.store(&cmd1).unwrap();
    assert_eq!(storage.image_count(), 1);

    // Store image 2: also 400 bytes. Should evict image 1.
    let cmd2 = make_test_command(2, 10, 10);
    storage.store(&cmd2).unwrap();
    assert_eq!(storage.image_count(), 1);
    assert!(storage.peek(1).is_none());
    assert!(storage.peek(2).is_some());
  }
}
