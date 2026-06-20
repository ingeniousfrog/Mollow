# Homebrew formula for Mollow (CLI).
# Publish this file in: github.com/ingeniousfrog/homebrew-tap/Formula/mollow.rb
# After each release, run packaging/update-homebrew-sha256.sh and copy here (see docs/homebrew.md).

class Mollow < Formula
  desc "Cross-platform machine inspection and performance-baseline CLI"
  homepage "https://github.com/ingeniousfrog/Mollow"
  license "Apache-2.0"
  version "0.1.2"

  on_macos do
    on_arm do
      url "https://github.com/ingeniousfrog/Mollow/releases/download/v0.1.2/mollow-aarch64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_RELEASE_SHA256_AARCH64_DARWIN"
    end
    on_intel do
      url "https://github.com/ingeniousfrog/Mollow/releases/download/v0.1.2/mollow-x86_64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_RELEASE_SHA256_X86_64_DARWIN"
    end
  end

  on_linux do
    url "https://github.com/ingeniousfrog/Mollow/releases/download/v0.1.2/mollow-x86_64-unknown-linux-gnu.tar.gz"
    sha256 "REPLACE_WITH_RELEASE_SHA256_LINUX"
  end

  def install
    bin.install "mollow"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/mollow --version")
    assert_match "inspect", shell_output("#{bin}/mollow --help")
  end
end
