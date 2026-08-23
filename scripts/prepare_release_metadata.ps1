[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [ValidateRange(1, [int]::MaxValue)]
    [int]$PrNumber,

    [Parameter(Mandatory)]
    [ValidatePattern('^\d+\.\d+\.\d+$')]
    [string]$BaseVersion,

    [string]$Summary,

    [switch]$Validate
)

$ErrorActionPreference = 'Stop'

function Get-PatchVersion([string]$Version) {
    $parts = $Version.Split('.')
    if ($parts.Count -ne 3) { throw "Invalid semantic version: $Version" }
    return "$($parts[0]).$($parts[1]).$([int]$parts[2] + 1)"
}

function Get-PackageVersion([string]$Content, [string]$Path) {
    $match = [regex]::Match($Content, '(?m)^version = "(?<version>\d+\.\d+\.\d+)"')
    if (-not $match.Success) { throw "Could not find package version in $Path" }
    return $match.Groups['version'].Value
}

$expectedVersion = Get-PatchVersion $BaseVersion
$cargoTomlPath = Join-Path $PSScriptRoot '..\Cargo.toml'
$cargoLockPath = Join-Path $PSScriptRoot '..\Cargo.lock'
$changelogPath = Join-Path $PSScriptRoot '..\CHANGELOG.md'
$cargoToml = [IO.File]::ReadAllText($cargoTomlPath)
$cargoLock = [IO.File]::ReadAllText($cargoLockPath)
$changelog = [IO.File]::ReadAllText($changelogPath)
$heading = "## [$expectedVersion] — PR #$PrNumber"

$lockPattern = '(?ms)(\[\[package\]\]\r?\nname = "blnk"\r?\nversion = ")\d+\.\d+\.\d+("\r?\n)'

if ($Validate) {
    if ((Get-PackageVersion $cargoToml $cargoTomlPath) -ne $expectedVersion) {
        throw "Cargo.toml must be $expectedVersion (one patch after $BaseVersion)."
    }
    if (-not [regex]::IsMatch($cargoLock, $lockPattern.Replace('\d+\.\d+\.\d+', [regex]::Escape($expectedVersion)))) {
        throw "Cargo.lock root package must be $expectedVersion."
    }
    if (-not $changelog.Contains($heading)) {
        throw "CHANGELOG.md must contain '$heading'."
    }
    Write-Output "Release metadata is valid for $heading."
    exit 0
}

if ([string]::IsNullOrWhiteSpace($Summary)) {
    throw 'Summary is required when updating release metadata.'
}

$currentVersion = Get-PackageVersion $cargoToml $cargoTomlPath
if ($currentVersion -ne $BaseVersion) {
    throw "Refusing to update from Cargo.toml $currentVersion; expected base version $BaseVersion."
}
if ($changelog.Contains($heading)) {
    throw "CHANGELOG.md already contains '$heading'."
}

$updatedToml = [regex]::Replace($cargoToml, '(?m)^version = "\d+\.\d+\.\d+"', "version = `"$expectedVersion`"", 1)
$updatedLock = [regex]::Replace($cargoLock, $lockPattern, "`${1}$expectedVersion`${2}", 1)
if ($updatedLock -eq $cargoLock) { throw 'Could not update the blnk root package version in Cargo.lock.' }
$updatedChangelog = [regex]::Replace(
    $changelog,
    '(?m)^## \[0\.1\.',
    "$heading`n- $Summary`n`n## [0.1.",
    1
)
if ($updatedChangelog -eq $changelog) { throw 'Could not insert the changelog entry.' }

[IO.File]::WriteAllText($cargoTomlPath, $updatedToml)
[IO.File]::WriteAllText($cargoLockPath, $updatedLock)
[IO.File]::WriteAllText($changelogPath, $updatedChangelog)
Write-Output "Updated Cargo.toml, Cargo.lock, and CHANGELOG.md for $heading. Run verification before merge."
