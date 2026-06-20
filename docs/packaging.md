# Packaging and installation

Mollow ships as a single CLI binary. Choose the path that matches your platform.

## macOS

### Homebrew (recommended after first release)

```bash
brew tap ingeniousfrog/tap
brew install mollow
```

See [homebrew.md](homebrew.md) for maintainer steps.

### Install script

```bash
curl -fsSL https://raw.githubusercontent.com/ingeniousfrog/Mollow/main/packaging/install.sh | bash
```

## Linux

Mollow is **not** in Debian/Ubuntu official apt repositories. Use one of these instead:

### Ubuntu / Debian install script

```bash
curl -fsSL https://raw.githubusercontent.com/ingeniousfrog/Mollow/main/packaging/install-ubuntu.sh | sudo bash
```

Installs to `/usr/local/bin` by default. For a user-local install:

```bash
MOLLOW_INSTALL_DIR="$HOME/.local/bin" bash install-ubuntu.sh
```

### Generic Linux / macOS install script

```bash
curl -fsSL https://raw.githubusercontent.com/ingeniousfrog/Mollow/main/packaging/install.sh | bash
```

Linux x86_64 installs the **musl** static binary by default. Override with
`MOLLOW_LINUX_TARGET=x86_64-unknown-linux-gnu` if you need the glibc build (2.35+).

### Homebrew on Linux

```bash
brew tap ingeniousfrog/tap
brew install mollow
```

### Build from source

```bash
cargo build --release -p mollow
```

## Windows

### PowerShell install script

```powershell
irm https://raw.githubusercontent.com/ingeniousfrog/Mollow/main/packaging/install.ps1 | iex
```

### Scoop

Copy [`packaging/scoop/mollow.json`](../packaging/scoop/mollow.json) into your bucket or tap,
update the `hash` field after each release, then:

```powershell
scoop install mollow
```

### winget

Use [`packaging/winget/ingeniousfrog.Mollow.yaml`](../packaging/winget/ingeniousfrog.Mollow.yaml)
as the starting point for a [winget-pkgs](https://github.com/microsoft/winget-pkgs) submission.

### Manual download

Download `mollow-x86_64-pc-windows-msvc.zip` from GitHub Releases, extract `mollow.exe`, and add
its folder to `PATH`.

## Release assets

Tag `v*` pushes trigger [`.github/workflows/release.yml`](../.github/workflows/release.yml),
which publishes prebuilt binaries for macOS, Linux, and Windows and updates
[homebrew-tap](https://github.com/ingeniousfrog/homebrew-tap) when `HOMEBREW_TAP_TOKEN`
is configured (see [homebrew.md](homebrew.md)).

After each release, maintainers should update Scoop/winget checksum placeholders in
`packaging/scoop/` and `packaging/winget/`:

```bash
./packaging/update-package-checksums.sh <version>
```

Homebrew tap manual fallback:

```bash
./packaging/update-homebrew-sha256.sh <version>
./packaging/push-homebrew-tap.sh
```
