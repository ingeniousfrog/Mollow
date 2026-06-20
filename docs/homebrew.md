# Homebrew distribution

Mollow is a command-line tool. Use a Homebrew **Formula** in
[ingeniousfrog/homebrew-tap](https://github.com/ingeniousfrog/homebrew-tap), not a **Cask**.

| Artifact | Homebrew type | Example in tap |
| --- | --- | --- |
| CacheBar (`.app` in DMG) | Cask | `Casks/cachebar.rb` |
| Mollow (`mollow` binary) | Formula | `Formula/mollow.rb` |

## User install (after first release)

```bash
brew tap ingeniousfrog/tap
brew install mollow
mollow inspect --format terminal --lang zh-CN
```

On Linux with Homebrew installed, the same tap installs the Linux tarball from the
multi-platform formula in [`packaging/homebrew/mollow.rb`](../packaging/homebrew/mollow.rb).

## Maintainer workflow

### Automated release (recommended)

Push a version tag to trigger [`.github/workflows/release.yml`](../.github/workflows/release.yml):

```bash
git tag v0.1.0
git push origin v0.1.0
```

The workflow builds and uploads:

| Asset | Platform |
| --- | --- |
| `mollow-aarch64-apple-darwin.tar.gz` | macOS Apple Silicon |
| `mollow-x86_64-apple-darwin.tar.gz` | macOS Intel |
| `mollow-x86_64-unknown-linux-gnu.tar.gz` | Linux x86_64 |
| `mollow-x86_64-pc-windows-msvc.zip` | Windows x86_64 |

### Update the tap formula

1. After the GitHub Release finishes, refresh checksum placeholders:

   ```bash
   ./packaging/update-homebrew-sha256.sh 0.1.0
   ```

2. Copy [`packaging/homebrew/mollow.rb`](../packaging/homebrew/mollow.rb) into the tap as
   `Formula/mollow.rb` (update `version` if needed).

3. Push to [homebrew-tap](https://github.com/ingeniousfrog/homebrew-tap). Users can then
   `brew install mollow`.

   ```bash
   ./packaging/push-homebrew-tap.sh
   ```

### Manual fallback

If you need to build locally:

```bash
cargo build --release -p mollow --target aarch64-apple-darwin
tar -czf mollow-aarch64-apple-darwin.tar.gz -C target/aarch64-apple-darwin/release mollow
shasum -a 256 mollow-aarch64-apple-darwin.tar.gz
```

## Relation to CacheBar tap docs

The tap README already documents multi-app usage: one tap, many casks **and** formulas.
Mollow only adds a `Formula/` entry alongside existing `Casks/cachebar.rb`.
