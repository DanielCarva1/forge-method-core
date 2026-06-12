param(
  [string]$PluginParent,
  [string]$MarketplacePath
)

$ErrorActionPreference = "Stop"

if (-not $MarketplacePath) {
  $MarketplacePath = Join-Path $HOME ".agents\plugins\marketplace.json"
}

$pluginName = "forge-method-core"
$repoRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)

function Get-MarketplaceRoot {
  param([Parameter(Mandatory=$true)][string]$Path)
  $fullPath = [System.IO.Path]::GetFullPath($Path)
  $fileName = [System.IO.Path]::GetFileName($fullPath)
  $pluginsDir = Split-Path -Parent $fullPath
  $agentsDir = Split-Path -Parent $pluginsDir
  $candidateRoot = Split-Path -Parent $agentsDir
  if (
    $fileName -eq "marketplace.json" -and
    (Split-Path -Leaf $pluginsDir) -eq "plugins" -and
    (Split-Path -Leaf $agentsDir) -eq ".agents"
  ) {
    return $candidateRoot
  }
  return (Split-Path -Parent $fullPath)
}

$marketplaceRoot = Get-MarketplaceRoot -Path $MarketplacePath
if (-not $PluginParent) {
  $PluginParent = Join-Path $marketplaceRoot "plugins"
}
$target = Join-Path $PluginParent $pluginName

function Assert-ChildPath {
  param(
    [Parameter(Mandatory=$true)][string]$Parent,
    [Parameter(Mandatory=$true)][string]$Child
  )
  $parentResolved = [System.IO.Path]::GetFullPath($Parent).TrimEnd([System.IO.Path]::DirectorySeparatorChar)
  $childResolved = [System.IO.Path]::GetFullPath($Child)
  $prefix = $parentResolved + [System.IO.Path]::DirectorySeparatorChar
  if (-not $childResolved.StartsWith($prefix, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "Refusing to write outside parent directory: $Child"
  }
}

function Get-RelativeChildPath {
  param(
    [Parameter(Mandatory=$true)][string]$Parent,
    [Parameter(Mandatory=$true)][string]$Child
  )
  $separator = [System.IO.Path]::DirectorySeparatorChar
  $parentResolved = [System.IO.Path]::GetFullPath($Parent).TrimEnd($separator) + $separator
  $childResolved = [System.IO.Path]::GetFullPath($Child)
  $parentUri = New-Object System.Uri($parentResolved)
  $childUri = New-Object System.Uri($childResolved)
  return [System.Uri]::UnescapeDataString($parentUri.MakeRelativeUri($childUri).ToString()).Replace("\", "/")
}

function Clear-ReadOnly {
  param([Parameter(Mandatory=$true)][string]$Path)
  if (-not (Test-Path -LiteralPath $Path)) {
    return
  }
  Get-ChildItem -LiteralPath $Path -Recurse -Force | ForEach-Object {
    $child = $_
    try {
      $child.Attributes = $child.Attributes -band (-bnot [System.IO.FileAttributes]::ReadOnly)
    } catch {
      Write-Verbose "Could not clear read-only attribute for $($child.FullName): $($_.Exception.Message)"
    }
  }
  $item = Get-Item -LiteralPath $Path -Force
  $item.Attributes = $item.Attributes -band (-bnot [System.IO.FileAttributes]::ReadOnly)
}

Assert-ChildPath -Parent $PluginParent -Child $target
Assert-ChildPath -Parent $marketplaceRoot -Child $target
New-Item -ItemType Directory -Force -Path $PluginParent | Out-Null

if (Test-Path -LiteralPath $target) {
  Clear-ReadOnly -Path $target
  Remove-Item -LiteralPath $target -Recurse -Force
}
New-Item -ItemType Directory -Force -Path $target | Out-Null

$entries = @(
  ".codex-plugin",
  "assets",
  "docs",
  "examples",
  "release-notes",
  "scripts",
  "skills",
  "templates",
  "AGENTS.md",
  "CHANGELOG.md",
  "CONTEXT.md",
  "install.ps1",
  "install.sh",
  "README.md",
  "VERSION"
)

foreach ($entry in $entries) {
  $source = Join-Path $repoRoot $entry
  if (Test-Path -LiteralPath $source) {
    Copy-Item -LiteralPath $source -Destination $target -Recurse -Force
  }
}

$relativeTarget = Get-RelativeChildPath -Parent $marketplaceRoot -Child $target
if ($relativeTarget.StartsWith("..", [System.StringComparison]::Ordinal) -or [System.IO.Path]::IsPathRooted($relativeTarget)) {
  throw "Plugin target must be inside the marketplace root: $target"
}
$sourcePath = "./$relativeTarget"

$marketplaceDir = Split-Path -Parent $MarketplacePath
New-Item -ItemType Directory -Force -Path $marketplaceDir | Out-Null

if (Test-Path -LiteralPath $MarketplacePath) {
  $marketplace = Get-Content -LiteralPath $MarketplacePath -Raw | ConvertFrom-Json
} else {
  $marketplace = [pscustomobject]@{
    name = "personal"
    interface = [pscustomobject]@{ displayName = "Personal" }
    plugins = @()
  }
}

if (-not $marketplace.name) {
  $marketplace | Add-Member -NotePropertyName name -NotePropertyValue "personal"
}
if (-not $marketplace.interface) {
  $marketplace | Add-Member -NotePropertyName interface -NotePropertyValue ([pscustomobject]@{ displayName = "Personal" })
}
if ($null -eq $marketplace.plugins) {
  $marketplace | Add-Member -NotePropertyName plugins -NotePropertyValue @()
}

$entry = [pscustomobject]@{
  name = $pluginName
  source = [pscustomobject]@{
    source = "local"
    path = $sourcePath
  }
  policy = [pscustomobject]@{
    installation = "AVAILABLE"
    authentication = "ON_INSTALL"
  }
  category = "Productivity"
}

$plugins = @($marketplace.plugins) | Where-Object { $_.name -ne $pluginName }
$marketplace.plugins = @($plugins + $entry)
$marketplace | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $MarketplacePath -Encoding UTF8

Write-Host "Installed local Codex plugin source: $target"
Write-Host "Updated marketplace: $MarketplacePath"
$encodedMarketplacePath = [System.Uri]::EscapeDataString([System.IO.Path]::GetFullPath($MarketplacePath))
Write-Host "Open in Codex: codex://plugins/${pluginName}?marketplacePath=$encodedMarketplacePath"
Write-Host "Share from Codex: codex://plugins/${pluginName}?marketplacePath=$encodedMarketplacePath&mode=share"
$defaultMarketplace = [System.IO.Path]::GetFullPath((Join-Path $HOME ".agents\plugins\marketplace.json"))
$currentMarketplace = [System.IO.Path]::GetFullPath($MarketplacePath)
if ($currentMarketplace -eq $defaultMarketplace) {
  Write-Host "Codex discovers the personal marketplace automatically. Open Codex plugins and select Forge Method Core."
} else {
  Write-Host "Register marketplace: codex plugin marketplace add `"$marketplaceRoot`""
}
