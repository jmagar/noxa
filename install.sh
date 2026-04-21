#!/usr/bin/env bash
# install.sh — One-liner installer for noxa
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/jmagar/noxa/main/install.sh | bash

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
RESET='\033[0m'

info()    { printf "${BLUE}[*]${RESET} %s\n" "$*"; }
success() { printf "${GREEN}[+]${RESET} %s\n" "$*"; }
warn()    { printf "${YELLOW}[!]${RESET} %s\n" "$*"; }
error()   { printf "${RED}[x]${RESET} %s\n" "$*" >&2; exit 1; }

echo
printf "\033[1;32m  noxa — Installer\033[0m\n"
echo

# ---------------------------------------------------------------------------
# 1. Rust / cargo
# ---------------------------------------------------------------------------
if ! command -v cargo &>/dev/null; then
    info "Rust not found. Installing via rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path
    # shellcheck source=/dev/null
    source "$HOME/.cargo/env"
    success "Rust installed."
else
    success "Rust $(rustc --version | awk '{print $2}') already installed."
fi

# Make sure cargo is on PATH for this session
export PATH="$HOME/.cargo/bin:$PATH"

# ---------------------------------------------------------------------------
# 2. Build & install noxa
# ---------------------------------------------------------------------------
info "Installing noxa via cargo..."
cargo install --git https://github.com/jmagar/noxa --bin noxa --bin noxa-mcp --bin noxa-rag-daemon --locked 2>&1 \
    | grep -E '^(error|warning: unused|Compiling|Finished|Installing|Replacing)' || true

if ! command -v noxa &>/dev/null; then
    # cargo install puts binaries in ~/.cargo/bin — not yet on PATH in this shell
    export PATH="$HOME/.cargo/bin:$PATH"
fi

if ! command -v noxa &>/dev/null; then
    error "noxa binary not found after install. Check cargo output above."
fi

success "noxa $(noxa --version 2>/dev/null || echo installed)."

# ---------------------------------------------------------------------------
# 3. Interactive setup
# ---------------------------------------------------------------------------
echo
info "Launching interactive setup..."
echo

exec noxa setup
