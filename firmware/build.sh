#!/usr/bin/env bash
# Builds the f16custom firmware for the Framework Laptop 16 ANSI keyboard and
# the RGB macropad. Meant to run in CI (GitHub Actions, qmkfm/qmk_cli container)
# but works on any Linux box with the QMK build tools installed.
#
#   ./firmware/build.sh            # build both
#   ./firmware/build.sh ansi       # build only the keyboard
#   ./firmware/build.sh macropad   # build only the macropad
#   FETCH_ONLY=1 ./firmware/build.sh   # just download Framework's QMK source
#
# Output: firmware/out/*.uf2
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# Framework's QMK fork, pinned to the release tag this keymap was written for.
QMK_REPO="${QMK_REPO:-https://github.com/FrameworkComputer/qmk_firmware.git}"
QMK_REF="${QMK_REF:-v0.3.1}"
QMK_DIR="${QMK_DIR:-$HERE/.qmk_firmware}"
OUT_DIR="$HERE/out"
KEYMAP=f16custom

targets=("$@")
[ ${#targets[@]} -eq 0 ] && targets=(ansi macropad)

if [ ! -d "$QMK_DIR/.git" ]; then
    echo ">> Cloning $QMK_REPO @ $QMK_REF into $QMK_DIR"
    git clone --depth 1 --branch "$QMK_REF" "$QMK_REPO" "$QMK_DIR"
fi
echo ">> Fetching the QMK submodules needed for RP2040"
git -C "$QMK_DIR" submodule update --init --depth 1 \
    lib/chibios lib/chibios-contrib lib/printf lib/pico-sdk lib/lufa

if [ "${FETCH_ONLY:-0}" = "1" ]; then
    echo ">> FETCH_ONLY=1: QMK source ready in $QMK_DIR"
    exit 0
fi

echo ">> Copying our keymaps into the QMK tree"
rm -rf "$QMK_DIR/users/$KEYMAP"
cp -r "$HERE/users/$KEYMAP" "$QMK_DIR/users/$KEYMAP"
for kb in "${targets[@]}"; do
    dest="$QMK_DIR/keyboards/framework/$kb/keymaps/$KEYMAP"
    rm -rf "$dest"
    cp -r "$HERE/keymaps/$kb/$KEYMAP" "$dest"
done

# Build number: the CI run number for builds of main and of release tags, so the app can tell
# which firmware is newer. Pull requests and builds made by hand are test builds (0).
BUILD=0
if [ "${GITHUB_ACTIONS:-}" = "true" ] && [ "${GITHUB_EVENT_NAME:-}" != "pull_request" ]; then
    case "${GITHUB_REF:-}" in
        refs/heads/main | refs/tags/*) BUILD="${GITHUB_RUN_NUMBER:-0}" ;;
    esac
fi
echo ">> Firmware build number: $BUILD"

mkdir -p "$OUT_DIR"
for kb in "${targets[@]}"; do
    echo ">> Building framework/$kb:$KEYMAP"
    make -C "$QMK_DIR" "framework/$kb:$KEYMAP" -j"$(nproc)" F16_FIRMWARE_BUILD="$BUILD"
    cp "$QMK_DIR/framework_${kb}_${KEYMAP}.uf2" "$OUT_DIR/"
done

echo ">> Done:"
ls -l "$OUT_DIR"
