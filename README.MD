# KazeTerm

## Under development

A lightweight cross-platform terminal app based on Alacritty, inspired by Windows Terminal.

## Screenshot

![screenshot](./assets/screenshots/Screenshot_2026-03-14.webp "Screenshot")

## Documentation
More details can be found at [wiki](https://github.com/fallenwood/kazeterm/wiki "")

## Config Overlays

Kazeterm loads `kazeterm.toml` as the base config and can merge extra TOML files listed in `imports`.
Imported files have higher priority than the base file, and later imports override earlier ones.

```toml
imports = ["~/kazeterm.windows.toml"]
```

Relative import paths are resolved from the file that declares them, and nested imports are supported.

### Terminal kernel

Terminal backends are selected through the terminal kernel abstraction:

```toml
[terminal]
kernel = "alacritty"
```

Supported values are `alacritty` (default), `ghostty`, and `vte`. Unsupported kernels fail explicitly instead of silently falling back.

### Key debug overlay

Enable the bottom-right shortcut inspector with:

```toml
[window]
key_debug_mode = true
```

## How to Build

### Environment Setup

#### Windows

MSVC target is required for release build.

#### Fedora

```
dnf install -y \
  clang \
  llvm \
  libxcb-devel \
  libxkbcommon-devel \
  libxkbcommon-x11-devel
```

#### Ubuntu

```
apt install -y \
  clang \
  llvm \
  libxcb1-dev \
  libxkbcommon-dev \
  libxkbcommon-x11-dev
```

#### macOS

XCode is needed for metal sdk

```
xcode-select --switch /Applications/Xcode.app/Contents/Developer
xcodebuild -runFirstLaunch
xcodebuild -downloadComponent MetalToolchain
```

### Run Build Commands

For debug build, run:

```
cargo build
```

For release build, run:

```
cargo build --release
```

On Windows, debug symbols are stored in PDB files, and on Linux they are in the binary.
To strip symbols from the binary, run:

```
llvm-objcopy --strip-all ./target/release/kazeterm
```

## Testing and Coverage

Run all unit tests:

```
cargo test --workspace
```

Generate coverage (requires `cargo-llvm-cov`):

```
cargo llvm-cov --workspace --all-features --lcov --output-path coverage/lcov.info
```

This produces `coverage/lcov.info`.

## License

- Some code is derived from Zed; this code inherits Zed's GPL-v3 license.

- The bundled Cascadia Code fonts follow the SIL Open Font License.

- The bundled lucide icons follows original MIT license.

