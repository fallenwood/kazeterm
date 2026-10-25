# Workspace architecture

Kazeterm separates the desktop composition root from reusable libraries and
headless terminal backends. Package names remain stable even where directories
have moved, so existing `cargo ... -p <package>` commands still work.

## Directory and package map

| Directory | Cargo package | Responsibility |
| --- | --- | --- |
| `apps/kazeterm` | `kazeterm` | Desktop executable, windows, UI composition, services, and platform integration |
| `crates/config` | `config` | Configuration schema, loading, validation, migrations, profiles, and palettes |
| `crates/themeing` | `themeing` | GPUI theme and zoom globals |
| `crates/kazeterm-event-system` | `kazeterm-event-system` | JSON event ingress and GPUI main-thread dispatch |
| `crates/kazeterm-ui-tree` | `kazeterm-ui-tree` | Serializable UI state, actions, reducer, and diffs |
| `crates/terminal/core` | `terminal-kernel` | Backend port, session resources/options, PTY process information, and protocols |
| `crates/terminal/view` | `terminal` | GPUI terminal model, interaction, image decoding/storage, and painting |
| `crates/terminal/backends/alacritty` | `terminal-kernel-alacritty` | Cross-platform Alacritty session factory |
| `crates/terminal/backends/vte` | `terminal-kernel-vte` | Linux-only VTE emulator and session factory |

`assets/` remains at repository root. The app's embedded fonts, icons, Windows
resources, and macOS bundle metadata retain their existing paths relative to the
app manifest (`../../assets`). Build outputs remain in root `target/`.

## Dependency direction

```text
kazeterm (composition root)
  ├── config
  ├── themeing ──> config
  ├── kazeterm-event-system
  ├── kazeterm-ui-tree
  ├── terminal (GPUI view) ──> config, themeing, terminal-kernel
  ├── terminal-kernel-alacritty ──> terminal-kernel
  └── terminal-kernel-vte [Linux] ──> terminal-kernel
```

- Core and backends must not depend on the terminal view, application config,
  theme bridge, or GPUI. They can compile and run protocol/emulator tests without
  compiling the desktop framework.
- `terminal-kernel` still uses Alacritty's grid, event, PTY, and ANSI types as the
  shared terminal vocabulary, and contains its existing `AlacrittyBackend`
  adapter. This refactor does not replace those types with a new emulator model.
- The GPUI view owns interaction and presentation state, not session creation.
- Only the executable selects backends and translates persisted configuration.
- `config` still uses GPUI color types, and the event-system crate still integrates
  with GPUI dispatch. They are not claimed to be headless domain-only crates.
- Declare internal paths once in root `[workspace.dependencies]`; member
  manifests use `workspace = true` instead of relative dependency paths.

## Session creation and ownership

1. The application resolves a shell/profile and translates `config::Config` into
   `terminal_kernel::SessionOptions`: initial size, bounded scrollback, cursor
   style, OSC 52 policy, and environment overrides.
2. A selected backend creates the PTY, emulator, output observers, and event loop.
   It returns `(TerminalSession, SessionEvents)` without constructing GPUI state.
3. `TerminalSession` owns the backend and input/resize ports, process metadata,
   shared keyboard/CNL atomics, graphics/CWD receivers, prompt file, and program
   status tracker.
4. The app converts that resource bundle with `terminal::Terminal::from(session)`
   and creates a `TerminalView`. Conversion moves the existing resources; it
   never opens a second PTY or restarts a process.
5. Existing event batching, subscriptions, cross-window terminal transfers,
   UI-tree reconciliation, and workspace persistence remain unchanged.

The core owns OSC 7/7501 observation, keyboard/DSR negotiation, and raw graphics
interception. The view retains Kitty command decoding, GPU image storage,
placements, and painting. Existing protocol/PTY exports under `terminal` remain
available as re-exports for application code and tests.

## Platform boundaries

Alacritty remains the default backend on Windows, macOS, and Linux. VTE is
selectable and compiled only on Linux. Both its source and manifest dependencies
are target-gated, so other platforms build an empty VTE crate during workspace
checks. The old `terminal-kernel-unix-bundle` is no longer needed.

Root `cargo build` and `cargo run` target the desktop app and its dependencies;
`cargo build --workspace` and `cargo test --workspace` cover all packages on every
supported platform without excluding VTE manually.

## Validation and enforcement

```sh
pwsh -NoProfile -File scripts/check-workspace.ps1
pwsh -NoProfile -File scripts/test-workspace.ps1
cargo test -p terminal-kernel -p terminal-kernel-alacritty -p terminal-kernel-vte
cargo fmt --all -- --check
cargo check --workspace --locked
cargo test --workspace --locked
cargo build --workspace --locked
```

The PowerShell 7 boundary checker reads Cargo metadata, including target-specific
and test edges, rejects forbidden workspace dependencies and GUI dependencies in
headless packages, and requires centralized internal paths. Its regression tests
need no external PowerShell modules. Both checks run in PR and release CI. New
crates must declare their allowed dependency boundary in the checker.
