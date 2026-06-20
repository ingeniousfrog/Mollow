#!/usr/bin/env bash
set -euo pipefail

# Recompute sha256 placeholders for packaging/homebrew/mollow.rb after a release.
# Usage: ./packaging/update-homebrew-sha256.sh 0.1.0

VERSION="${1:?usage: $0 <version-without-v>}"
REPO="${MOLLOW_REPO:-ingeniousfrog/Mollow}"
BASE="https://github.com/${REPO}/releases/download/v${VERSION}"
FORMULA="packaging/homebrew/mollow.rb"
tmp="$(mktemp)"
cp "${FORMULA}" "${tmp}"

update_sha256() {
  local placeholder="$1"
  local asset="$2"
  local url="${BASE}/${asset}"
  echo "Fetching sha256 for ${asset}..."
  local sha256
  sha256="$(curl -fsSL "${url}" | shasum -a 256 | awk '{print $1}')"
  sed -i.bak "s/${placeholder}/${sha256}/" "${tmp}"
}

update_sha256 "REPLACE_WITH_RELEASE_SHA256_AARCH64_DARWIN" "mollow-aarch64-apple-darwin.tar.gz"
update_sha256 "REPLACE_WITH_RELEASE_SHA256_X86_64_DARWIN" "mollow-x86_64-apple-darwin.tar.gz"
update_sha256 "REPLACE_WITH_RELEASE_SHA256_LINUX" "mollow-x86_64-unknown-linux-gnu.tar.gz"

rm -f "${tmp}.bak"
mv "${tmp}" "${FORMULA}"
echo "Updated ${FORMULA}. Copy it to homebrew-tap/Formula/mollow.rb and push."
