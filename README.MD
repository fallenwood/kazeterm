# KazeTerm

## Under development

A lightweight cross-platform terminal app based on Alacritty, inspired by Windows Terminal.

## Screenshot

![screenshot2](./assets/screenshots/screenshot3.webp "Screenshot")

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

## File Paths

### Configuration

| Platform | Path |
|----------|------|
| Windows  | `%APPDATA%\kazeterm\kazeterm.toml` |
| Linux/macOS    | `~/.config/kazeterm/kazeterm.toml` |

### Custom Themes

| Platform | Path |
|----------|------|
| Windows  | `%APPDATA%\kazeterm\themes\` |
| Linux/macOS    | `~/.config/kazeterm/themes/` |

### Desktop Integration (Linux)

On first launch, KazeTerm installs icon and desktop entry files for window manager / compositor icon display:

| File | Path |
|------|------|
| Desktop entry | `~/.local/share/applications/kazeterm.desktop` |
| SVG icon      | `~/.local/share/icons/hicolor/scalable/apps/kazeterm.svg` |
| PNG icons     | `~/.local/share/icons/hicolor/{size}x{size}/apps/kazeterm.png` (256, 128, 64, 48) |

## License

- Some code is derived from Zed; this code inherits Zed's GPL-v3 license.

- The bundled Cascadia Code fonts follow the SIL Open Font License.

- The bundled lucide icons follows original MIT license.

