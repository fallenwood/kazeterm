# Cross-architecture CI builds

- Replaced the native Windows x64 and macOS Intel runners with x64 targets built on the existing Windows ARM64 and macOS Apple Silicon runners.
- Made each CI/PR matrix entry carry an explicit Rust target and artifact architecture.
- Added target installation, target-specific Cargo build/coverage/bundle commands, cache keys, output paths, and release package names.
- Select Windows x64 coverage explicitly with `matrix.arch == 'X64'`.

Validation: parsed both workflow YAML files with `Bun.YAML.parse` and ran `git diff --check`.
