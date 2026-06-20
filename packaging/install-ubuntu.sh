#!/usr/bin/env bash
# Ubuntu / Debian x86_64 installer for Mollow (GitHub Releases binary).
set -euo pipefail

VERSION="${MOLLOW_VERSION:-}"
REPO="${MOLLOW_REPO:-ingeniousfrog/Mollow}"
INSTALL_DIR="${MOLLOW_INSTALL_DIR:-/usr/local/bin}"
TARGET="${MOLLOW_LINUX_TARGET:-x86_64-unknown-linux-musl}"

fetch_latest_version() {
  curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null \
    | grep -m1 '"tag_name"' \
    | sed -E 's/.*"tag_name":[[:space:]]*"v([^"]+)".*/\1/' \
    || true
}

if [[ -z "${VERSION}" ]]; then
  VERSION="$(fetch_latest_version)"
fi
VERSION="${VERSION:-0.1.3}"

ASSET="mollow-${TARGET}.tar.gz"
URL="https://github.com/${REPO}/releases/download/v${VERSION}/${ASSET}"

usage() {
  cat <<EOF
Install Mollow on Ubuntu/Debian (x86_64) from GitHub Releases.

Uses the musl static binary by default for compatibility with older glibc
(Ubuntu 20.04, Alibaba Cloud ECS, etc.). Override with MOLLOW_LINUX_TARGET if needed.

Environment variables:
  MOLLOW_VERSION       Release version without leading v (default: latest GitHub release)
  MOLLOW_REPO          GitHub repository (default: ${REPO})
  MOLLOW_INSTALL_DIR   Install directory (default: ${INSTALL_DIR})
  MOLLOW_LINUX_TARGET    Linux target triple (default: ${TARGET})

Examples:
  curl -fsSL https://raw.githubusercontent.com/${REPO}/main/packaging/install-ubuntu.sh | sudo bash
  MOLLOW_INSTALL_DIR="\$HOME/.local/bin" bash install-ubuntu.sh
EOF
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

if [[ "$(uname -s)" != "Linux" || "$(uname -m)" != "x86_64" ]]; then
  echo "install-ubuntu.sh supports Linux x86_64 only." >&2
  echo "Use packaging/install.sh on macOS or build from source." >&2
  exit 1
fi

if ! command -v curl >/dev/null 2>&1; then
  echo "curl is required. Install with: sudo apt-get update && sudo apt-get install -y curl" >&2
  exit 1
fi

if ! command -v tar >/dev/null 2>&1; then
  echo "tar is required. Install with: sudo apt-get install -y tar" >&2
  exit 1
fi

tmpdir="$(mktemp -d)"
cleanup() {
  rm -rf "${tmpdir}"
}
trap cleanup EXIT

echo "Installing mollow v${VERSION} for ${TARGET}..."
curl -fsSL "${URL}" -o "${tmpdir}/${ASSET}"
tar -xzf "${tmpdir}/${ASSET}" -C "${tmpdir}"

mkdir -p "${INSTALL_DIR}"
if [[ -w "${INSTALL_DIR}" ]]; then
  install -m 0755 "${tmpdir}/mollow" "${INSTALL_DIR}/mollow"
else
  echo "Installing to ${INSTALL_DIR} requires elevated permissions..."
  sudo install -m 0755 "${tmpdir}/mollow" "${INSTALL_DIR}/mollow"
fi

echo "Installed mollow to ${INSTALL_DIR}/mollow"
if ! command -v mollow >/dev/null 2>&1; then
  echo "Ensure ${INSTALL_DIR} is on your PATH."
fi

if ! "${INSTALL_DIR}/mollow" --version; then
  echo "mollow failed to start." >&2
  echo "If you see GLIBC_* errors, ensure you are on mollow >= 0.1.2 with the musl binary," >&2
  echo "or build from source: cargo build --release -p mollow" >&2
  exit 1
fi
