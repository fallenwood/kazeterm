# crates/config/

## Responsibility

Provides Kazeterm's configuration domain crate. It owns the serializable application settings, versioned TOML loading and migration pipeline, imported-config overlay semantics, global transition-animation policy, theme and palette resolution, shell/profile discovery, keyboard-shortcut parsing, and Alacritty import conversion.

## Design

- `Cargo.toml` defines a Rust 2024 library crate. `serde`/`toml` form the persistence boundary, `dirs` locates platform configuration roots, `gpui` supplies color types and the `Global` marker, `which` supports shell discovery, and `tracing` reports recoverable load/discovery failures.
- The public API is a facade rooted at `src/lib.rs`; implementation modules remain private unless consumers need a complete subsystem (`palette`, `migration`, and `alacritty_import`).
- Typed settings use `#[serde(default)]` plus explicit `Default` implementations so missing fields remain backward-compatible. Runtime getters clamp user-controlled numeric values before UI, animation, or terminal consumers use them; animation frame counts are additionally capped to prevent runaway repaint loops.
- Raw TOML is deliberately retained until migrations and recursive overlays finish. Only then is it deserialized into `Config`, which prevents historical layouts from leaking into runtime code.
- Platform differences are compile-time strategies (`cfg` branches) for paths, fonts, shell candidates, kernel availability, and modifier labels.

## Flow

1. `Config::load` locates or creates `kazeterm.toml` under the platform config directory.
2. The base file is parsed as `toml::Value`, upgraded by the ordered migration registry, and recursively merged with `imports`; later overlays win, while `version` and `imports` never overwrite the base metadata.
3. The merged value becomes the typed `Config`; transient container profiles are rediscovered and platform-dependent settings are validated.
4. When the base file migrated, the original text is backed up and the migrated base TOML is rewritten. Imported files are merge inputs and are not rewritten by this path.
5. The application installs `Config` as a GPUI global. Hot reload repeats the load pipeline, rebinds keys, recreates theme state, and reconfigures watched import/theme paths.

## Integration

- Detailed source map: [src/codemap.md](src/codemap.md)
- Primary consumer: `crates/kazeterm`, which loads and globally publishes `Config`, watches all resolved config files, initializes embedded/custom themes, builds windows, constructs transition schedules from `Config.animation`, and persists updater/import state.
- Terminal consumer: `crates/terminal`, which converts `KeybindingConfig` into GPUI bindings and reads terminal, cursor, font, color, and behavior settings through the main application.
- Theme consumer: `crates/themeing`, whose `SettingsStore` receives the resolved `Palette` from `crates/kazeterm/src/config.rs`.
- External data boundaries: the user's TOML files, custom and embedded theme TOML, `~/.ssh/config`, the host executable/PATH, container-runtime command output, and optional Alacritty TOML.
