# crates/config/src/theme/

## Responsibility

Loads user-selectable theme TOML from custom, embedded, or development assets; chooses light/dark variants; and expands a compact set of theme colors into the complete semantic and terminal `Palette` used by the UI.

## Design

- `mod.rs` defines the persistence schema (`ThemeFile`, optional-heavy `ThemeColors`, and `ThemeMode`), source registration/discovery, hexadecimal color parsing, variant choice, and default fallback.
- `colors.rs` implements `ThemeColors::to_palette`. Invalid or absent required colors fall back field-by-field to the compile-time default theme; accent, border, bright ANSI colors, cursor, overlay, selection, and search colors use explicit fallback rules.
- **Chain of Responsibility:** loading tries the configured custom directory, registered embedded assets, several executable/current-directory development paths, then the compile-time `assets/themes/one.toml` fallback.
- **Dependency injection via function registry:** the main executable registers `EmbeddedThemeLoader` and `EmbeddedThemeLister` once through `OnceLock`, keeping asset packaging out of this crate.
- **Lazy Singleton:** the bundled default TOML is parsed once into `DEFAULT_THEME_FILE`. The mutable custom path is process-wide behind `RwLock<Option<PathBuf>>`.
- **Seed expansion:** `ThemeColors` resolves to `palette::ThemeSeed`; `Palette::from_seed` derives all semantic UI states and bright/dim ANSI variants by color blending, limiting how much each theme file must specify.

## Flow

1. At startup, `crates/kazeterm/src/main.rs` registers embedded asset callbacks and sets the configured or default custom-theme directory.
2. `crates/kazeterm/src/config.rs` resolves `ThemeMode` (including system appearance) and calls `load_theme(name, is_dark)`.
3. `load_theme_from_assets` follows source priority and parses the selected TOML into `ThemeFile`; `list_available_themes` unions custom filenames with embedded names, deduplicates, and sorts them.
4. The requested dark variant uses `ThemeFile::dark`; light uses `light` when present and otherwise falls back to that theme's dark variant.
5. `ThemeColors::to_palette` resolves missing/invalid fields against the appropriate bundled default variant, constructs a seed, and derives a complete `Palette`.
6. If no named theme loads, `load_theme` returns the bundled default name and palette. The application then applies window opacity before placing it in `themeing::SettingsStore`.

## Integration

- Depends on `crate::palette::{Palette, ThemeSeed, blend}` and GPUI `Hsla`/`Rgba` color types.
- Embedded callbacks are supplied by `crates/kazeterm/src/assets.rs`; custom theme paths come from `Config.appearance.themes_path` or the platform config directory.
- `config_watcher` watches the custom theme directory and rebuilds `SettingsStore` when theme TOML changes.
- `alacritty_import` creates a `ThemeFile` compatible with this pipeline and writes it to the custom theme directory.
- Public functions/types are re-exported from `crate::lib`: parsing, listing, loading, source registration, and custom-path access.
