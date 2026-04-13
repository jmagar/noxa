#!/usr/bin/env bash
# setup.sh — Thin shim. Builds noxa if needed, then delegates to `noxa setup`.
#
# For first-time install from a fresh clone:
#   ./setup.sh
#
# After cargo install (no clone needed):
#   noxa setup
#
# One-liner install (no clone at all):
#   curl -fsSL https://raw.githubusercontent.com/jmagar/noxa/main/install.sh | bash

set -euo pipefail

BOLD="\x1b[1m"
DIM="\x1b[2m"
CYAN="\x1b[96m"
GREEN="\x1b[92m"
RESET="\x1b[0m"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BINARY="$SCRIPT_DIR/target/release/noxa"

if [[ ! -x "$BINARY" ]]; then
    echo
    echo -e "  ${BOLD}Building noxa...${RESET}  ${DIM}(first run)${RESET}"
    echo
    cd "$SCRIPT_DIR"
    cargo build --release -p noxa-cli 2>&1 | sed 's/^/  /'
    echo
    echo -e "  ${GREEN}✓${RESET} ${BOLD}Build complete${RESET}"
    echo
fi

exec "$BINARY" setup "$@"
