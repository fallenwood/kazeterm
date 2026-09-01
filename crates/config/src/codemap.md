# crates/config/src/

## Responsibility

Implements the configuration crate's typed model and all adapters that translate filesystem, platform, keyboard, theme, shell, and foreign-config data into values consumed by Kazeterm.

## Design

| Source | Responsibility |
|---|---|
| `lib.rs` | Public facade and aggregate `Config`; defaults, animation policy/easing, validation, config-path discovery, default-file generation, migrations, recursive imports, typed deserialization, migration backups, updater-state mutations, and GPUI global registration. |
| `keybinding.rs` | Bidirectional key-first/legacy TOML adapter, action registry, platform defaults, override merging, normalized parsing/matching, and display labels. |
| `profiles.rs` | Serializable local shell profiles plus transient detected container profiles and `Config` lookup helpers. |
| `shell.rs` | Platform strategy for ordered shell discovery, Visual Studio developer shells, and Docker/Podman/Distrobox container commands. |
| `ssh.rs` | Read-only parser for concrete `Host` entries in `~/.ssh/config`; wildcard and negated patterns are excluded. |
| `palette.rs` | Complete semantic/terminal `Palette` and deterministic derivation of UI, bright, and dim variants from a small theme seed. |
| `alacritty_import.rs` | Anti-corruption layer that parses a supported subset of Alacritty TOML into a Kazeterm config patch and optional custom theme. |
| `theme/` | Theme parsing, discovery, fallback resolution, and `ThemeColors` to `Palette` conversion. See [theme/codemap.md](theme/codemap.md). |
| `migration/` | Ordered raw-TOML schema upgrades. See [migration/codemap.md](migration/codemap.md). |

Key architectural patterns:

- **Facade/Aggregate:** `lib.rs` re-exports stable types and functions while `Config` groups color, appearance, animation, font, window, tab, pane, terminal, cursor, notification, update, profile, and keybinding sections.
- **Defaulted configuration objects:** every nested settings object supplies defaults; computed accessors normalize tab widths, opacity, divider width, scrollback, and timing values at use time. `AnimationConfig` defaults to enabled, 180 ms total duration, 15 ms requested frame interval, `ease_in_out`, and 0.82 fade-start opacity.
- **Bounded transition policy:** `AnimationEasing::apply` clamps progress to `[0, 1]` and selects linear, quadratic ease-in/ease-out, or GPUI ease-in-out interpolation. `AnimationConfig` treats a disabled switch or zero duration as immediate, caps duration at 5 seconds, clamps frame intervals to 4–1000 ms, caps schedules at 600 frames, derives an exact per-frame duration, and clamps finite fade opacity while replacing non-finite values with the default.
- **Pipeline:** filesystem text → raw TOML → migrations → recursive overlays → typed `Config` → platform validation/runtime augmentation.
- **Specialized merge strategy:** ordinary tables deep-merge, scalar/array overlays replace, and keybindings are canonicalized into `binding -> action` before merging so both current and legacy formats compose correctly.
- **Runtime augmentation:** `container_profiles` is skipped by Serde and repopulated after load; theme source functions and custom theme path are process-wide registries.

## Flow

### Configuration lifecycle

1. `Config::load` creates a default file when absent, then delegates to `load_from_path`.
2. `read_raw_config_with_content` preserves the original bytes for backup and parses a mutable `toml::Value`.
3. `migration::apply_migrations` mutates only the base document. `apply_imports` resolves relative/absolute/home paths, tracks canonicalized paths to stop duplicate cycles, logs unreadable imports, and overlays nested imports recursively.
4. `try_into::<Config>` applies Serde defaults. Shell/container discovery supplies dynamic profiles, and terminal-kernel validation rejects unsupported platform selections.
5. A successful migration creates a collision-resistant timestamped backup before rewriting the upgraded base file. Failures to back up prevent the rewrite but still return the usable in-memory config.

### Runtime consumers

- `Config::get_config_file_paths` traverses the same import graph for `config_watcher`; it includes discovered paths even when an imported file cannot currently be read.
- Updater setters migrate and edit only `[auto_update]`, preserving unrelated raw TOML. The one-shot workspace flag is read and atomically cleared through the same path.
- `crates/kazeterm/src/components/transitions.rs` converts `Config.animation` into an optional `TransitionSpec`; active schedules drive eased geometry/opacity frames, while disabled or zero-duration settings cause callers to apply final state immediately.
- `KeybindingList` normalizes and deduplicates strings, `ParsedKeybinding` maps modifiers/key aliases to GPUI event semantics, and application/terminal layers call `matches` or register each binding.
- Profile helpers choose the configured default, fall back to detected shells, and combine local, container, and SSH names for UI selection.
- Alacritty import parses into a sparse patch; the main-app dialog saves any generated theme, applies the patch to the global config, and persists the resulting file.

## Integration

- `crates/kazeterm/src/main.rs`: calls `Config::load`, registers embedded theme callbacks, configures custom theme paths, and publishes configuration globals.
- `crates/kazeterm/src/config_watcher.rs`: watches the base file, recursive imports, and custom theme directory; reloads configuration/theme and rebinds terminal keys.
- `crates/kazeterm/src/components/`: consumes window/tab/pane/profile/keybinding fields, uses `AnimationConfig` for tab-bar geometry and broad UI fade transitions, launches the Alacritty import workflow, and uses the config directory for workspace/support files.
- `crates/terminal/src/lib.rs`: registers configurable terminal keybindings; terminal views consume color/font/cursor/scrollback/input fields supplied by the app.
- `crates/kazeterm/src/config.rs` and `crates/themeing`: translate `ColorsConfig`/`AppearanceConfig` plus system appearance into a `SettingsStore` containing the resolved palette.
- Filesystem and process integrations are intentionally best-effort where appropriate: missing imports/themes/shell tools are logged or skipped, while malformed base TOML and unsupported terminal kernels fail `Config::load`.
