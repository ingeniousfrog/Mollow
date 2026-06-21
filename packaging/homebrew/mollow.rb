# Homebrew formula for Mollow (CLI).
# Publish this file in: github.com/ingeniousfrog/homebrew-tap/Formula/mollow.rb
# After each release, run packaging/update-homebrew-sha256.sh and copy here (see docs/homebrew.md).

class Mollow < Formula
  desc "Cross-platform machine inspection and performance-baseline CLI"
  homepage "https://github.com/ingeniousfrog/Mollow"
  license "Apache-2.0"
  version "0.1.4"

  on_macos do
    on_arm do
      url "https://github.com/ingeniousfrog/Mollow/releases/download/v0.1.4/mollow-aarch64-apple-darwin.tar.gz"
      sha256 "5e452c4ddd2543c9f506e5a0d22b1a4661f77f7009e23635c388db53cd375377"
    end
    on_intel do
      url "https://github.com/ingeniousfrog/Mollow/releases/download/v0.1.4/mollow-x86_64-apple-darwin.tar.gz"
      sha256 "1d419b68267f8edc10b87ac3a4e88df0832000d1b8d3f3f100ac4cad4a4f7d1a"
    end
  end

  on_linux do
    url "https://github.com/ingeniousfrog/Mollow/releases/download/v0.1.4/mollow-x86_64-unknown-linux-gnu.tar.gz"
    sha256 "5b8942e76e290eef82eef58df9ae695ba08e9ab88160e93c0c2e92e1db26b852"
  end

  def install
    bin.install "mollow"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/mollow --version")
    assert_match "inspect", shell_output("#{bin}/mollow --help")
  end
end
