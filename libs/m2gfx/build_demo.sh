#!/bin/bash
# Build the m2gfx demo app
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
M2C="${M2C:-m2c}"

"$M2C" "$SCRIPT_DIR/../../example_apps/gfx_demo.mod" \
    -I "$SCRIPT_DIR/src" \
    --cflag -I/opt/homebrew/include \
    -l SDL2 -l SDL2_ttf \
    -L /opt/homebrew/lib \
    "$SCRIPT_DIR/src/gfx_bridge.c" \
    -o /tmp/gfx_demo

echo "Built: /tmp/gfx_demo"
if [ "$1" = "run" ]; then
    /tmp/gfx_demo
fi
