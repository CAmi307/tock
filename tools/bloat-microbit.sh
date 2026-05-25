#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MANIFEST_PATH="${ROOT_DIR}/boards/microbit_v2/Cargo.toml"
TARGET="thumbv7em-none-eabi"
TOOLCHAIN="nightly-2025-11-03"

RUSTFLAGS_JSON='["--cfg","cfg_tock_buildflagssentinel","-C","linker=rust-lld","-C","linker-flavor=ld.lld","-C","relocation-model=static","-C","link-arg=-nmagic","-C","link-arg=-icf=all","-C","symbol-mangling-version=v0","-C","lto","-Z","emit-stack-sizes"]'

RUSTFLAGS_JSON_LESS_OPTIM='["--cfg","cfg_tock_buildflagssentinel","-C","linker=rust-lld","-C","linker-flavor=ld.lld","-C","relocation-model=static","-C","link-arg=-nmagic","-C","symbol-mangling-version=v0","-Z","emit-stack-sizes"]'


exec cargo "+${TOOLCHAIN}" bloat \
  --manifest-path "${MANIFEST_PATH}" \
  --release \
  --target "${TARGET}" \
  -Z build-std=core,compiler_builtins \
  -Z build-std-features=core/optimize_for_size \
  --config "target.${TARGET}.rustflags=${RUSTFLAGS_JSON_LESS_OPTIM}" \
  "$@"
