#!/usr/bin/env bash
# Copy packaging/homebrew/mollow.rb into ingeniousfrog/homebrew-tap and push.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TAP_REPO="${MOLLOW_TAP_REPO:-https://github.com/ingeniousfrog/homebrew-tap.git}"
TAP_DIR="${MOLLOW_TAP_DIR:-$(mktemp -d)}"
KEEP_TAP_DIR="${MOLLOW_KEEP_TAP_DIR:-0}"

cleanup() {
  if [[ "${KEEP_TAP_DIR}" == "0" && -d "${TAP_DIR}" ]]; then
    rm -rf "${TAP_DIR}"
  fi
}
trap cleanup EXIT

if [[ ! -f "${ROOT}/packaging/homebrew/mollow.rb" ]]; then
  echo "missing ${ROOT}/packaging/homebrew/mollow.rb" >&2
  exit 1
fi

if [[ ! -d "${TAP_DIR}/.git" ]]; then
  git clone "${TAP_REPO}" "${TAP_DIR}"
fi

mkdir -p "${TAP_DIR}/Formula"
cp "${ROOT}/packaging/homebrew/mollow.rb" "${TAP_DIR}/Formula/mollow.rb"

cd "${TAP_DIR}"
git add Formula/mollow.rb
if git diff --cached --quiet; then
  echo "No formula changes to push."
  exit 0
fi

git commit -m "Update mollow formula"
git push origin main
echo "Pushed Formula/mollow.rb to ${TAP_REPO}"
