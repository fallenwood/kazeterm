use std::{
  fs,
  io::{self, Write},
  path::Path,
  process::Command,
  thread,
  time::Duration,
};

use kitty_protocol_client::{
  DirectUpload, PixelFormat, Placement, TransmitAction, encode_delete_image,
  encode_delete_placement, encode_direct_upload, encode_placement, encode_png_rgba,
  encode_yazi_legacy, query_order_probe, rgb_test_pattern, rgba_test_pattern,
  verify_query_before_secondary_da, yazi_delete_all,
};

#[cfg(windows)]
mod windows_console {
  use std::{
    io, ptr,
    time::{Duration, Instant},
  };

  use windows_sys::Win32::{
    Foundation::{HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT},
    Storage::FileSystem::ReadFile,
    System::{
      Console::{
        ENABLE_ECHO_INPUT, ENABLE_LINE_INPUT, ENABLE_PROCESSED_INPUT,
        ENABLE_VIRTUAL_TERMINAL_INPUT, GetConsoleMode, GetStdHandle, STD_INPUT_HANDLE,
        SetConsoleMode,
      },
      Threading::WaitForSingleObject,
    },
  };

  struct InputMode {
    handle: HANDLE,
    original: u32,
  }

  impl InputMode {
    fn raw() -> io::Result<Self> {
      let handle = unsafe { GetStdHandle(STD_INPUT_HANDLE) };
      let mut original = 0;
      if unsafe { GetConsoleMode(handle, &mut original) } == 0 {
        return Err(io::Error::last_os_error());
      }
      let raw = (original & !(ENABLE_ECHO_INPUT | ENABLE_LINE_INPUT | ENABLE_PROCESSED_INPUT))
        | ENABLE_VIRTUAL_TERMINAL_INPUT;
      if unsafe { SetConsoleMode(handle, raw) } == 0 {
        return Err(io::Error::last_os_error());
      }
      Ok(Self { handle, original })
    }
  }

  impl Drop for InputMode {
    fn drop(&mut self) {
      unsafe {
        SetConsoleMode(self.handle, self.original);
      }
    }
  }

  fn read_input(handle: HANDLE, buffer: &mut [u8]) -> io::Result<usize> {
    let mut read = 0;
    if unsafe {
      ReadFile(
        handle,
        buffer.as_mut_ptr(),
        buffer.len() as u32,
        &mut read,
        ptr::null_mut(),
      )
    } == 0
    {
      return Err(io::Error::last_os_error());
    }
    Ok(read as usize)
  }

  fn drain_input(handle: HANDLE, buffer: &mut [u8]) -> io::Result<()> {
    while unsafe { WaitForSingleObject(handle, 0) } == WAIT_OBJECT_0 {
      if read_input(handle, buffer)? == 0 {
        break;
      }
    }
    Ok(())
  }

  pub fn transact(timeout: Duration, send: impl FnOnce() -> io::Result<()>) -> io::Result<Vec<u8>> {
    let input_mode = InputMode::raw()?;
    let mut buffer = [0_u8; 512];
    drain_input(input_mode.handle, &mut buffer)?;
    send()?;
    let deadline = Instant::now() + timeout;
    let mut captured = Vec::new();

    loop {
      let now = Instant::now();
      if now >= deadline {
        break;
      }
      let remaining = deadline
        .saturating_duration_since(now)
        .as_millis()
        .min(u32::MAX as u128);
      let wait = unsafe { WaitForSingleObject(input_mode.handle, remaining as u32) };
      if wait == WAIT_TIMEOUT {
        break;
      }
      if wait != WAIT_OBJECT_0 {
        return Err(io::Error::last_os_error());
      }

      let read = read_input(input_mode.handle, &mut buffer)?;
      captured.extend_from_slice(&buffer[..read]);
      if crate::verify_query_before_secondary_da(&captured).is_ok() {
        break;
      }
    }

    Ok(captured)
  }
}

fn write_frames(stdout: &mut impl Write, frames: &[Vec<u8>]) -> io::Result<()> {
  for frame in frames {
    stdout.write_all(frame)?;
  }
  stdout.flush()
}

fn upload(
  format: PixelFormat,
  image_id: u32,
  cursor_movement: u8,
) -> Result<Vec<Vec<u8>>, Box<dyn std::error::Error>> {
  let width = 128;
  let height = 64;
  let rgba = rgba_test_pattern(width, height);
  let data = match format {
    PixelFormat::Rgb => rgb_test_pattern(width, height),
    PixelFormat::Rgba => rgba,
    PixelFormat::Png => encode_png_rgba(&rgba, width, height)?,
  };
  Ok(encode_direct_upload(
    &data,
    DirectUpload {
      format,
      width,
      height,
      action: TransmitAction::StoreAndDisplay,
      image_id,
      display_columns: 16,
      display_rows: 8,
      z_index: 0,
      cursor_movement,
      quiet: 2,
    },
  ))
}

