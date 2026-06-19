# Homebrew formula for Mollow (CLI).
# Publish this file in: github.com/ingeniousfrog/homebrew-tap/Formula/mollow.rb
# After each release, update version, url, and sha256 (see docs/homebrew.md).

class Mollow < Formula
  desc "Cross-platform machine inspection and performance-baseline CLI"
  homepage "https://github.com/ingeniousfrog/Mollow"
  url "https://github.com/ingeniousfrog/Mollow/releases/download/v0.1.0/mollow-aarch64-apple-darwin.tar.gz"
  sha256 "REPLACE_WITH_RELEASE_SHA256"
  license "Apache-2.0"
  version "0.1.0"

  depends_on macos: ">= :big_sur"

  def install
    bin.install "mollow"
  end

  test do
    assert_match "inspect", shell_output("#{bin}/mollow --help")
  end
end
