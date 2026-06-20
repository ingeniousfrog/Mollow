#Requires -Version 5.1
param(
    [string]$Version = "0.1.2",
    [string]$Repo = "ingeniousfrog/Mollow",
    [string]$InstallDir = "$env:LOCALAPPDATA\Programs\Mollow\bin"
)

$ErrorActionPreference = "Stop"

function Show-Usage {
    Write-Host @"
Install Mollow from GitHub Releases.

Parameters:
  -Version     Release version without leading v (default: 0.1.2)
  -Repo        GitHub repository (default: ingeniousfrog/Mollow)
  -InstallDir  Install directory (default: %LOCALAPPDATA%\Programs\Mollow\bin)

Example:
  irm https://raw.githubusercontent.com/ingeniousfrog/Mollow/main/packaging/install.ps1 | iex
"@
}

if ($args -contains "-h" -or $args -contains "--help") {
    Show-Usage
    exit 0
}

$target = "x86_64-pc-windows-msvc"
$asset = "mollow-$target.zip"
$url = "https://github.com/$Repo/releases/download/v$Version/$asset"
$tempDir = Join-Path $env:TEMP ("mollow-install-" + [guid]::NewGuid().ToString("N"))
$zipPath = Join-Path $tempDir $asset

try {
    New-Item -ItemType Directory -Force -Path $tempDir, $InstallDir | Out-Null
    Write-Host "Installing mollow v$Version for $target..."
    Invoke-WebRequest -Uri $url -OutFile $zipPath
    Expand-Archive -Path $zipPath -DestinationPath $tempDir -Force
    Copy-Item (Join-Path $tempDir "mollow.exe") (Join-Path $InstallDir "mollow.exe") -Force
    Write-Host "Installed mollow to $InstallDir\mollow.exe"

    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if ($userPath -notlike "*$InstallDir*") {
        [Environment]::SetEnvironmentVariable("Path", "$userPath;$InstallDir", "User")
        Write-Host "Added $InstallDir to the user PATH. Restart your terminal to use mollow."
    }

    & (Join-Path $InstallDir "mollow.exe") --version
}
finally {
    Remove-Item -Recurse -Force $tempDir -ErrorAction SilentlyContinue
}
