#!/usr/bin/env bash
set -euo pipefail

REPO_URL="${ROT_REPO_URL:-https://github.com/akashrtd/rot.git}"
PACKAGE="rot-cli"

if ! command -v cargo >/dev/null 2>&1; then
  echo "error: cargo not found. Install Rust first: https://rustup.rs/" >&2
  exit 1
fi

INSTALL_ARGS=(--git "$REPO_URL" --locked "$PACKAGE")

# Optional tag install:
#   ROT_VERSION=v0.1.0 curl -fsSL .../install.sh | bash
if [[ -n "${ROT_VERSION:-}" ]]; then
  INSTALL_ARGS=(--git "$REPO_URL" --tag "$ROT_VERSION" --locked "$PACKAGE")
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

if command -v rot >/dev/null 2>&1; then
  echo "Installed: $(rot --version)"
else
  echo "Installed, but 'rot' is not on PATH yet."
  echo "Add \$HOME/.cargo/bin to your PATH and restart the shell."
fi
