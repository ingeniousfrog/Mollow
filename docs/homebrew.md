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

## Maintainer workflow

1. Build release binaries (at minimum Apple Silicon macOS):

   ```bash
   cargo build --release -p mollow
   tar -czf mollow-aarch64-apple-darwin.tar.gz -C target/release mollow
   shasum -a 256 mollow-aarch64-apple-darwin.tar.gz
   ```

2. Create a GitHub Release on `ingeniousfrog/Mollow` (tag `v<version>`) and upload the tarball.

3. Copy [`packaging/homebrew/mollow.rb`](../packaging/homebrew/mollow.rb) into the tap as `Formula/mollow.rb`.

4. Set `version`, `url`, and `sha256` to match the release asset.

5. Push to `homebrew-tap`. Users can then `brew install mollow`.

## Optional: Intel macOS / Linux

Add separate release assets and either:

- one formula with `on_macos` / `on_linux` blocks and per-arch `url` + `sha256`, or
- split formulas (`mollow`, `mollow-linux`) if that is simpler to maintain.

## Relation to CacheBar tap docs

The tap README already documents multi-app usage: one tap, many casks **and** formulas.
Mollow only adds a `Formula/` entry alongside existing `Casks/cachebar.rb`.
