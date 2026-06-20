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
      sha256 "4907288d0e4ac7064beb8643f5f409eafe0c119c0c24b1f8e5bf2c9a0469a15c"
    end
    on_intel do
      url "https://github.com/ingeniousfrog/Mollow/releases/download/v0.1.2/mollow-x86_64-apple-darwin.tar.gz"
      sha256 "e1b575a1ae7ad3f9ec2bee3a79ebb78a60a1d40d2700c0299920922270df55e6"
    end
  end

  on_linux do
    url "https://github.com/ingeniousfrog/Mollow/releases/download/v0.1.2/mollow-x86_64-unknown-linux-gnu.tar.gz"
    sha256 "4c25d1892ad53f897e9d9d7b0c77e36db9a16d935edec26337bc3362b6200266"
  end

  def install
    bin.install "mollow"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/mollow --version")
    assert_match "inspect", shell_output("#{bin}/mollow --help")
  end
end
