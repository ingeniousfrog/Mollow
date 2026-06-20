#!/usr/bin/env bash
# Push packaging/homebrew/mollow.rb to ingeniousfrog/homebrew-tap (CI or local with token).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FORMULA="${ROOT}/packaging/homebrew/mollow.rb"
TAP_REPO="${MOLLOW_TAP_REPO:-ingeniousfrog/homebrew-tap}"
TAP_DIR="${MOLLOW_TAP_DIR:-$(mktemp -d)}"
KEEP_TAP_DIR="${MOLLOW_KEEP_TAP_DIR:-0}"
TOKEN="${HOMEBREW_TAP_TOKEN:-${GITHUB_TOKEN:-}}"

cleanup() {
  if [[ "${KEEP_TAP_DIR}" == "0" && -d "${TAP_DIR}" ]]; then
    rm -rf "${TAP_DIR}"
  fi
}
trap cleanup EXIT

if [[ ! -f "${FORMULA}" ]]; then
  echo "missing ${FORMULA}" >&2
  exit 1
fi

if [[ -z "${TOKEN}" ]]; then
  echo "HOMEBREW_TAP_TOKEN (or GITHUB_TOKEN) is required to push homebrew-tap." >&2
  exit 1
fi

VERSION="$(grep -E '^  version "' "${FORMULA}" | sed -E 's/^  version "(.*)"$/\1/')"
if [[ -z "${VERSION}" ]]; then
  echo "could not read version from ${FORMULA}" >&2
  exit 1
fi

git clone "https://x-access-token:${TOKEN}@github.com/${TAP_REPO}.git" "${TAP_DIR}"
mkdir -p "${TAP_DIR}/Formula"
cp "${FORMULA}" "${TAP_DIR}/Formula/mollow.rb"

cd "${TAP_DIR}"
git config user.name "github-actions[bot]"
git config user.email "41898282+github-actions[bot]@users.noreply.github.com"
git add Formula/mollow.rb

if git diff --cached --quiet; then
  echo "No formula changes to push."
  exit 0
fi

git commit -m "Update mollow formula to v${VERSION}."
git push origin HEAD
echo "Pushed Formula/mollow.rb to https://github.com/${TAP_REPO}"
