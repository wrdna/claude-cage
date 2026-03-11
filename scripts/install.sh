#!/usr/bin/env bash

set -e

CURRENT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )/.." && pwd )"
cd "$CURRENT_DIR"

echo "claude-cage: building from source..."

# Check for cargo
if ! command -v cargo &>/dev/null; then
    if [ -f "$HOME/.cargo/env" ]; then
        source "$HOME/.cargo/env"
    elif [ -d "$HOME/.cargo/bin" ]; then
        export PATH="$HOME/.cargo/bin:$PATH"
    else
        echo "claude-cage: cargo not found. Install Rust: https://rustup.rs"
        exit 1
    fi
fi

cargo build --release

# Symlink binary into PATH for hook commands (claude-cage state)
INSTALL_DIR="$HOME/.local/bin"
mkdir -p "$INSTALL_DIR"
ln -sf "$CURRENT_DIR/target/release/claude-cage" "$INSTALL_DIR/claude-cage"

echo "claude-cage: installed successfully"
