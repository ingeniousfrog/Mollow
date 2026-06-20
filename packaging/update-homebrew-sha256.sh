#!/usr/bin/env bash
set -euo pipefail

# Refresh packaging/homebrew/mollow.rb from published GitHub Release assets.
# Usage: ./packaging/update-homebrew-sha256.sh 0.1.0

VERSION="${1:?usage: $0 <version-without-v>}"
REPO="${MOLLOW_REPO:-ingeniousfrog/Mollow}"
BASE="https://github.com/${REPO}/releases/download/v${VERSION}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

fetch_sha256() {
  local asset="$1"
  local url="${BASE}/${asset}"
  echo "Fetching sha256 for ${asset}..."
  curl -fsSL "${url}" | shasum -a 256 | awk '{print $1}'
}

SHA_ARM64="$(fetch_sha256 "mollow-aarch64-apple-darwin.tar.gz")"
SHA_X86_64_DARWIN="$(fetch_sha256 "mollow-x86_64-apple-darwin.tar.gz")"
SHA_LINUX="$(fetch_sha256 "mollow-x86_64-unknown-linux-gnu.tar.gz")"

"${ROOT}/packaging/render-homebrew-formula.sh" \
  "${VERSION}" "${SHA_ARM64}" "${SHA_X86_64_DARWIN}" "${SHA_LINUX}"
echo "Updated packaging/homebrew/mollow.rb. Push with ./packaging/push-homebrew-tap.sh if needed."
