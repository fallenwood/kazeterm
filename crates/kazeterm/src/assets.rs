use anyhow::anyhow;
use gpui::{App, AssetSource, Result, SharedString};
use rust_embed::RustEmbed;
use std::borrow::Cow;

#[derive(RustEmbed)]
// #[folder = "$CARGO_MANIFEST_DIR/../../assets"]
#[folder = "../../assets"]
#[include = "icons/**/*.svg"]
#[include = "fonts/*.ttf"]
#[include = "themes/*.toml"]
pub struct Assets;

impl Assets {
  /// Get a theme file by name from embedded assets
  /// Returns the raw TOML bytes if found
  pub fn get_theme(name: &str) -> Option<Cow<'static, [u8]>> {
    let path = format!("themes/{}.toml", name);
    Self::get(&path).map(|f| f.data)
  }

  /// List all available embedded theme names
  pub fn list_themes() -> Vec<String> {
    Self::iter()
      .filter(|p| p.starts_with("themes/") && p.ends_with(".toml"))
      .map(|p| {
        p.strip_prefix("themes/")
          .unwrap_or(&p)
          .strip_suffix(".toml")
          .unwrap_or(&p)
          .to_string()
      })
      .collect()
  }
}

impl AssetSource for Assets {
  fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
    if path.is_empty() {
      return Ok(None);
    }

    Self::get(path)
      .map(|f| Some(f.data))
      .ok_or_else(|| anyhow!("could not find asset at path \"{path}\""))
  }

  fn list(&self, path: &str) -> Result<Vec<SharedString>> {
    Ok(
      Self::iter()
        .filter_map(|p| p.starts_with(path).then(|| p.into()))
        .collect(),
    )
  }
}

impl Assets {
  pub fn load_fonts(&self, cx: &App) -> anyhow::Result<()> {
    let font_paths = self.list("fonts")?;
    let mut embedded_fonts = Vec::new();
    for font_path in font_paths {
      if font_path.ends_with(".ttf") {
        let font_bytes = cx
          .asset_source()
          .load(&font_path)?
          .expect("Assets should never return None");
        embedded_fonts.push(font_bytes);
      }
    }

    cx.text_system().add_fonts(embedded_fonts)
  }
}

/// Wrapper function for embedded theme loader registration
/// Returns Vec<u8> to match the EmbeddedThemeLoader type signature
pub fn embedded_theme_loader(name: &str) -> Option<Vec<u8>> {
  Assets::get_theme(name).map(|cow| cow.to_vec())
}

/// Wrapper function for embedded theme lister registration
pub fn embedded_theme_lister() -> Vec<String> {
  Assets::list_themes()
}
