# crates/themeing/

## Responsibility

Provides the application's runtime theme model, GPUI global theme state, terminal zoom state, and conversion from terminal ANSI colors to GPUI colors.

## Design

- `Theme` and `ThemeStyles` wrap the `config::Palette` used by the application and terminal renderer.
- `SettingsStore` is a GPUI `Global`; `ActiveTheme` gives GPUI `App` callers uniform access to the active theme.
- `ZoomState` is a bounded GPUI global for terminal font scaling.
- `AnsiColor`, `AnsiNamedColor`, and `AnsiRgb` form a backend-neutral color vocabulary. Conversion functions implement an Adapter from named, indexed, and RGB terminal colors to `gpui::Hsla`.
- `defaults.rs` constructs the default `SettingsStore`; `init_gpui_component_theme` mirrors Kazeterm colors and fonts into `gpui_component::Theme`.

## Flow

1. The application builds a `SettingsStore` from `config::Palette` and installs it as a GPUI global.
2. UI components read the store through `ActiveTheme`; GPUI Component controls receive equivalent colors and fonts through `init_gpui_component_theme`.
3. Terminal rendering passes ANSI colors through `convert_color`; indexed colors resolve against the active palette or the xterm color cube.
4. Zoom actions update `ZoomState`, and terminal layout derives an effective font size from its multiplier.

## Integration

- Depends on: `config` for palettes and font configuration; `gpui` and `gpui-component` for global state and widget styling.
- Consumed by: `kazeterm` UI components, `terminal` rendering and zoom behavior, and `terminal-kernel` color adapters.
- Entry points: `src/lib.rs` for public models/converters/globals and `src/defaults.rs` for default theme construction.
