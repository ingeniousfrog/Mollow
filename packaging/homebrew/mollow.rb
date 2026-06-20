# Homebrew formula for Mollow (CLI).
# Publish this file in: github.com/ingeniousfrog/homebrew-tap/Formula/mollow.rb
# After each release, run packaging/update-homebrew-sha256.sh and copy here (see docs/homebrew.md).

class Mollow < Formula
  desc "Cross-platform machine inspection and performance-baseline CLI"
  homepage "https://github.com/ingeniousfrog/Mollow"
  license "Apache-2.0"
  version "0.1.1"

  on_macos do
    on_arm do
      url "https://github.com/ingeniousfrog/Mollow/releases/download/v0.1.1/mollow-aarch64-apple-darwin.tar.gz"
      sha256 "e23b64c1b7c6af9c44e2f7d9f186d1f227865053d998cc618875bfd18b53e869"
    end
    on_intel do
      url "https://github.com/ingeniousfrog/Mollow/releases/download/v0.1.1/mollow-x86_64-apple-darwin.tar.gz"
      sha256 "23c6500fead62e97c3a4a8847716db7679952e8ca0246d2a979eac814c46a896"
    end
  end

  on_linux do
    url "https://github.com/ingeniousfrog/Mollow/releases/download/v0.1.1/mollow-x86_64-unknown-linux-gnu.tar.gz"
    sha256 "626b6eff23ad0f7d74fb93bcbb579ff5ba0100a42749adf5034e98a2433a0528"
  end

  def install
    bin.install "mollow"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/mollow --version")
    assert_match "inspect", shell_output("#{bin}/mollow --help")
  end
end
