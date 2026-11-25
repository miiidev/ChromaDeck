<#
.SYNOPSIS
  Sync ChromaDeck's version across package.json, package-lock.json,
  src-tauri/Cargo.toml, src-tauri/Cargo.lock, and src-tauri/tauri.conf.json.

.EXAMPLE
  npm run bump -- 0.9.6
  powershell -NoProfile -File scripts/bump-version.ps1 0.9.6 -Tag
  powershell -NoProfile -File scripts/bump-version.ps1 0.9.6 -Push
#>
param(
  [Parameter(Mandatory = $true, Position = 0)]
  [string]$Version,

  [switch]$Tag,
  [switch]$Push,
  [switch]$DryRun
)

$ErrorActionPreference = "Stop"

# Strip optional leading 'v' (e.g. v0.9.6 -> 0.9.6)
$cleanVersion = $Version.Trim() -replace "^[vV]", ""
if ($cleanVersion -notmatch "^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$") {
  throw "Invalid semver version '$Version'. Expected e.g. 0.9.6 or 1.0.0-rc.1"
}

if ($Push) {
  $Tag = $true
}

$root = Split-Path -Parent $PSScriptRoot
$utf8NoBom = [System.Text.UTF8Encoding]::new($false)

$pkgPath = Join-Path $root "package.json"
$lockPath = Join-Path $root "package-lock.json"
$cargoPath = Join-Path $root "src-tauri/Cargo.toml"
$cargoLockPath = Join-Path $root "src-tauri/Cargo.lock"
$tauriConfPath = Join-Path $root "src-tauri/tauri.conf.json"

function Write-Utf8NoBom([string]$Path, [string]$Content) {
  [System.IO.File]::WriteAllText($Path, $Content, $utf8NoBom)
}

# Read current versions
$pkgText = [System.IO.File]::ReadAllText($pkgPath)
$tauriText = [System.IO.File]::ReadAllText($tauriConfPath)
$cargoText = [System.IO.File]::ReadAllText($cargoPath)

$currentPkg = ([regex]::Match($pkgText, '"version"\s*:\s*"([^"]+)"')).Groups[1].Value
$currentTauri = ([regex]::Match($tauriText, '"version"\s*:\s*"([^"]+)"')).Groups[1].Value
$currentCargo = ([regex]::Match($cargoText, '(?ms)\[package\].*?^version\s*=\s*"([^"]+)"')).Groups[1].Value

Write-Host "ChromaDeck version bump -> $cleanVersion"
Write-Host "  package.json:            $currentPkg -> $cleanVersion"
Write-Host "  src-tauri/Cargo.toml:    $currentCargo -> $cleanVersion"
Write-Host "  src-tauri/tauri.conf.json: $currentTauri -> $cleanVersion"

if ($Tag) {
  $tagName = "v$cleanVersion"
  $existing = git -C $root tag -l $tagName
  if ($existing) {
    throw "Git tag '$tagName' already exists."
  }
}

if ($DryRun) {
  Write-Host "[DryRun] No files were modified."
  exit 0
}

# 1. Update package.json (first "version" occurrence only)
$newPkgText = ([regex]'"version"\s*:\s*"[^"]+"').Replace($pkgText, "`"version`": `"$cleanVersion`"", 1)
Write-Utf8NoBom $pkgPath $newPkgText

# 2. Update package-lock.json top-level and root package version if present
if (Test-Path -LiteralPath $lockPath) {
  $lockText = [System.IO.File]::ReadAllText($lockPath)
  # Replace top-level "version" and packages[""].version (first two occurrences)
  $rxLock = [regex]'"version"\s*:\s*"[^"]+"'
  $newLockText = $rxLock.Replace($lockText, "`"version`": `"$cleanVersion`"", 2)
  Write-Utf8NoBom $lockPath $newLockText
}

# 3. Update src-tauri/tauri.conf.json (first "version" occurrence only)
$newTauriText = ([regex]'"version"\s*:\s*"[^"]+"').Replace($tauriText, "`"version`": `"$cleanVersion`"", 1)
Write-Utf8NoBom $tauriConfPath $newTauriText

# 4. Update src-tauri/Cargo.toml ([package] version line only)
$rxCargo = [regex]'(?ms)(\[package\].*?^version\s*=\s*")[^"]+(")'
if (-not $rxCargo.IsMatch($cargoText)) {
  throw "Could not find [package] version in $cargoPath"
}
$newCargoText = $rxCargo.Replace($cargoText, "`${1}$cleanVersion`${2}", 1)
Write-Utf8NoBom $cargoPath $newCargoText

# 5. Update src-tauri/Cargo.lock package entry for chromadeck
if (Test-Path -LiteralPath $cargoLockPath) {
  $cargoLockText = [System.IO.File]::ReadAllText($cargoLockPath)
  $rxCargoLock = [regex]'(?ms)(name\s*=\s*"chromadeck"\r?\nversion\s*=\s*")[^"]+(")'
  if ($rxCargoLock.IsMatch($cargoLockText)) {
    $newCargoLockText = $rxCargoLock.Replace($cargoLockText, "`${1}$cleanVersion`${2}", 1)
    Write-Utf8NoBom $cargoLockPath $newCargoLockText
  }
}

Write-Host "Updated all version files to $cleanVersion."

if ($Tag) {
  $tagName = "v$cleanVersion"
  git -C $root add package.json src-tauri/Cargo.toml src-tauri/tauri.conf.json
  if (Test-Path -LiteralPath $lockPath) { git -C $root add package-lock.json }
  if (Test-Path -LiteralPath $cargoLockPath) { git -C $root add src-tauri/Cargo.lock }
  git -C $root commit -m "release: $tagName"
  if ($LASTEXITCODE -ne 0) { throw "git commit failed." }
  git -C $root tag -a $tagName -m "ChromaDeck $tagName"
  if ($LASTEXITCODE -ne 0) { throw "git tag failed." }
  Write-Host "Created commit and tag $tagName."

  if ($Push) {
    $branch = (git -C $root rev-parse --abbrev-ref HEAD).Trim()
    git -C $root push origin $branch
    if ($LASTEXITCODE -ne 0) { throw "git push branch failed." }
    git -C $root push origin $tagName
    if ($LASTEXITCODE -ne 0) { throw "git push tag failed." }
    Write-Host "Pushed $branch and $tagName to origin. GitHub Release workflow triggered."
  } else {
    Write-Host "Next: run 'git push origin HEAD && git push origin $tagName' to trigger the release workflow."
  }
}
