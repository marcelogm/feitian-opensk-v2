#!/usr/bin/env bash
# Builds the two DFU packages of a STOCK OpenSK (no Feitian features) for the first install:
#   work/out/dongle-1.zip  Wasefire bootloader + platform (side A)
#   work/out/dongle-2.zip  Wasefire bootloader alone (lets the Nordic bootloader ignore platform updates)
# --sd-req=0 lets the package overwrite a SoftDevice left by other firmware (e.g. "nRF52 Connectivity").
set -euo pipefail
. "$(dirname "$0")/common.sh"

# `flash --artifacts` builds the hex files and stops at 3 prompts; we answer them with ENTER.
printf '\n\n\n' | in_docker bash -c 'cd third_party/wasefire && "$@"' _ "${XTASK[@]}" flash --artifacts

in_docker bash -c '
  set -e
  cd third_party/wasefire/target/wasefire
  for n in 1:platform 2:bootloader; do
    nrfutil nrf5sdk-tools pkg generate --hw-version=52 --sd-req=0 --application-version=0 \
      --application=${n#*:}.hex /out/dongle-${n%%:*}.zip >/dev/null
  done'
ls -l "$OUT_DIR"/dongle-1.zip "$OUT_DIR"/dongle-2.zip
