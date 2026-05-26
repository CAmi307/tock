#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MANIFEST_PATH="${ROOT_DIR}/boards/microbit_v2/Cargo.toml"
TARGET="thumbv7em-none-eabi"
TOOLCHAIN="nightly-2024-01-01"
SYSROOT="$(rustc +${TOOLCHAIN} --print sysroot)"
BOARD_DIR="${ROOT_DIR}/boards/microbit_v2"

RUSTFLAGS_JSON='["--cfg","cfg_tock_buildflagssentinel","-C","link-arg=-Tlayout.ld","-C","linker=rust-lld","-C","linker-flavor=ld.lld","-C","relocation-model=static","-C","link-arg=-nmagic","-C","symbol-mangling-version=v0","--remap-path-prefix='"${ROOT_DIR}"'/=","--remap-path-prefix='"${SYSROOT}"'/lib/rustlib/src/rust/library/core=/core/","-C","link-arg=-L'"${BOARD_DIR}"'"]'

RUSTFLAGS_JSON_LESS_OPTIM='["--cfg","cfg_tock_buildflagssentinel","-C","linker=rust-lld","-C","linker-flavor=ld.lld","-C","relocation-model=static","-C","link-arg=-nmagic","-C","symbol-mangling-version=v0","-Z","emit-stack-sizes"]'

exec cargo "+${TOOLCHAIN}" bloat \
  --manifest-path "${MANIFEST_PATH}" \
  --release \
  --target "${TARGET}" \
  -Z build-std=core,compiler_builtins \
  --config "target.${TARGET}.rustflags=${RUSTFLAGS_JSON}" \
  "$@"
