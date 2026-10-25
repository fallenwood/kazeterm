#requires -Version 7.0
# Dependency-free regression tests; no Pester module installation is required.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'check-workspace.ps1')

function Assert-True {
  param([bool]$Condition, [string]$Message)
  if (-not $Condition) { throw $Message }
}

function New-FixturePackages {
  foreach ($entry in $script:AllowedDependencies.GetEnumerator()) {
    @{
      name = $entry.Key
      manifest_path = $script:FixtureManifest
      dependencies = @($entry.Value | ForEach-Object { @{ name = $_ } })
    }
  }
}

function Invoke-RegressionTest {
  param([string]$Name, [scriptblock]$Body)
  try {
    Set-Content -LiteralPath $script:FixtureManifest -Value '[package]' -Encoding utf8
    & $Body
    Write-Host "PASS: $Name"
  } catch {
    $script:Failures++
    [Console]::Error.WriteLine("FAIL: ${Name}: $($_.Exception.Message)")
  }
}

$script:Failures = 0
$directory = Join-Path ([IO.Path]::GetTempPath()) "kazeterm-workspace-$([guid]::NewGuid())"
$null = New-Item -ItemType Directory -Path $directory
$script:FixtureManifest = Join-Path $directory 'Cargo.toml'
try {
  Invoke-RegressionTest 'Allowed dependency graph' {
    $violations = @(Test-WorkspaceBoundaries -Packages @(New-FixturePackages))
    Assert-True ($violations.Count -eq 0) ($violations -join '; ')
  }

  Invoke-RegressionTest 'Headless packages reject UI/config dependencies' {
    foreach ($edge in @(
      @('terminal-kernel', 'themeing'),
      @('terminal-kernel-alacritty', 'terminal'),
      @('terminal-kernel-vte', 'config'),
      @('terminal-kernel', 'gpui-fast'),
      @('kazeterm-ui-tree', 'gpui-kit')
    )) {
      $packages = @(New-FixturePackages)
      $package = $packages | Where-Object { $_.name -ceq $edge[0] }
      $package.dependencies += @{ name = $edge[1] }
      $violations = @(Test-WorkspaceBoundaries -Packages $packages)
      Assert-True (@($violations | Where-Object { $_.Contains($edge[1]) }).Count -gt 0) (
        "Did not reject $($edge[0]) -> $($edge[1])"
      )
    }
  }

  Invoke-RegressionTest 'Member paths are rejected in every dependency section' {
    foreach ($section in @(
      'dependencies',
      'dev-dependencies',
      'build-dependencies',
      'target.''cfg(target_os = "windows")''.dependencies',
      'target.''cfg(target_os = "windows")''.dev-dependencies',
      'target.''cfg(target_os = "windows")''.build-dependencies'
    )) {
      foreach ($declaration in @(
        "[$section]`nterminal-kernel = { path = '../core' }",
        "[$section.terminal-kernel]`npath = '../core'",
        "[$section]`nterminal-kernel.path = '../core'",
        "[$section]`nterminal-kernel = {`n  'path' = '../core'`n}"
      )) {
        Set-Content -LiteralPath $script:FixtureManifest -Value $declaration -Encoding utf8
        $violations = @(Test-WorkspaceBoundaries -Packages @(New-FixturePackages))
        Assert-True (@($violations | Where-Object { $_.Contains('workspace.dependencies') }).Count -gt 0) (
          "Did not reject member path in $declaration"
        )
      }
    }
  }

  Invoke-RegressionTest 'Comments, string values, source paths, and workspace inheritance are allowed' {
    $manifest = @'
[package]
description = '''
[dependencies]
example = { path = "not a dependency" }
'''
[lib]
path = "src/lib.rs"
[[bin]]
name = "fixture"
path = "src/main.rs"
[dependencies]
# terminal-kernel = { path = "not active" }
terminal-kernel = { workspace = true, features = [
  "path", "text containing path = '../core'"
] }
path = "1.0"
[dependencies.example]
version = 'path = "only text"'
[target.'cfg(target_os = "windows")'.dependencies]
terminal-kernel.workspace = true
'@
    Set-Content -LiteralPath $script:FixtureManifest -Value $manifest -Encoding utf8
    $violations = @(Test-WorkspaceBoundaries -Packages @(New-FixturePackages))
    Assert-True ($violations.Count -eq 0) ($violations -join '; ')

    # A real path after an array-table header must still be found.
    Add-Content -LiteralPath $script:FixtureManifest -Value "[dev-dependencies]`nexample = { path = '../core' }"
    $violations = @(Test-WorkspaceBoundaries -Packages @(New-FixturePackages))
    Assert-True ($violations.Count -gt 0) 'Lost dependency table context after [[bin]]'
  }

  Invoke-RegressionTest 'Missing and unknown members require an explicit boundary' {
    $packages = @(New-FixturePackages | Where-Object { $_.name -cne 'terminal-kernel' })
    $packages += @{ name = 'unknown'; manifest_path = $script:FixtureManifest; dependencies = @() }
    $violations = @(Test-WorkspaceBoundaries -Packages $packages)
    Assert-True (@($violations | Where-Object { $_.Contains('terminal-kernel: missing') }).Count -gt 0) (
      'Missing workspace member was not reported'
    )
    Assert-True (@($violations | Where-Object { $_.Contains('unknown: declare') }).Count -gt 0) (
      'Unknown workspace member was not reported'
    )
  }
} finally {
  Remove-Item -LiteralPath $directory -Recurse -Force
}

if ($script:Failures -gt 0) {
  [Console]::Error.WriteLine("$script:Failures workspace checker test(s) failed.")
  exit 1
}
Write-Host 'All 5 workspace checker regression tests passed.'
