use base64::{Engine as _, engine::general_purpose::STANDARD};

pub const MAX_ENCODED_CHUNK_SIZE: usize = 4096;

const APC_START: &[u8] = b"\x1b_G";
const STRING_TERMINATOR: &[u8] = b"\x1b\\";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PixelFormat {
  Rgb,
  Rgba,
  Png,
}

impl PixelFormat {
  fn protocol_value(self) -> u8 {
    match self {
      Self::Rgb => 24,
      Self::Rgba => 32,
      Self::Png => 100,
    }
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransmitAction {
  Store,
  StoreAndDisplay,
}

impl TransmitAction {
  fn protocol_value(self) -> char {
    match self {
      Self::Store => 't',
      Self::StoreAndDisplay => 'T',
    }
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectUpload {
  pub format: PixelFormat,
  pub width: u32,
  pub height: u32,
  pub action: TransmitAction,
  pub image_id: u32,
  pub display_columns: u32,
  pub display_rows: u32,
  pub z_index: i32,
  pub cursor_movement: u8,
  pub quiet: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Placement {
  pub image_id: u32,
  pub placement_id: u32,
  pub crop_x: u32,
  pub crop_y: u32,
  pub crop_width: u32,
  pub crop_height: u32,
  pub display_columns: u32,
  pub display_rows: u32,
  pub z_index: i32,
  pub cursor_movement: u8,
}

pub fn encode_direct_upload(data: &[u8], upload: DirectUpload) -> Vec<Vec<u8>> {
  let mut controls = format!(
    "a={},f={},t=d,i={}",
    upload.action.protocol_value(),
    upload.format.protocol_value(),
    upload.image_id,
  );
  if upload.format != PixelFormat::Png {
    controls.push_str(&format!(",s={},v={}", upload.width, upload.height));
  }
  if upload.display_columns != 0 {
    controls.push_str(&format!(",c={}", upload.display_columns));
  }
  if upload.display_rows != 0 {
    controls.push_str(&format!(",r={}", upload.display_rows));
  }
  controls.push_str(&format!(
    ",z={},C={},q={}",
    upload.z_index, upload.cursor_movement, upload.quiet,
  ));

  encode_chunked(&controls, data)
}

pub fn encode_placement(placement: Placement) -> Vec<u8> {
  apc(
    &format!(
      "a=p,i={},p={},x={},y={},w={},h={},c={},r={},z={},C={},q=2",
      placement.image_id,
      placement.placement_id,
      placement.crop_x,
      placement.crop_y,
      placement.crop_width,
      placement.crop_height,
      placement.display_columns,
      placement.display_rows,
      placement.z_index,
      placement.cursor_movement,
    ),
    &[],
  )
}

pub fn encode_delete_placement(image_id: u32, placement_id: u32) -> Vec<u8> {
  apc(&format!("q=2,a=d,d=i,i={image_id},p={placement_id}"), &[])
}

pub fn encode_delete_image(image_id: u32) -> Vec<u8> {
  apc(&format!("q=2,a=d,d=I,i={image_id}"), &[])
}

pub fn query_order_probe() -> Vec<u8> {
  let mut probe = apc("a=q,i=31,s=1,v=1,f=24", b"AAAA");
  probe.extend_from_slice(b"\x1b[>c");
  probe
}

pub fn verify_query_before_secondary_da(response: &[u8]) -> Result<(usize, usize), String> {
  let kitty = find_subslice(response, b"\x1b_Gi=31;OK\x1b\\")
    .ok_or_else(|| "missing Kitty query response for image 31".to_string())?;
  let secondary_da = find_subslice(response, b"\x1b[>1;")
    .ok_or_else(|| "missing Secondary DA response".to_string())?;
  if kitty >= secondary_da {
    return Err("Secondary DA arrived before the Kitty query response".to_string());
  }
  Ok((kitty, secondary_da))
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
  haystack
    .windows(needle.len())
    .position(|window| window == needle)
}

pub fn rgb_test_pattern(width: u32, height: u32) -> Vec<u8> {
  let mut pixels = Vec::with_capacity(width as usize * height as usize * 3);
  for y in 0..height {
    for x in 0..width {
      pixels.extend_from_slice(&[
        ((x * 255) / width.max(1)) as u8,
        ((y * 255) / height.max(1)) as u8,
        ((x ^ y).wrapping_mul(29) & 0xff) as u8,
      ]);
    }
  }
  pixels
}

pub fn rgba_test_pattern(width: u32, height: u32) -> Vec<u8> {
  let mut pixels = Vec::with_capacity(width as usize * height as usize * 4);
  let mut noise = 0x9e37_79b9_u32;
  for y in 0..height {
    for x in 0..width {
      noise ^= noise << 13;
      noise ^= noise >> 17;
      noise ^= noise << 5;
      let checker = if (x / 8 + y / 8) % 2 == 0 { 0x30 } else { 0xd0 };
      pixels.extend_from_slice(&[
        checker,
        (noise & 0xff) as u8,
        (((x + y) * 255) / (width + height).max(1)) as u8,
        (96 + ((x * 159) / width.max(1))) as u8,
      ]);
    }
  }
  pixels
}

pub fn encode_png_rgba(
  pixels: &[u8],
  width: u32,
  height: u32,
) -> Result<Vec<u8>, png::EncodingError> {
  let mut encoded = Vec::new();
  let mut encoder = png::Encoder::new(&mut encoded, width, height);
  encoder.set_color(png::ColorType::Rgba);
  encoder.set_depth(png::BitDepth::Eight);
  let mut writer = encoder.write_header()?;
  writer.write_image_data(pixels)?;
  writer.finish()?;
  Ok(encoded)
}

pub fn encode_yazi_legacy(
  pixels: &[u8],
  format: PixelFormat,
  width: u32,
  height: u32,
) -> Vec<Vec<u8>> {
  let encoded = STANDARD.encode(pixels);
  yazi_legacy_frames(encoded.as_bytes(), format, width, height)
}

pub fn yazi_legacy_frames(
  encoded: &[u8],
  format: PixelFormat,
  width: u32,
  height: u32,
) -> Vec<Vec<u8>> {
  let chunks = encoded.chunks(MAX_ENCODED_CHUNK_SIZE).collect::<Vec<_>>();
  let chunk_count = chunks.len();

  chunks
    .into_iter()
    .enumerate()
    .map(|(index, chunk)| {
      let more = u8::from(index + 1 < chunk_count);
      let controls = if index == 0 {
        format!(
          "q=2,a=T,z=-1,C=1,f={},s={width},v={height},m={more}",
          format.protocol_value(),
        )
      } else {
        format!("m={more}")
      };
      apc(&controls, chunk)
    })
    .collect()
}

fn encode_chunked(initial_controls: &str, data: &[u8]) -> Vec<Vec<u8>> {
  let encoded = STANDARD.encode(data);
  let chunks = encoded
    .as_bytes()
    .chunks(MAX_ENCODED_CHUNK_SIZE)
    .collect::<Vec<_>>();
  let chunk_count = chunks.len();

  chunks
    .into_iter()
    .enumerate()
    .map(|(index, chunk)| {
      let more = u8::from(index + 1 < chunk_count);
      let controls = if index == 0 {
        format!("{initial_controls},m={more}")
      } else {
        format!("m={more}")
      };
      apc(&controls, chunk)
    })
    .collect()
}

pub fn yazi_delete_all() -> Vec<u8> {
  apc("q=2,a=d,d=A", &[])
}

pub fn apc(controls: &str, payload: &[u8]) -> Vec<u8> {
  let mut frame = Vec::with_capacity(
    APC_START.len()
      + controls.len()
      + usize::from(!payload.is_empty())
      + payload.len()
      + STRING_TERMINATOR.len(),
  );
  frame.extend_from_slice(APC_START);
  frame.extend_from_slice(controls.as_bytes());
  if !payload.is_empty() {
    frame.push(b';');
    frame.extend_from_slice(payload);
  }
  frame.extend_from_slice(STRING_TERMINATOR);
  frame
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn yazi_single_chunk_is_initial_and_final() {
    let frames = yazi_legacy_frames(b"YWJj", PixelFormat::Rgb, 2, 1);

    assert_eq!(
      frames,
      [b"\x1b_Gq=2,a=T,z=-1,C=1,f=24,s=2,v=1,m=0;YWJj\x1b\\"],
    );
  }

  #[test]
  fn yazi_chunk_controls_and_boundaries_are_byte_exact() {
    let encoded = vec![b'A'; MAX_ENCODED_CHUNK_SIZE * 2 + 1];
    let frames = yazi_legacy_frames(&encoded, PixelFormat::Rgba, 41, 23);

    let mut first = b"\x1b_Gq=2,a=T,z=-1,C=1,f=32,s=41,v=23,m=1;".to_vec();
    first.extend_from_slice(&encoded[..MAX_ENCODED_CHUNK_SIZE]);
    first.extend_from_slice(STRING_TERMINATOR);

    let mut continuation = b"\x1b_Gm=1;".to_vec();
    continuation.extend_from_slice(&encoded[MAX_ENCODED_CHUNK_SIZE..MAX_ENCODED_CHUNK_SIZE * 2]);
    continuation.extend_from_slice(STRING_TERMINATOR);

    assert_eq!(frames, [first, continuation, b"\x1b_Gm=0;A\x1b\\".to_vec()]);
  }

  #[test]
  fn yazi_delete_all_is_byte_exact() {
    assert_eq!(yazi_delete_all(), b"\x1b_Gq=2,a=d,d=A\x1b\\");
  }

  #[test]
  fn yazi_encoder_base64_encodes_before_chunking() {
    let pixels = vec![0xff; 3073];
    let frames = encode_yazi_legacy(&pixels, PixelFormat::Rgb, 1, 1024);

    assert_eq!(frames.len(), 2);
    assert!(frames[0].starts_with(b"\x1b_Gq=2,a=T,z=-1,C=1,f=24,s=1,v=1024,m=1;"));
    assert_eq!(frames[1], b"\x1b_Gm=0;/w==\x1b\\");
  }

  #[test]
  fn direct_rgb_upload_uses_raw_dimensions_and_chunking() {
    let data = vec![0x55; 3073];
    let frames = encode_direct_upload(
      &data,
      DirectUpload {
        format: PixelFormat::Rgb,
        width: 1,
        height: 1024,
        action: TransmitAction::StoreAndDisplay,
        image_id: 101,
        display_columns: 4,
        display_rows: 3,
        z_index: -1,
        cursor_movement: 1,
        quiet: 2,
      },
    );

    assert_eq!(frames.len(), 2);
    assert!(
      frames[0].starts_with(b"\x1b_Ga=T,f=24,t=d,i=101,s=1,v=1024,c=4,r=3,z=-1,C=1,q=2,m=1;")
    );
    assert_eq!(frames[1], b"\x1b_Gm=0;VQ==\x1b\\");
  }

  #[test]
  fn png_upload_uses_png_metadata_instead_of_raw_dimensions() {
    let pixels = rgba_test_pattern(64, 32);
    let png = encode_png_rgba(&pixels, 64, 32).expect("encode PNG fixture");
    let frames = encode_direct_upload(
      &png,
      DirectUpload {
        format: PixelFormat::Png,
        width: 64,
        height: 32,
        action: TransmitAction::StoreAndDisplay,
        image_id: 102,
        display_columns: 8,
        display_rows: 4,
        z_index: 0,
        cursor_movement: 0,
        quiet: 2,
      },
    );

    assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
    assert!(frames.len() > 1);
    assert!(frames[0].starts_with(b"\x1b_Ga=T,f=100,t=d,i=102,c=8,r=4,z=0,C=0,q=2,m=1;"));
  }

  #[test]
  fn placement_and_deletion_controls_are_byte_exact() {
    let placement = encode_placement(Placement {
      image_id: 103,
      placement_id: 7,
      crop_x: 4,
      crop_y: 5,
      crop_width: 20,
      crop_height: 10,
      display_columns: 6,
      display_rows: 3,
      z_index: -2,
      cursor_movement: 1,
    });

    assert_eq!(
      placement,
      b"\x1b_Ga=p,i=103,p=7,x=4,y=5,w=20,h=10,c=6,r=3,z=-2,C=1,q=2\x1b\\"
    );
    assert_eq!(
      encode_delete_placement(103, 7),
      b"\x1b_Gq=2,a=d,d=i,i=103,p=7\x1b\\"
    );
    assert_eq!(encode_delete_image(103), b"\x1b_Gq=2,a=d,d=I,i=103\x1b\\");
  }

  #[test]
  fn query_order_probe_is_byte_exact() {
    assert_eq!(
      query_order_probe(),
      b"\x1b_Ga=q,i=31,s=1,v=1,f=24;AAAA\x1b\\\x1b[>c"
    );
  }

  #[test]
  fn accepts_kitty_response_before_secondary_da() {
    let response = b"noise\x1b_Gi=31;OK\x1b\\\x1b[>1;202603;0c";

    assert_eq!(verify_query_before_secondary_da(response), Ok((5, 17)));
  }

  #[test]
  fn rejects_secondary_da_before_kitty_response() {
    let response = b"\x1b[>1;202603;0c\x1b_Gi=31;OK\x1b\\";

    assert_eq!(
      verify_query_before_secondary_da(response),
      Err("Secondary DA arrived before the Kitty query response".to_string()),
    );
  }
}
