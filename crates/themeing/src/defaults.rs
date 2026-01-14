use std::sync::Arc;

use config::Palette;
use gpui::SharedString;

pub fn default_settings() -> crate::SettingsStore {
  let settings = crate::SettingsStore {
    active_theme: Arc::new(crate::Theme {
      id: String::from("one dark"),
      name: SharedString::from("One Dark"),
      styles: crate::ThemeStyles {
        colors: Palette::default(),
      },
    }),
  };
  settings
}
