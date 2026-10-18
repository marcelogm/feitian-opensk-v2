#!/usr/bin/env bash
# Runs inside the container. Produces out/uicr-fix.hex and out/uicr-fix.zip (DFU package).
set -euo pipefail
cd "$(dirname "$0")"
BIN=/work/third_party/wasefire/.root/bin
cargo build --release --bin uicr-fix
mkdir -p out
$BIN/rust-objcopy -O ihex target/thumbv7em-none-eabi/release/uicr-fix out/uicr-fix.hex
$BIN/rust-size target/thumbv7em-none-eabi/release/uicr-fix
rm -f out/uicr-fix.zip
nrfutil nrf5sdk-tools pkg generate --hw-version=52 --sd-req=0 --application-version=0 --application=out/uicr-fix.hex out/uicr-fix.zip
