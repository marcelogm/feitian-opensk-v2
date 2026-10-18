#!/usr/bin/env bash
# Flashes a DFU package. The dongle must be in DFU mode (red LED breathing, USB 1915:521f).
# Usage: scripts/20-dfu-flash.sh work/out/dongle-1.zip
set -euo pipefail
. "$(dirname "$0")/common.sh"

zip="${1:?usage: $0 <package.zip>}"
[ -f "$zip" ] || die "$zip not found"
[ "$(usb_mode)" = dfu ] || die "dongle is not in DFU mode (expected USB 1915:521f); tap the button and retry"

abs="$(cd "$(dirname "$zip")" && pwd)/$(basename "$zip")"
case "$abs" in
  "$OUT_DIR"/*) in_path="/out/${abs#"$OUT_DIR"/}" ;;
  "$REPO_DIR"/*) in_path="/repo/${abs#"$REPO_DIR"/}" ;;
  *) die "package must be inside $REPO_DIR" ;;
esac
in_docker nrfutil device program --traits=nordicDfu --firmware="$in_path"
sleep 4
echo "dongle now: $(usb_mode)"
