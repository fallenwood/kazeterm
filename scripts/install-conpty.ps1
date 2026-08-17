param(
  [Parameter(Mandatory = $true)]
  [ValidateSet("x86_64-pc-windows-msvc", "aarch64-pc-windows-msvc")]
  [string]$TargetTriple,

  [Parameter(Mandatory = $true)]
  [string]$Destination,

  [string]$PackagePath
)

$ErrorActionPreference = "Stop"

$packageVersion = "1.24.260710001"
$packageSha256 = "175640566A3B59C4B132070EE96C2C77E5AB7EDD2E92732A5EB3610BBF63D90E"
$packageUrl = "https://www.nuget.org/api/v2/package/Microsoft.Windows.Console.ConPTY/$packageVersion"
$cacheRoot = Join-Path $PSScriptRoot "..\target\conpty-cache\$packageVersion"
$cachedPackage = Join-Path $cacheRoot "Microsoft.Windows.Console.ConPTY.$packageVersion.nupkg"
$extractedRoot = Join-Path $cacheRoot "package"

New-Item -ItemType Directory -Force -Path $cacheRoot | Out-Null

if ($PackagePath) {
  $package = (Resolve-Path -LiteralPath $PackagePath).Path
} else {
  $package = $cachedPackage
  if (-not (Test-Path -LiteralPath $package)) {
    Invoke-WebRequest -UseBasicParsing -Uri $packageUrl -OutFile $package
  }
}

$actualHash = (Get-FileHash -LiteralPath $package -Algorithm SHA256).Hash
if ($actualHash -ne $packageSha256) {
  if (-not $PackagePath) {
    Remove-Item -LiteralPath $package -Force -ErrorAction SilentlyContinue
  }
  throw "ConPTY package checksum mismatch: expected $packageSha256, got $actualHash"
}

if (-not (Test-Path -LiteralPath $extractedRoot)) {
  New-Item -ItemType Directory -Force -Path $extractedRoot | Out-Null
  Add-Type -AssemblyName System.IO.Compression.FileSystem
  [System.IO.Compression.ZipFile]::ExtractToDirectory($package, $extractedRoot)
}

$destinationPath = (New-Item -ItemType Directory -Force -Path $Destination).FullName
$runtimeArchitecture = switch ($TargetTriple) {
  "x86_64-pc-windows-msvc" { "x64" }
  "aarch64-pc-windows-msvc" { "arm64" }
}

$dll = Join-Path $extractedRoot "runtimes\win-$runtimeArchitecture\native\conpty.dll"
Copy-Item -LiteralPath $dll -Destination (Join-Path $destinationPath "conpty.dll") -Force

$hostArchitectures = if ($runtimeArchitecture -eq "x64") {
  @("x64", "arm64")
} else {
  @("arm64")
}

foreach ($hostArchitecture in $hostArchitectures) {
  $hostDestination = (
    New-Item -ItemType Directory -Force -Path (Join-Path $destinationPath $hostArchitecture)
  ).FullName
  $hostExecutable = Join-Path $extractedRoot "build\native\runtimes\$hostArchitecture\OpenConsole.exe"
  Copy-Item -LiteralPath $hostExecutable -Destination (
    Join-Path $hostDestination "OpenConsole.exe"
  ) -Force
}

Write-Host "Installed Microsoft ConPTY $packageVersion for $TargetTriple in $destinationPath"
