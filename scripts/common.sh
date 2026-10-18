# Shared configuration for all scripts. Source it, don't run it.
# shellcheck shell=bash

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK_DIR="${WORK_DIR:-$REPO_DIR/work}"
OPENSK_DIR="$WORK_DIR/opensk"
WASEFIRE_DIR="$OPENSK_DIR/third_party/wasefire"
OUT_DIR="$WORK_DIR/out"
IMAGE="${IMAGE:-opensk-build}"

# Versions this procedure was validated with (2026-10-17).
OPENSK_REPO="https://github.com/google/OpenSK.git"
OPENSK_COMMIT="636d85b8ae492f30157cb38d9d84c3c40b9f1f0c"
WASEFIRE_COMMIT="6d8f4fc9714004af9d86b4218da0ce6d52db84f4"
PATCH="$REPO_DIR/patches/wasefire-feitian-unpin.patch"

# Same build as OpenSK's `./flash.sh nrf52840_dongle`, run from the Wasefire dir.
XTASK=(cargo xtask --release --native applet rust ../.. --opt-level=z
  --features=ctap1,config-command
  runner nordic --board=dongle --opt-level=z --features=usb-ctap
  --features=software-crypto-aes256-cbc,software-crypto-hmac-sha256,software-crypto-p256-ecdh,software-crypto-p256-ecdsa)
FEITIAN_FEATURES=(--features=feitian-unpin --features=feitian-button-p018)

mkdir -p "$OUT_DIR"

_docker_args() {
  local tty=()
  [ -t 0 ] && tty=(-t)
  local dialout
  dialout="$(getent group dialout | cut -d: -f3)"
  printf '%s\n' --rm -i "${tty[@]}" --privileged \
    -v /dev:/dev ${dialout:+--group-add "$dialout"} \
    -v "$OPENSK_DIR:/work:z" -v "$REPO_DIR:/repo:z" -v "$OUT_DIR:/out:z" \
    -v opensk-cargo-registry:/home/dev/.cargo/registry \
    -v opensk-cargo-git:/home/dev/.cargo/git \
    -w /work
}

# Run a command in the build container as the unprivileged `dev` user (uid 1000).
in_docker() {
  local args
  mapfile -t args < <(_docker_args)
  docker run "${args[@]}" "$IMAGE" "$@"
}

# Run as root: needed by the Wasefire USB protocol (libusb on /dev/bus/usb).
# Gives back ownership of build outputs afterwards.
in_docker_root() {
  local args
  mapfile -t args < <(_docker_args)
  docker run "${args[@]}" -u 0 -e CARGO_NET_OFFLINE="${CARGO_NET_OFFLINE:-false}" "$IMAGE" \
    bash -c '"$@"; rc=$?; chown -R 1000:1000 /work/target /work/third_party/wasefire/target /work/third_party/wasefire/.root /out 2>/dev/null; exit $rc' _ "$@"
}

usb_mode() {
  if lsusb | grep -q '1915:521f'; then echo dfu
  elif lsusb | grep -q '18d1:0239'; then echo wasefire
  elif lsusb | grep -q '1209:0001'; then echo uicr-fix
  else echo none; fi
}

die() { echo "error: $*" >&2; exit 1; }
