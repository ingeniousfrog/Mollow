#!/usr/bin/env bash
set -euo pipefail

# Update Scoop and winget checksum placeholders after a release.
# Usage: ./packaging/update-package-checksums.sh 0.1.0

VERSION="${1:?usage: $0 <version-without-v>}"
REPO="${MOLLOW_REPO:-ingeniousfrog/Mollow}"
BASE="https://github.com/${REPO}/releases/download/v${VERSION}"
WINDOWS_ASSET="mollow-x86_64-pc-windows-msvc.zip"
URL="${BASE}/${WINDOWS_ASSET}"

echo "Fetching sha256 for ${WINDOWS_ASSET}..."
SHA256="$(curl -fsSL "${URL}" | shasum -a 256 | awk '{print $1}')"

SCOOP="packaging/scoop/mollow.json"
WINGET="packaging/winget/ingeniousfrog.Mollow.yaml"

sed -i.bak "s/REPLACE_WITH_RELEASE_SHA256_WINDOWS/${SHA256}/" "${SCOOP}"
sed -i.bak "s/REPLACE_WITH_RELEASE_SHA256_WINDOWS/${SHA256}/" "${WINGET}"
rm -f "${SCOOP}.bak" "${WINGET}.bak"

echo "Updated ${SCOOP} and ${WINGET}."
