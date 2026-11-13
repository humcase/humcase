#!/usr/bin/env bash
# Builds the Linux AppImage and .deb locally in Docker (no CI minutes).
#   tools/linux/build.sh            build the bundles into tools/linux/out/
#   tools/linux/build.sh check      cargo fmt --check, clippy and cargo test on Linux
#   tools/linux/build.sh shell      interactive shell in the build container
#   tools/linux/build.sh exec CMD   one command in src-tauri (e.g. 'cargo test ai_settings')
# Caches (Rust toolchain, cargo registry, target folder, npm) live in Docker volumes, so rebuilds
# are fast. The repository is mounted read-only and copied inside the container, so Windows
# node_modules and target folders never mix with the Linux ones.
set -euo pipefail

cd "$(dirname "$0")/../.."
REPO="$(pwd)"
# Git Bash on Windows: Docker wants a Windows path for the bind mounts.
if pwd -W >/dev/null 2>&1; then REPO="$(pwd -W)"; fi
OUT="$REPO/tools/linux/out"
mkdir -p tools/linux/out
export MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL='*'

IMAGE=humcase-linux-build
docker build -t "$IMAGE" tools/linux

run() {
  docker run --rm ${TTY:+-it} \
    -v "$REPO:/src:ro" \
    -v "$OUT:/out" \
    -v humcase-linux-cache:/cache \
    -v humcase-linux-npm:/root/.npm \
    -e CARGO_TARGET_DIR=/cache/target \
    "$IMAGE" bash -c "$1"
}

PREPARE='set -euo pipefail
mkdir -p /build && cd /build
tar -C /src --exclude=./.git --exclude=node_modules --exclude=./src-tauri/target --exclude=./tools/linux/out \
    --exclude=./sense/target --exclude=./.claude -cf - . | tar -xf -
CHANNEL="$(sed -n "s/^channel *= *\"\(.*\)\"/\1/p" src-tauri/rust-toolchain.toml)"
rustup toolchain list | grep -q "^$CHANNEL" || rustup toolchain install --profile minimal --component rustfmt --component clippy "$CHANNEL"
# The Tauri CLI runs rustc from the repository root, outside the folder pinned by rust-toolchain.toml.
rustup default "$CHANNEL" >/dev/null 2>&1
'

case "${1:-bundle}" in
  bundle)
    run "$PREPARE"'
npm ci
npm ci --prefix frontend
npx tauri build --bundles deb,appimage
rm -rf /out/*.AppImage /out/*.deb
cp /cache/target/release/bundle/appimage/*.AppImage /cache/target/release/bundle/deb/*.deb /out/
cp /cache/target/release/humcase /out/humcase
ls -l /out
'
    ;;
  check)
    run "$PREPARE"'
cd src-tauri
# The checks do not need the real interface, only a frontend/dist folder for tauri-build.
mkdir -p ../frontend/dist && [ -e ../frontend/dist/index.html ] || echo "<html></html>" > ../frontend/dist/index.html
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
'
    ;;
  smoke)
    # Install the .deb and start the AppImage in a throwaway container: build.sh smoke cli|ui [x,y]
    shift
    docker run --rm -v "$REPO:/src:ro" -v "$OUT:/out" "$IMAGE" bash /src/tools/linux/smoke.sh "$@"
    ;;
  exec)
    # One command in src-tauri, e.g.: build.sh exec 'cargo test ai_settings'
    run "$PREPARE"'cd src-tauri && mkdir -p ../frontend/dist && [ -e ../frontend/dist/index.html ] || echo "<html></html>" > ../frontend/dist/index.html
'"${2:?command}"
    ;;
  shell)
    TTY=1 run "$PREPARE"'exec bash'
    ;;
  *) echo "usage: $0 [bundle|check|shell]" >&2; exit 2 ;;
esac
