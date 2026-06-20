#!/usr/bin/env bash
set -euo pipefail

VERSION="${MOLLOW_VERSION:-0.1.2}"
REPO="${MOLLOW_REPO:-ingeniousfrog/Mollow}"
INSTALL_DIR="${MOLLOW_INSTALL_DIR:-${HOME}/.local/bin}"

usage() {
  cat <<EOF
Install Mollow from GitHub Releases.

Environment variables:
  MOLLOW_VERSION      Release tag version without leading v (default: ${VERSION})
  MOLLOW_REPO         GitHub repository (default: ${REPO})
  MOLLOW_LINUX_TARGET Linux target triple (default on x86_64: x86_64-unknown-linux-musl)

Examples:
  curl -fsSL https://raw.githubusercontent.com/${REPO}/main/packaging/install.sh | bash
  MOLLOW_INSTALL_DIR=/usr/local/bin curl -fsSL ... | bash
EOF
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

detect_target() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"

  case "${os}/${arch}" in
    Darwin/arm64) echo "aarch64-apple-darwin" ;;
    Darwin/x86_64) echo "x86_64-apple-darwin" ;;
    Linux/x86_64) echo "${MOLLOW_LINUX_TARGET:-x86_64-unknown-linux-musl}" ;;
    *)
      echo "unsupported platform: ${os}/${arch}" >&2
      echo "Build from source: cargo build --release -p mollow" >&2
      exit 1
      ;;
  esac
}

TARGET="$(detect_target)"
BASE_URL="https://github.com/${REPO}/releases/download/v${VERSION}"
ASSET="mollow-${TARGET}.tar.gz"
URL="${BASE_URL}/${ASSET}"

tmpdir="$(mktemp -d)"
cleanup() {
  rm -rf "${tmpdir}"
}
trap cleanup EXIT

echo "Installing mollow v${VERSION} for ${TARGET}..."
curl -fsSL "${URL}" -o "${tmpdir}/${ASSET}"
tar -xzf "${tmpdir}/${ASSET}" -C "${tmpdir}"

mkdir -p "${INSTALL_DIR}"
install -m 0755 "${tmpdir}/mollow" "${INSTALL_DIR}/mollow"

echo "Installed mollow to ${INSTALL_DIR}/mollow"
if ! command -v mollow >/dev/null 2>&1; then
  echo "Add ${INSTALL_DIR} to your PATH if needed."
fi
mollow --version
