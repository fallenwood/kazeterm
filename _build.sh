#! /usr/bin/env sh

cargo build --release
llvm-objcopy --strip-all target/release/kazeterm
