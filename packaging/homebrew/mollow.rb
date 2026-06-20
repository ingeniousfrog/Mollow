# Homebrew formula for Mollow (CLI).
# Publish this file in: github.com/ingeniousfrog/homebrew-tap/Formula/mollow.rb
# After each release, run packaging/update-homebrew-sha256.sh and copy here (see docs/homebrew.md).

class Mollow < Formula
  desc "Cross-platform machine inspection and performance-baseline CLI"
  homepage "https://github.com/ingeniousfrog/Mollow"
  license "Apache-2.0"
  version "0.1.0"

  on_macos do
    on_arm do
      url "https://github.com/ingeniousfrog/Mollow/releases/download/v0.1.0/mollow-aarch64-apple-darwin.tar.gz"
      sha256 "240a6728102ddee8edc051e0243ba894e0b0fbbab7089dbf798c26d22734b4e8"
    end
    on_intel do
      url "https://github.com/ingeniousfrog/Mollow/releases/download/v0.1.0/mollow-x86_64-apple-darwin.tar.gz"
      sha256 "7fc9128816aefca6b7a5ac0f4ee06ea42e0757319e391c9a97163ab672525f86"
    end
  end

  on_linux do
    url "https://github.com/ingeniousfrog/Mollow/releases/download/v0.1.0/mollow-x86_64-unknown-linux-gnu.tar.gz"
    sha256 "50b50b5fc5a6e6d27a2b64f701a0de27208100532413d6cb73ab7c9ce6e16818"
  end

  def install
    bin.install "mollow"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/mollow --version")
    assert_match "inspect", shell_output("#{bin}/mollow --help")
  end
end
