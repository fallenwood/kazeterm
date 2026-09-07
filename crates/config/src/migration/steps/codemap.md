# crates/config/src/migration/steps/

## Responsibility

Contains the adjacent, chronological raw-TOML transformations used by the migration registry. Each module owns one historical schema transition and always stamps the destination version when the document root is a table.

## Design

- `mod.rs` declares every step module and re-exports its migration function to the parent registry.
- Steps are intentionally small and synchronous. They use table mutation, `entry`, `contains_key`, and conditional removal/rename to retain existing user choices while supplying newly required structure.
- Keybinding migrations consult platform-specific `KeybindingConfig::default` values and, at the format boundary, delegate to `keybinding::rewrite_keybinding_table_to_key_first`.
- The large `20260412.3 → 20260414.1` step is the schema pivot from flat root keys to typed nested sections; later steps operate on those sections.

| Transition | Transformation |
|---|---|
| `0 → 20260208.1` | Introduces the version field. |
| `20260208.1 → 20260220.1` | Adds flat `vertical_tabs = false`. |
| `20260220.1 → 20260303.1` | Introduces keybinding support through defaults; version-only mutation. |
| `20260303.1 → 20260306.1` | Adds flat `background_opacity = 1.0`. |
| `20260306.1 → 20260322.1` | Introduces pane-navigation bindings through defaults; version-only mutation. |
| `20260322.1 → 20260323.1` | Adds flat scrollback, cursor, OSC 52, and copy-on-select defaults. |
| `20260323.1 → 20260323.2` | Introduces `toggle_tab_bar` through defaults; version-only mutation. |
| `20260323.2 → 20260327.1` | Adds flat `background_blur = false`. |
| `20260327.1 → 20260407.1` | Adds flat right-click context-menu setting and removes obsolete `right_click`. |
| `20260407.1 → 20260411.1` | Adds default new-tab and profile-specific bindings to an existing keybinding table. |
| `20260411.1 → 20260411.2` | Adds flat startup-maximized setting. |
| `20260411.2 → 20260411.3` | Adds flat split-divider width. |
| `20260411.3 → 20260412.1` | Adds flat inactive-pane opacity. |
| `20260412.1 → 20260412.2` | Adds root `imports = []`. |
| `20260412.2 → 20260412.3` | Adds flat tab-title debounce interval. |
| `20260412.3 → 20260414.1` | Moves/renames flat keys into `[appearance]`, `[font]`, `[window]`, `[tab]`, `[pane]`, `[terminal]`, `[cursor]`, and `[notification]`. |
| `20260414.1 → 20260414.2` | Moves theme selection from `[appearance]` and bold/contrast controls from `[terminal]` into `[colors]`. |
| `20260414.2 → 20260415.1` | Adds `[terminal].ctrl_scroll_zoom = true`. |
| `20260415.1 → 20260415.2` | Adds pixel-based tab label min/max widths. |
| `20260415.2 → 20260415.3` | Adds terminal mouse-hiding setting. |
| `20260415.3 → 20260416.1` | Introduces optional character-based tab widths; preserves pixel fallbacks and only advances version. |
| `20260416.1 → 20260416.2` | Repairs incorrectly inserted macOS tab shortcuts and adds `window.key_debug_mode`. |
| `20260416.2 → 20260416.3` | Adds direct tab-selection bindings to an existing keybinding table. |
| `20260416.3 → 20260417.1` | Adds directional pane-focus bindings to an existing keybinding table. |
| `20260417.1 → 20260417.2` | Adds terminal focus-on-hover. |
| `20260417.2 → 20260417.3` | Adds terminal-kernel selection with `alacritty` default. |
| `20260417.3 → 20260419.1` | Canonicalizes keybindings from legacy action-first entries to `key = action`. |
| `20260419.1 → 20260421.1` | Renames `select_tab_9` to `select_last_tab` in either keybinding representation. |
| `20260421.1 → 20260422.1` | Adds the default `toggle_hidden_panes` binding in key-first form. |
| `20260422.1 → 20260512.1` | Creates/populates `[auto_update]` with `check = "never"` and the one-shot restore flag. |
| `20260512.1 → 20260901.1` | Creates/populates `[animation]` without overwriting existing values: `enabled = true`, `duration_ms = 180`, `frame_interval_ms = 15`, `easing = "ease_in_out"`, and `fade_start_opacity =1.0`. |
| `20260901.1 → 20260905.1` | Removes obsolete `animation.fade_start_opacity`; transitions now animate geometry only. |

## Flow

1. The parent dispatcher determines the document's starting step from its raw `version`.
2. It invokes each exported function in registry order; each function mutates the same `toml::Value` in place.
3. Default-insertion steps leave an existing key untouched, structural steps move owned values into the current section layout, and compatibility steps normalize legacy encodings.
4. Each successful table-root step writes its `to_version`, enabling the next adjacent transformation and leaving the final document at `CURRENT_CONFIG_VERSION`.

## Integration

- Functions are crate-private and consumed only by `migration/mod.rs` through `use steps::*`.
- `migrate_v20260407_1_to_20260411_1`, the macOS shortcut repair, and later binding-addition steps depend on `crate::KeybindingConfig::default()`.
- `migrate_v20260417_3_to_20260419_1` depends on the keybinding module's legacy/current parser and canonical rewrite helper.
- These transformations must stay aligned with `Config`'s Serde field names/defaults and generated TOML layout; otherwise the post-migration `try_into::<Config>` boundary fails or silently falls back.
