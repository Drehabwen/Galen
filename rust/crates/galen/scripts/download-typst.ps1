# Download Typst binary for Windows and place it in the sidecar location.
# Run this before `cargo tauri build` to bundle Typst with Galen.
#
# Usage: .\scripts\download-typst.ps1

$ErrorActionPreference = "Stop"

$version = "0.13.1"
$base = "https://github.com/typst/typst/releases/download/v$version"
$zip = "typst-x86_64-pc-windows-msvc.zip"
$url = "$base/$zip"
$targetDir = Join-Path $PSScriptRoot "..\src-tauri\binaries"
$targetExe = Join-Path $targetDir "typst-x86_64-pc-windows-msvc.exe"

# Ensure target directory exists
New-Item -ItemType Directory -Force -Path $targetDir | Out-Null

if (Test-Path $targetExe) {
    Write-Host "✓ Typst already exists at $targetExe"
    exit 0
}

Write-Host "↓ Downloading Typst v$version for Windows..."
$tempZip = Join-Path $env:TEMP $zip
Invoke-WebRequest -Uri $url -OutFile $tempZip

Write-Host "📦 Extracting..."
$tempDir = Join-Path $env:TEMP "typst-extract"
New-Item -ItemType Directory -Force -Path $tempDir | Out-Null
Expand-Archive -Path $tempZip -DestinationPath $tempDir -Force

# The zip contains a single `typst-x86_64-pc-windows-msvc` dir with `typst.exe` inside
$extractedExe = Get-ChildItem -Recurse -Filter "typst.exe" -Path $tempDir | Select-Object -First 1
if (-not $extractedExe) {
    Write-Error "Could not find typst.exe in extracted archive"
    exit 1
}

Move-Item -Path $extractedExe.FullName -Destination $targetExe -Force

# Cleanup
Remove-Item -Path $tempDir -Recurse -Force
Remove-Item -Path $tempZip -Force

Write-Host "✓ Typst installed at $targetExe"
Write-Host "  $(Get-Item $targetExe | Select-Object -ExpandProperty Length) bytes"
