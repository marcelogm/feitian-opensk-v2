#!/usr/bin/env bash
# Builds the docker image, clones OpenSK at the pinned commit and applies the Feitian patch.
# Safe to re-run.
set -euo pipefail
. "$(dirname "$0")/common.sh"

docker build -t "$IMAGE" "$REPO_DIR"

if [ ! -d "$OPENSK_DIR/.git" ]; then
  git clone "$OPENSK_REPO" "$OPENSK_DIR"
fi
git -C "$OPENSK_DIR" fetch --quiet origin
git -C "$OPENSK_DIR" checkout --quiet "$OPENSK_COMMIT"
git -C "$OPENSK_DIR" submodule update --init --quiet
[ "$(git -C "$WASEFIRE_DIR" rev-parse HEAD)" = "$WASEFIRE_COMMIT" ] \
  || die "unexpected Wasefire commit; this procedure was validated with $WASEFIRE_COMMIT"

if git -C "$WASEFIRE_DIR" apply --reverse --check "$PATCH" 2>/dev/null; then
  echo "patch already applied"
else
  git -C "$WASEFIRE_DIR" apply "$PATCH"
  echo "patch applied"
fi

echo "setup done: OpenSK in $OPENSK_DIR"
