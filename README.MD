Kaze Term
---

# !! [Under development]

A lightweight cross-platform terminal app based on alacritty, inspired by windows terminal

# Screenshot

![screenshot1](./assets/screenshots/screenshot1.avif "")

# How to build

## Environment Setup

### Windows

msvc target is needed

### Fedora

```
dnf install -y \
  clang \
  mold \
  llvm \
  libxcb-devel \
  libxkbcommon-devel \
  libxkbcommon-x11-devel
```

### Ubuntu

```
apt install -y \
  clang \
  mold \
  llvm \
  libxcb1-dev \
  libxkbcommon-dev \
  libxkbcommon-x11-dev
```

### macOS
No special requirements needed

## Run Build Commands

For debug build, run

```
cargo build
```

For release build, run

```
cargo build --release
```

On windows the debug symbols is store in pdb files, and on linux it's in the binary.
To strip the symbols from binary, run

```
llvm-objcopy --strip-all ./target/release/kazeterm
```

# Testing & Coverage

Run all unit tests:

```
cargo test --workspace
```

Generate coverage (requires `cargo-llvm-cov`):

```
cargo llvm-cov --workspace --all-features --lcov --output-path coverage/lcov.info
```

This produces `coverage/lcov.info`.

# LICENSE

- As some code picked from Zed, the code inhrients Zed's LICENSE of GPL-v3

- The bundled Cascadia Code fonts follows SIL OPEN FONT LICENSE
