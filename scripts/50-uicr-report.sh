#!/usr/bin/env bash
# OPTIONAL diagnostics (only while the reset pin still works, i.e. before 40-platform-update.sh):
# builds the uicr-fix firmware and flashes it over DFU. It reports UICR, RESETREAS and the
# P1.06/P0.18 levels over USB serial (/dev/serial/by-id/usb-1209_uicr-fix-if00).
# It replaces OpenSK: tap the button to get back to DFU and re-run 20-dfu-flash.sh afterwards.
#
# Usage: scripts/50-uicr-report.sh build|flash|read|dump
#   dump  saves the Nordic bootloader (0xE0000..0x100000) to work/out/bootloader.txt (read-only)
set -euo pipefail
. "$(dirname "$0")/common.sh"

tty=/dev/serial/by-id/usb-1209_uicr-fix-if00
case "${1:-}" in
  build) in_docker bash -c 'cd /repo/uicr-fix && ./build.sh && cp out/uicr-fix.zip /out/' ;;
  flash) "$(dirname "$0")/20-dfu-flash.sh" "$OUT_DIR/uicr-fix.zip" ;;
  read)
    [ -e "$tty" ] || die "uicr-fix is not running"
    in_docker bash -c "stty -F $tty raw -echo; timeout 3 cat $tty" | tr -d '\r' | tail -25 ;;
  dump)
    [ -e "$tty" ] || die "uicr-fix is not running"
    in_docker bash -c "stty -F $tty raw -echo; timeout 25 cat $tty > /out/bootloader.raw & sleep 2; printf 'DUMP\r\n' > $tty; wait" || true
    tr -d '\r' < "$OUT_DIR/bootloader.raw" > "$OUT_DIR/bootloader.txt"
    echo "lines: $(grep -c '^D ' "$OUT_DIR/bootloader.txt") (expected 4096)"; grep 'DUMP FIM' "$OUT_DIR/bootloader.txt" ;;
  *) die "usage: $0 build|flash|read|dump" ;;
esac
