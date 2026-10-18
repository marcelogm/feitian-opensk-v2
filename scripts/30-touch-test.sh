#!/usr/bin/env bash
# Asks the running OpenSK for a credential (CTAP2 makeCredential), which requires a touch.
# Tap the button when the LED blinks green. Prints "OK: touch recognized ..." on success.
set -euo pipefail
. "$(dirname "$0")/common.sh"

[ "$(usb_mode)" = wasefire ] || die "OpenSK is not running (expected USB 18d1:0239)"
in_docker_root python3 -u /repo/tools/touch_test.py
