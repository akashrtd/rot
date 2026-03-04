#!/usr/bin/env bash
set -euo pipefail

REPO_URL="${ROT_REPO_URL:-https://github.com/akashrtd/rot.git}"
PACKAGE="rot-cli"
CARGO_BIN_DIR="${CARGO_HOME:-$HOME/.cargo}/bin"

if ! command -v cargo >/dev/null 2>&1; then
  echo "error: cargo not found. Install Rust first: https://rustup.rs/" >&2
  exit 1
fi

INSTALL_ARGS=(--git "$REPO_URL" --locked --bin rot "$PACKAGE")

# Optional tag install:
#   ROT_VERSION=v0.1.0 curl -fsSL .../install.sh | bash
if [[ -n "${ROT_VERSION:-}" ]]; then
  INSTALL_ARGS=(--git "$REPO_URL" --tag "$ROT_VERSION" --locked --bin rot "$PACKAGE")
fi

# Force reinstall when requested:
#   ROT_FORCE=1 curl -fsSL .../install.sh | bash
if [[ "${ROT_FORCE:-0}" == "1" ]]; then
  INSTALL_ARGS+=(--force)
fi

if [[ "${ROT_DRY_RUN:-0}" == "1" ]]; then
  echo "cargo install ${INSTALL_ARGS[*]}"
  exit 0
fi

echo "Installing rot via cargo..."
cargo install "${INSTALL_ARGS[@]}"

# Try to activate cargo bin path for this shell run.
if [[ -f "${CARGO_HOME:-$HOME/.cargo}/env" ]]; then
  # shellcheck disable=SC1090
  . "${CARGO_HOME:-$HOME/.cargo}/env"
fi
if [[ ":$PATH:" != *":$CARGO_BIN_DIR:"* ]]; then
  export PATH="$CARGO_BIN_DIR:$PATH"
fi

if command -v rot >/dev/null 2>&1; then
  echo "Installed: $(rot --version)"
else
  echo "Installed, but 'rot' is not on PATH in your current shell."
  echo "Run one of:"
  echo "  source \"\${CARGO_HOME:-\$HOME/.cargo}/env\""
  echo "  export PATH=\"$CARGO_BIN_DIR:\$PATH\""
fi
