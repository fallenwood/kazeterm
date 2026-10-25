# apps/

## Responsibility

Executable composition roots, separate from reusable workspace libraries.

## Directory map

- [`kazeterm/`](kazeterm/codemap.md) contains the desktop executable. It selects
  terminal backends, translates configuration into headless session options,
  assembles GPUI windows, and owns application services and platform integration.

## Integration

Apps may depend on workspace libraries; libraries must not depend on apps.
Root `cargo build`/`cargo run` select `kazeterm`. Build outputs remain in the
workspace `target/` directory.
