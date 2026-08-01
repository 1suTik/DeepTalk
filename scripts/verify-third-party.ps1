#requires -Version 5.1
<#
.SYNOPSIS
验证 THIRD_PARTY_NOTICES.md：每个登记项目包含 URL、许可证、固定版本/commit、复用板块和修改说明；
并交叉校验 package.json 与 Cargo.toml 的全部直接依赖均已登记。

.EXAMPLE
powershell -ExecutionPolicy Bypass -File scripts/verify-third-party.ps1
#>
[CmdletBinding()]
param(
    [string]$RepoRoot = ""
)

$ErrorActionPreference = "Stop"
if (-not $RepoRoot) { $RepoRoot = Split-Path -Parent $PSScriptRoot }
$noticesPath = Join-Path $RepoRoot "THIRD_PARTY_NOTICES.md"
$packagePath = Join-Path $RepoRoot "package.json"
$cargoPath = Join-Path $RepoRoot "src-tauri\Cargo.toml"

$errors = @()

if (-not (Test-Path -LiteralPath $noticesPath)) {
    Write-Error "THIRD_PARTY_NOTICES.md not found: $noticesPath"
    exit 1
}

# ---------- 1. 解析登记表 ----------
$rows = @()
$inTable = $false
$seenHeader = $false
foreach ($line in [System.IO.File]::ReadAllLines($noticesPath)) {
    if ($line -match "^\|") {
        if (-not $seenHeader) {
            $seenHeader = $true
            continue
        }
        if ($line -match "^\|\s*-") { continue }
        $cells = $line.TrimStart("|").TrimEnd("|") -split "\|" | ForEach-Object { $_.Trim() }
        if ($cells.Count -ge 9) {
            $rows += [PSCustomObject]@{
                Name     = $cells[0]
                Purpose  = $cells[1]
                Url      = $cells[2]
                License  = $cells[3]
                Version  = $cells[4]
                Reuse    = $cells[5]
                Mods     = $cells[6]
                Deps     = $cells[7]
                Status   = $cells[8]
            }
        }
    }
}

if ($rows.Count -eq 0) {
    Write-Error "No entries parsed from the notices table."
    exit 1
}

foreach ($row in $rows) {
    $label = "row '$($row.Name)'"
    if (-not $row.Url -or $row.Url -eq "-") { $errors += "${label}: missing URL" }
    elseif ($row.Url -notmatch "^https?://") { $errors += "${label}: URL must start with http(s): $($row.Url)" }
    if (-not $row.License -or $row.License -eq "-") { $errors += "${label}: missing license" }
    if (-not $row.Version -or $row.Version -eq "-") { $errors += "${label}: missing pinned version/commit" }
    if (-not $row.Reuse -or $row.Reuse -eq "-") { $errors += "${label}: missing reused components" }
    if (-not $row.Mods -or $row.Mods -eq "-") { $errors += "${label}: missing modification notes" }
}

# ---------- 2. 交叉校验直接依赖 ----------
function Normalize-Name([string]$name) {
    return ($name.ToLowerInvariant() -replace "[@\-._/]", "")
}

$depAliases = @{}
foreach ($row in $rows) {
    foreach ($alias in ($row.Deps -split ",")) {
        $alias = $alias.Trim()
        if ($alias -and $alias -ne "-") {
            $depAliases[(Normalize-Name $alias)] = $true
        }
    }
}

$missing = @()
if (Test-Path -LiteralPath $packagePath) {
    $pkg = Get-Content -LiteralPath $packagePath -Raw | ConvertFrom-Json
    foreach ($section in @($pkg.dependencies, $pkg.devDependencies)) {
        foreach ($dep in $section.PSObject.Properties.Name) {
            if (-not $depAliases.ContainsKey((Normalize-Name $dep))) {
                $missing += "npm dependency '$dep' (package.json) is not registered in THIRD_PARTY_NOTICES.md"
            }
        }
    }
}

if (Test-Path -LiteralPath $cargoPath) {
    $cargo = Get-Content -LiteralPath $cargoPath -Raw
    foreach ($sectionName in @("[dependencies]", "[build-dependencies]")) {
        $start = $cargo.IndexOf($sectionName, [System.StringComparison]::OrdinalIgnoreCase)
        if ($start -lt 0) { continue }
        $after = $cargo.IndexOf("`n[", $start + $sectionName.Length, [System.StringComparison]::Ordinal)
        if ($after -lt 0) { $after = $cargo.Length }
        $block = $cargo.Substring($start, $after - $start)
        foreach ($m in [regex]::Matches($block, "(?m)^([A-Za-z][A-Za-z0-9_-]*)\s*=")) {
            $dep = $m.Groups[1].Value
            if (-not $depAliases.ContainsKey((Normalize-Name $dep))) {
                $missing += "cargo dependency '$dep' ($sectionName) is not registered in THIRD_PARTY_NOTICES.md"
            }
        }
    }
}

$errors += $missing

# ---------- 3. 输出 ----------
if ($errors.Count -gt 0) {
    Write-Host "Third-party manifest FAILED:" -ForegroundColor Red
    $errors | ForEach-Object { Write-Host "  - $_" -ForegroundColor Red }
    exit 1
}

Write-Host "Third-party manifest OK ($($rows.Count) entries, all direct dependencies registered)." -ForegroundColor Green
exit 0
