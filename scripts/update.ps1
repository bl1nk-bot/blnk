# PowerShell Update Script for blnk
#
# Usage:
#   .\scripts\update.ps1             # Manual update check and apply
#   .\scripts\update.ps1 -SetAuto    # Enable auto update
#   .\scripts\update.ps1 -SetManual  # Enable manual update (default)

[CmdletBinding()]
param(
    [switch]$SetAuto,
    [switch]$SetManual,
    [string]$TargetDir = "$HOME\.blnk\bin",
    [string]$Repo = "bl1nk-bot/blnk"
)

$ErrorActionPreference = "Stop"
$configFile = "$HOME\.blnk\update_config.json"

if ($SetAuto) {
    if (Test-Path $configFile) {
        $cfg = Get-Content $configFile | ConvertFrom-Json
        $cfg.update_mode = "auto"
        $cfg | ConvertTo-Json | Set-Content -Path $configFile -Force
    }
    Write-Host "==> Configured update mode: auto" -ForegroundColor Green
    exit 0
}

if ($SetManual) {
    if (Test-Path $configFile) {
        $cfg = Get-Content $configFile | ConvertFrom-Json
        $cfg.update_mode = "manual"
        $cfg | ConvertTo-Json | Set-Content -Path $configFile -Force
    }
    Write-Host "==> Configured update mode: manual" -ForegroundColor Green
    exit 0
}

# Run Update
$installScript = Join-Path $PSScriptRoot "install.ps1"
if (Test-Path $installScript) {
    & $installScript -Update -TargetDir $TargetDir -Repo $Repo
} else {
    irm "https://raw.githubusercontent.com/$Repo/main/scripts/install.ps1" | iex
}
