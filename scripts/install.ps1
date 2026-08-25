# PowerShell Install Script for blnk
# Installs blnk binary, sets user PATH, and supports update flags.
#
# Usage:
#   irm https://raw.githubusercontent.com/bl1nk-bot/blnk/main/scripts/install.ps1 | iex
#   # With auto-update mode:
#   & .\scripts\install.ps1 -AutoUpdate
#   # Manual update:
#   & .\scripts\install.ps1 -Update

[CmdletBinding()]
param(
    [switch]$AutoUpdate,
    [switch]$Update,
    [string]$TargetDir = "$HOME\.blnk\bin",
    [string]$Repo = "bl1nk-bot/blnk"
)

$ErrorActionPreference = "Stop"

Write-Host "==> Checking latest blnk release..." -ForegroundColor Cyan

$releaseUrl = "https://api.github.com/repos/$Repo/releases/latest"
try {
    $release = Invoke-RestMethod -Uri $releaseUrl -Headers @{ "User-Agent" = "blnk-installer" }
} catch {
    Write-Error "Failed to fetch latest release from $releaseUrl : $_"
    exit 1
}

$tag = $release.tag_name
$targetAsset = $release.assets | Where-Object { $_.name -like "*x86_64-pc-windows-msvc.zip" } | Select-Object -First 1

if (-not $targetAsset) {
    Write-Error "Could not find Windows asset (*x86_64-pc-windows-msvc.zip) in release $tag"
    exit 1
}

$zipUrl = $targetAsset.browser_download_url
$tempZip = Join-Path $env:TEMP "blnk-latest.zip"
$tempExtract = Join-Path $env:TEMP "blnk-extract"

Write-Host "==> Downloading blnk $tag from $zipUrl..." -ForegroundColor Cyan
Invoke-WebRequest -Uri $zipUrl -OutFile $tempZip -UseBasicParsing

if (Test-Path $tempExtract) {
    Remove-Item -Recurse -Force $tempExtract
}
New-Item -ItemType Directory -Force -Path $tempExtract | Out-Null
Expand-Archive -Path $tempZip -DestinationPath $tempExtract -Force

$exeFile = Get-ChildItem -Path $tempExtract -Filter "blnk.exe" -Recurse | Select-Object -First 1
if (-not $exeFile) {
    Write-Error "blnk.exe not found in extracted archive."
    exit 1
}

if (-not (Test-Path $TargetDir)) {
    New-Item -ItemType Directory -Force -Path $TargetDir | Out-Null
}

$destExe = Join-Path $TargetDir "blnk.exe"
Copy-Item -Path $exeFile.FullName -Destination $destExe -Force
Remove-Item -Force $tempZip
Remove-Item -Recurse -Force $tempExtract

Write-Host "==> Installed blnk to $destExe" -ForegroundColor Green

# Add to User PATH if not already present
$userPath = [Environment]::GetEnvironmentVariable("PATH", "User")
$normalizedTarget = [System.IO.Path]::GetFullPath($TargetDir).TrimEnd('\')
$paths = $userPath -split ';' | ForEach-Object { [System.IO.Path]::GetFullPath($_).TrimEnd('\') }

if ($paths -notcontains $normalizedTarget) {
    Write-Host "==> Adding $TargetDir to User PATH..." -ForegroundColor Cyan
    $newPath = if ([string]::IsNullOrWhiteSpace($userPath)) { $TargetDir } else { "$userPath;$TargetDir" }
    [Environment]::SetEnvironmentVariable("PATH", $newPath, "User")
    $env:PATH = "$env:PATH;$TargetDir"
    Write-Host "==> PATH updated successfully." -ForegroundColor Green
} else {
    Write-Host "==> $TargetDir is already in User PATH." -ForegroundColor DarkGray
}

# Save Update Configuration
$configDir = "$HOME\.blnk"
$updateConfigFile = Join-Path $configDir "update_config.json"
$updateMode = if ($AutoUpdate) { "auto" } else { "manual" }

$updateSettings = [PSCustomObject]@{
    update_mode = $updateMode
    current_version = $tag
    repo = $Repo
    installed_at = (Get-Date).ToString("o")
}

$updateSettings | ConvertTo-Json | Set-Content -Path $updateConfigFile -Force

Write-Host "==> Update mode set to: $updateMode (default is manual)" -ForegroundColor Yellow
Write-Host "==> Installation complete! Run 'blnk --version' to get started." -ForegroundColor Green