fn run_kitten(path: &Path) -> io::Result<()> {
  let status = Command::new("kitten")
    .args(["icat", "--transfer-mode=stream"])
    .arg(path)
    .status()?;
  if status.success() {
    Ok(())
  } else {
    Err(io::Error::other(format!(
      "kitten icat exited with {status}"
    )))
  }
}

fn run_query_order(stdout: &mut impl Write) -> Result<String, Box<dyn std::error::Error>> {
  #[cfg(windows)]
  {
    let response = windows_console::transact(Duration::from_secs(2), || {
      stdout.write_all(&query_order_probe())?;
      stdout.flush()
    })?;
    let (kitty_offset, secondary_da_offset) = verify_query_before_secondary_da(&response)
      .map_err(|message| format!("{message}; captured: {}", response.escape_ascii()))?;
    return Ok(format!(
      "query-order passed: Kitty response at byte {kitty_offset}, Secondary DA at byte {secondary_da_offset}"
    ));
  }

  #[cfg(not(windows))]
  {
    let _ = stdout;
    Err("query-order currently supports Windows only".into())
  }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let mut args = std::env::args().skip(1);
  let fixture = args.next();
  let stdout = io::stdout();
  let mut stdout = stdout.lock();

  match fixture.as_deref() {
    Some("yazi-legacy") => {
      let pixels = [
        0xff, 0x20, 0x20, 0xff, 0x20, 0xff, 0x20, 0xff, 0x20, 0x20, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff,
      ];
      for frame in encode_yazi_legacy(&pixels, PixelFormat::Rgba, 2, 2) {
        stdout.write_all(&frame)?;
      }
      stdout.flush()?;
    }
    Some("yazi-delete-all") => {
      stdout.write_all(&yazi_delete_all())?;
      stdout.flush()?;
    }
    Some("rgb") => write_frames(&mut stdout, &upload(PixelFormat::Rgb, 1001, 1)?)?,
    Some("rgba") => write_frames(&mut stdout, &upload(PixelFormat::Rgba, 1002, 1)?)?,
    Some("png-stream") => write_frames(&mut stdout, &upload(PixelFormat::Png, 1003, 1)?)?,
    Some("crop-delete") => {
      let width = 128;
      let height = 64;
      let pixels = rgba_test_pattern(width, height);
      let frames = encode_direct_upload(
        &pixels,
        DirectUpload {
          format: PixelFormat::Rgba,
          width,
          height,
          action: TransmitAction::Store,
          image_id: 1004,
          display_columns: 0,
          display_rows: 0,
          z_index: 0,
          cursor_movement: 1,
          quiet: 2,
        },
      );
      write_frames(&mut stdout, &frames)?;
      stdout.write_all(&encode_placement(Placement {
        image_id: 1004,
        placement_id: 1,
        crop_x: 0,
        crop_y: 0,
        crop_width: width / 2,
        crop_height: height,
        display_columns: 8,
        display_rows: 8,
        z_index: -1,
        cursor_movement: 1,
      }))?;
      stdout.write_all(&encode_placement(Placement {
        image_id: 1004,
        placement_id: 2,
        crop_x: width / 2,
        crop_y: 0,
        crop_width: width / 2,
        crop_height: height,
        display_columns: 8,
        display_rows: 8,
        z_index: -1,
        cursor_movement: 1,
      }))?;
      stdout.flush()?;
      thread::sleep(Duration::from_millis(1200));
      stdout.write_all(&encode_delete_placement(1004, 1))?;
      stdout.flush()?;
      thread::sleep(Duration::from_millis(1200));
      stdout.write_all(&encode_delete_image(1004))?;
      stdout.flush()?;
    }
    Some("cursor-policy") => {
      stdout.write_all(b"C=1 marker:")?;
      write_frames(&mut stdout, &upload(PixelFormat::Rgba, 1005, 1)?)?;
      stdout.write_all(b" cursor stayed here\r\nC=0 marker:\r\n")?;
      write_frames(&mut stdout, &upload(PixelFormat::Rgb, 1006, 0)?)?;
      stdout.write_all(b"cursor moved after image\r\n")?;
      stdout.flush()?;
    }
    Some("kitten-stream") => {
      let path = args.next().ok_or("kitten-stream requires a PNG path")?;
      run_kitten(Path::new(&path))?;
    }
    Some("query-order") => {
      let report_path = args.next();
      match run_query_order(&mut stdout) {
        Ok(result) => {
          if let Some(report_path) = report_path {
            fs::write(report_path, &result)?;
          }
          eprintln!("{result}");
        }
        Err(error) => {
          let result = format!("query-order failed: {error}");
          if let Some(report_path) = report_path {
            fs::write(report_path, &result)?;
          }
          return Err(result.into());
        }
      }
    }
    _ => {
      eprintln!(
        "usage: kitty-protocol-client \
         <yazi-legacy|yazi-delete-all|rgb|rgba|png-stream|crop-delete|cursor-policy|query-order [REPORT]|kitten-stream PNG>"
      );
      std::process::exit(2);
    }
  }

  Ok(())
}
