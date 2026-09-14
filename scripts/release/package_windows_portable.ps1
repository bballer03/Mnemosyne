# Package a Windows portable zip from Tauri build output (M21.B).
# Usage (PowerShell):
#   .\scripts\release\package_windows_portable.ps1 -Version 0.3.1 -SourceDir path\to\bundle -OutDir dist
param(
  [Parameter(Mandatory = $true)][string]$Version,
  [Parameter(Mandatory = $true)][string]$SourceDir,
  [Parameter(Mandatory = $true)][string]$OutDir
)

$ErrorActionPreference = "Stop"

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

$exe = Get-ChildItem -Path $SourceDir -Recurse -Filter "Mnemosyne.exe" -ErrorAction SilentlyContinue |
  Where-Object { $_.Name -notmatch 'setup' } |
  Select-Object -First 1
if (-not $exe) {
  # Cargo [[bin]] name is mnemosyne-desktop; Tauri productName is Mnemosyne.
  $exe = Get-ChildItem -Path $SourceDir -Recurse -Filter "mnemosyne-desktop.exe" -ErrorAction SilentlyContinue |
    Select-Object -First 1
}
if (-not $exe) {
  Write-Error "Mnemosyne.exe / mnemosyne-desktop.exe not found under $SourceDir"
  exit 1
}

$stage = Join-Path $OutDir "Mnemosyne-$Version-windows-x64-portable-stage"
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Force -Path $stage | Out-Null

Copy-Item -Path $exe.FullName -Destination (Join-Path $stage "Mnemosyne.exe")
$readme = @"
Mnemosyne portable (Windows x64)
Version: $Version

Goal: unzip this archive and double-click Mnemosyne.exe.
No Java/JVM or developer toolchain required.

Prerequisite: Microsoft Edge WebView2 Runtime (Evergreen).
JVM-free alone is not enough — WebView2 must be present.
Windows 10/11 images that already include WebView2 can unzip-and-click.
If the app fails to start with a WebView/runtime error, install WebView2 from Microsoft, then retry.

This package is labeled: portable with WebView2 prerequisite.
It is not a silent offline single-file drop-in and is not launch-proven from WSL.
"@
Set-Content -Path (Join-Path $stage "README-PORTABLE.txt") -Value $readme -Encoding UTF8

$zipName = "Mnemosyne-$Version-windows-x64-portable.zip"
$zipPath = Join-Path $OutDir $zipName
if (Test-Path $zipPath) { Remove-Item -Force $zipPath }

Compress-Archive -Path (Join-Path $stage "*") -DestinationPath $zipPath -Force
Remove-Item -Recurse -Force $stage

Write-Host "Wrote $zipPath"
