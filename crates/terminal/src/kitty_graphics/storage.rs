use std::{collections::HashMap, sync::Arc};

use gpui::RenderImage;
use image::{ImageBuffer, Rgba};
use tracing::warn;

use super::command::{KittyCommand, KittyFormat, StoredImage};

const DEFAULT_MAX_MEMORY: usize = 320 * 1024 * 1024; // 320 MB

/// Image storage with LRU eviction.
pub struct KittyImageStorage {
  images: HashMap<u32, StoredImage>,
  /// Access order for LRU eviction (most recently used at the back).
  access_order: Vec<u32>,
  /// Total memory used by stored images.
  total_memory: usize,
  /// Maximum memory before eviction.
  max_memory: usize,
  /// Next auto-assigned image ID.
  next_id: u32,
}

impl KittyImageStorage {
  pub fn new() -> Self {
    Self {
      images: HashMap::new(),
      access_order: Vec::new(),
      total_memory: 0,
      max_memory: DEFAULT_MAX_MEMORY,
      next_id: 1,
    }
  }

  /// Store a decoded image from a completed Kitty command.
  /// Returns the assigned image ID, or an error message.
  pub fn store(&mut self, cmd: &KittyCommand) -> Result<u32, String> {
    let image_id = if cmd.image_id == 0 {
      self.allocate_id()
    } else {
      cmd.image_id
    };

    let (width, height, rgba_data) = decode_image_data(cmd)?;

    let img_buf: ImageBuffer<Rgba<u8>, Vec<u8>> =
      ImageBuffer::from_raw(width, height, rgba_data.clone())
        .ok_or_else(|| "Failed to create image buffer".to_string())?;

    let frame = image::Frame::new(img_buf);
    let render_image = Arc::new(RenderImage::new(vec![frame]));
    let memory_bytes = (width as usize) * (height as usize) * 4;

    // Evict old images if needed.
    while self.total_memory + memory_bytes > self.max_memory && !self.access_order.is_empty() {
      self.evict_oldest();
    }

    // Remove existing image with same ID.
    if let Some(old) = self.images.remove(&image_id) {
      self.total_memory = self.total_memory.saturating_sub(old.memory_bytes);
      self.access_order.retain(|&id| id != image_id);
    }

    self.images.insert(
      image_id,
      StoredImage {
        id: image_id,
        render_image,
        width,
        height,
        memory_bytes,
      },
    );
    self.access_order.push(image_id);
    self.total_memory += memory_bytes;

    Ok(image_id)
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
      true
    } else {
      false
    }
  }

  /// Delete all images.
  pub fn clear(&mut self) {
    self.images.clear();
    self.access_order.clear();
    self.total_memory = 0;
  }

  pub fn image_count(&self) -> usize {
    self.images.len()
  }

  fn allocate_id(&mut self) -> u32 {
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
      if let Some(img) = self.images.remove(&oldest_id) {
        self.total_memory = self.total_memory.saturating_sub(img.memory_bytes);
      }
      self.access_order.remove(0);
    }
  }
}

/// Decode raw image data from a Kitty command payload into BGRA pixels.
///
/// GPUI's `paint_image` expects BGRA format, so all decode paths convert to BGRA.
fn decode_image_data(cmd: &KittyCommand) -> Result<(u32, u32, Vec<u8>), String> {
  let (w, h, mut data) = match cmd.format {
    KittyFormat::Png => decode_png(&cmd.payload)?,
    KittyFormat::Rgba => {
      let w = cmd.source_width;
      let h = cmd.source_height;
      if w == 0 || h == 0 {
        return Err("RGBA format requires s= and v= (width/height)".to_string());
      }
      let expected = (w as usize) * (h as usize) * 4;
      if cmd.payload.len() != expected {
        return Err(format!(
          "RGBA payload size mismatch: expected {} got {}",
          expected,
          cmd.payload.len()
        ));
      }
      (w, h, cmd.payload.clone())
    }
    KittyFormat::Rgb => {
      let w = cmd.source_width;
      let h = cmd.source_height;
      if w == 0 || h == 0 {
        return Err("RGB format requires s= and v= (width/height)".to_string());
      }
      let expected = (w as usize) * (h as usize) * 3;
      if cmd.payload.len() != expected {
        return Err(format!(
          "RGB payload size mismatch: expected {} got {}",
          expected,
          cmd.payload.len()
        ));
      }
      // Convert RGB to RGBA.
      let mut rgba = Vec::with_capacity((w as usize) * (h as usize) * 4);
      for chunk in cmd.payload.chunks_exact(3) {
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

fn decode_png(data: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
  let img = image::load_from_memory_with_format(data, image::ImageFormat::Png)
    .or_else(|_| {
      // Fall back to auto-detect format (some clients send JPEG as f=100).
      image::load_from_memory(data)
    })
    .map_err(|e| {
      warn!("Failed to decode image: {}", e);
      format!("Failed to decode image: {e}")
    })?;

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
