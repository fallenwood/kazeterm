# crates/

## Responsibility

Contains the Rust workspace's application, configuration, UI-state, presentation, theming, and terminal-emulation layers. The crates separate platform-agnostic contracts from GPUI presentation and concrete terminal backends.

## Design

The workspace is organized around explicit dependency boundaries:

- `config` owns persisted/runtime settings and palette data.
- `themeing` adapts configuration colors into GPUI globals and ANSI color resolution.
- `terminal-kernel` defines the object-safe emulator port and shared terminal type vocabulary.
- `terminal` is the GPUI-facing terminal model, interaction, and renderer layer.
- `terminal-kernel-alacritty` and `terminal-kernel-vte` assemble concrete PTY/emulator sessions behind the shared ports.
- `kazeterm-ui-tree` models UI state with reducer actions and structural diffs.
- `kazeterm-event-system` carries internal and external commands onto the GPUI main thread.
- `kazeterm` is the executable composition root.
- `terminal-kernel-unix-bundle` makes the Unix-only VTE backend participate in default Unix workspace builds without breaking Windows builds.

## Flow

1. `kazeterm` loads `config` and installs `themeing` globals.
2. The application selects a terminal kernel, whose session factory creates a PTY, a backend implementation, a `terminal::Terminal`, and an event receiver.
3. `terminal` translates GPUI input into PTY messages and converts backend snapshots into GPU-rendered content.
4. UI commands mutate `kazeterm-ui-tree`; tree diffs are reconciled into concrete GPUI entities.
5. `kazeterm-event-system` normalizes programmatic, stdin, or socket commands and dispatches them through the application event bus.

## Integration

| Crate | Responsibility | Detailed map |
| --- | --- | --- |
| `config` | Configuration schema, loading, migration, and theme palette data. | [Map](config/codemap.md) |
| `kazeterm` | Executable composition root and GPUI application UI. | [Map](kazeterm/codemap.md) |
| `kazeterm-event-system` | Thread-safe event ingress and main-thread GPUI dispatch. | [Map](kazeterm-event-system/codemap.md) |
| `kazeterm-ui-tree` | Serializable UI state, reducer actions, and reconciliation diffs. | [Map](kazeterm-ui-tree/codemap.md) |
| `themeing` | Active theme globals and terminal color conversion. | [Map](themeing/codemap.md) |
| `terminal` | GPUI terminal model, interaction handling, and rendering. | [Map](terminal/codemap.md) |
| `terminal-kernel` | Backend-neutral terminal contracts and shared emulator types. | [Map](terminal-kernel/codemap.md) |
| `terminal-kernel-alacritty` | Alacritty-backed PTY session assembly. | [Map](terminal-kernel-alacritty/codemap.md) |
| `terminal-kernel-vte` | Custom VTE-parser-backed PTY session assembly. | [Map](terminal-kernel-vte/codemap.md) |
| `terminal-kernel-unix-bundle` | Unix-only backend build aggregation. | [Map](terminal-kernel-unix-bundle/codemap.md) |
