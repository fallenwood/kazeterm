# crates/

## Responsibility

Reusable workspace libraries. The executable composition root lives separately
in `apps/kazeterm`; package names remain stable after directory reorganization.

## Dependency boundaries

- `config` owns persisted/runtime settings and palette data.
- `themeing -> config` adapts settings into GPUI globals.
- `kazeterm-ui-tree` owns serializable state and reducer/diff contracts.
- `kazeterm-event-system` supplies external ingress and GPUI dispatch.
- Terminal core and backends remain headless; only the terminal view depends on
  config, theming, and GPUI.
- Root `[workspace.dependencies]` owns all internal paths.

See [architecture](../docs/architecture.md) and run
`pwsh -NoProfile -File scripts/check-workspace.ps1` to check these rules.

## Directory map

| Directory | Package / responsibility | Detailed map |
| --- | --- | --- |
| `config` | `config`: configuration schema, loading, migration, profiles, palettes | [Map](config/codemap.md) |
| `themeing` | `themeing`: GPUI theme and zoom state | [Map](themeing/codemap.md) |
| `kazeterm-ui-tree` | Serializable UI aggregate, reducer, diffs | [Map](kazeterm-ui-tree/codemap.md) |
| `kazeterm-event-system` | Event protocol, ingress, GPUI dispatch | [Map](kazeterm-event-system/codemap.md) |
| `terminal/core` | `terminal-kernel`: session/backend ports and PTY/protocol support | [Map](terminal/core/codemap.md) |
| `terminal/view` | `terminal`: GPUI presentation and input | [Map](terminal/view/codemap.md) |
| `terminal/backends/alacritty` | `terminal-kernel-alacritty`: cross-platform factory | [Map](terminal/backends/alacritty/codemap.md) |
| `terminal/backends/vte` | `terminal-kernel-vte`: Linux-only factory/emulator | [Map](terminal/backends/vte/codemap.md) |
