#!/usr/bin/env bash
# IRREVERSIBLE on the first run: installs the patched Wasefire runner through Wasefire's own
# USB platform update (no DFU). On its first boot the runner writes UICR.PSELRESET[1]=0x10,
# which permanently disables the reset pin (the button) — and with it the only way into DFU.
#
# Each run writes the OTHER A/B slot. Run it, replug, run 30-touch-test.sh, then run it once
# more so both slots carry the patched runner.
set -euo pipefail
. "$(dirname "$0")/common.sh"

[ "$(usb_mode)" = wasefire ] || die "OpenSK is not running (expected USB 18d1:0239)"
git -C "$WASEFIRE_DIR" apply --reverse --check "$PATCH" 2>/dev/null \
  || die "Feitian patch is not applied; run scripts/00-setup.sh"

if [ "${YES:-}" != 1 ]; then
  cat <<'EOF'
After this update the dongle can NEVER enter DFU again without an SWD probe.
Future updates only through this script (Wasefire platform update).
Type "yes" to continue:
EOF
  read -r answer
  [ "$answer" = yes ] || die "aborted"
fi

in_docker_root bash -c 'cd third_party/wasefire && "$@"' _ "${XTASK[@]}" "${FEITIAN_FEATURES[@]}" update
sleep 4
echo "dongle now: $(usb_mode). Replug it, then run scripts/30-touch-test.sh"
