//! Shared scaffolding for UI tests (headless GPUI).
//!
//! This module is only compiled under `cfg(test)`. It provides a helper
//! [`init_test_app`] that installs the globals Kazeterm components expect
//! (a default [`::config::Config`] plus a [`themeing::SettingsStore`]) inside a
//! [`gpui::TestAppContext`], so component tests can construct views with just
//! a `window` + `cx`.
//!
//! Typical usage:
//!
//! ```ignore
//! #[gpui::test]
//! fn close_confirm_dialog_renders(cx: &mut gpui::TestAppContext) {
//!   crate::test_support::init_test_app(cx);
//!   let window = cx.add_window(|window, cx| {
//!     crate::components::CloseConfirmDialog::new(true, window, cx)
//!   });
//!   cx.run_until_parked();
//!   assert!(window.read(cx).is_ok());
//! }
//! ```
#![cfg(test)]

use ::config::Config;
use gpui::TestAppContext;
use themeing::SettingsStore;

/// Install the globals every Kazeterm view expects: a default [`Config`] and a
/// [`SettingsStore`] built from it, plus `gpui_component::init` so theme
/// lookups resolve.
///
/// Safe to call multiple times per test context; later calls overwrite the
/// globals in-place.
pub fn init_test_app(cx: &mut TestAppContext) {
  cx.update(|cx| {
    // Register the embedded theme loader/lister so `config::load_theme`
    // returns deterministic colors during tests.
    ::config::register_embedded_theme_loader(crate::assets::embedded_theme_loader);
    ::config::register_embedded_theme_lister(crate::assets::embedded_theme_lister);

    gpui_component::init(cx);

    let mut config = Config::default();
    // Disable workspace-restore so `MainWindow::new` always creates a fresh
    // tab instead of trying to rehydrate tabs from the dev's real home dir.
    config.window.restore_workspace = false;
    let settings = crate::config::create_settings_store(&config, /* system_is_dark */ true);

    cx.set_global(settings);
    cx.set_global(config.clone());

    SettingsStore::init_gpui_component_theme(cx);

    // Register terminal keybindings + ZoomState global so TerminalView/Element
    // can paint without panicking during e2e tests.
    terminal::init(cx, &config.keybindings);
  });
}
