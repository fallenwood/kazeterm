#requires -Version 7.0
<#
.SYNOPSIS
Checks workspace dependency boundaries using Cargo metadata and PowerShell only.
#>
[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$script:AllowedDependencies = [ordered]@{
  'config' = @()
  'kazeterm-ui-tree' = @()
  'kazeterm-event-system' = @()
  'themeing' = @('config')
  'terminal-kernel' = @()
  'terminal-kernel-alacritty' = @('terminal-kernel')
  'terminal-kernel-vte' = @('terminal-kernel')
  'terminal' = @('config', 'themeing', 'terminal-kernel')
  'kazeterm' = @(
    'config', 'themeing', 'kazeterm-ui-tree', 'kazeterm-event-system',
    'terminal', 'terminal-kernel', 'terminal-kernel-alacritty', 'terminal-kernel-vte'
  )
}
$script:HeadlessPackages = @(
  'terminal-kernel', 'terminal-kernel-alacritty', 'terminal-kernel-vte', 'kazeterm-ui-tree'
)

function Find-DependencyPath {
  param([Parameter(Mandatory)][string]$Manifest)

  # Cargo validates TOML. This lexer only locates explicit dependency path keys,
  # skipping comments/string values and distinguishing tables from value arrays.
  $pattern = @'
(?xs)
  (?<Comment> \#[^\r\n]* )
| (?<String> """(?:\\.|(?!""")[^\\])*""" | '''.*?''' | "(?:\\.|[^"\\\r\n])*" | '[^'\r\n]*' )
| (?<Newline> \r?\n )
| (?<Key> [A-Za-z0-9_-]+ )
| (?<Symbol> [\[\]{}=.] )
'@
  $tokens = [regex]::Matches($Manifest, $pattern)
  $inDependencies = $false
  $perDependency = $false
  $lineStart = $true
  $braces = 0
  $arrays = 0
  $previous = ''

  for ($i = 0; $i -lt $tokens.Count; $i++) {
    $token = $tokens[$i]
    $value = $token.Value
    if ($token.Groups['Comment'].Success) { continue }
    if ($token.Groups['Newline'].Success) {
      $lineStart = $true
      continue
    }
    if ($lineStart -and $value -eq '[' -and $braces -eq 0 -and $arrays -eq 0) {
      $keys = [System.Collections.Generic.List[string]]::new()
      for ($i++; $i -lt $tokens.Count; $i++) {
        $part = $tokens[$i]
        if ($part.Value -eq ']' -or $part.Groups['Newline'].Success) { break }
        if ($part.Groups['Key'].Success -or $part.Groups['String'].Success) {
          $keys.Add($part.Value.Trim([char[]]@('"', "'")))
        }
      }
      if ($i + 1 -lt $tokens.Count -and $tokens[$i + 1].Value -eq ']') { $i++ }
      $offset = if ($keys.Count -ge 3 -and $keys[0] -ceq 'target') { 2 } else { 0 }
      $inDependencies = $keys.Count -gt $offset -and
        @('dependencies', 'dev-dependencies', 'build-dependencies') -ccontains $keys[$offset]
      $perDependency = $inDependencies -and $keys.Count -gt ($offset + 1)
      $lineStart = $false
      $previous = ']'
      continue
    }

    $key = $value.Trim([char[]]@('"', "'"))
    if ($inDependencies -and $key -ceq 'path' -and
        ($perDependency -or $braces -gt 0 -or $previous -eq '.')) {
      $next = $i + 1
      while ($next -lt $tokens.Count -and $tokens[$next].Groups['Comment'].Success) { $next++ }
      if ($next -lt $tokens.Count -and $tokens[$next].Value -eq '=') {
        # Return line numbers, not parsed path values (which Cargo already resolves).
        1 + ([regex]::Matches($Manifest.Substring(0, $token.Index), '\n')).Count
      }
    }
    switch ($value) {
      '{' { $braces++ }
      '}' { $braces-- }
      '[' { $arrays++ }
      ']' { $arrays-- }
    }
    $lineStart = $false
    $previous = $value
  }
}

function Test-WorkspaceBoundaries {
  param([Parameter(Mandatory)][AllowEmptyCollection()][object[]]$Packages)

  $violations = [System.Collections.Generic.List[string]]::new()
  $names = @($Packages | ForEach-Object { $_.name })
  foreach ($required in $script:AllowedDependencies.Keys) {
    if ($names -cnotcontains $required) {
      $violations.Add("${required}: missing from workspace members")
    }
  }
  foreach ($package in $Packages) {
    $name = $package.name
    if (-not $script:AllowedDependencies.Contains($name)) {
      $violations.Add("${name}: declare its dependency boundary in scripts/check-workspace.ps1")
      continue
    }
    foreach ($dependency in $package.dependencies) {
      $dependencyName = $dependency.name
      if ($names -ccontains $dependencyName -and
          $script:AllowedDependencies[$name] -cnotcontains $dependencyName) {
        $violations.Add("$name must not depend on $dependencyName")
      }
      if ($script:HeadlessPackages -ccontains $name -and
          $dependencyName.StartsWith('gpui', [StringComparison]::Ordinal)) {
        $violations.Add("$name must remain independent of GPUI ($dependencyName)")
      }
    }
    $manifest = Get-Content -LiteralPath $package.manifest_path -Raw
    foreach ($line in @(Find-DependencyPath -Manifest $manifest)) {
      $violations.Add("${name}: declare dependency paths in workspace.dependencies (line $line)")
    }
  }
  $violations.ToArray()
}

function Invoke-WorkspaceCheck {
  $root = Split-Path -Parent $PSScriptRoot
  Push-Location -LiteralPath $root
  try {
    $json = & cargo metadata --no-deps --locked --format-version 1 | Out-String
    if ($LASTEXITCODE -ne 0) { throw "cargo metadata failed (exit $LASTEXITCODE)" }
  } finally {
    Pop-Location
  }
  $metadata = ConvertFrom-Json -InputObject $json -AsHashtable
  $violations = @(Test-WorkspaceBoundaries -Packages $metadata.packages)
  if ($violations.Count -gt 0) {
    throw ("Workspace boundary violations:`n- " + ($violations -join "`n- "))
  }
  Write-Host 'Workspace dependency boundaries are valid.'
}

# Dot-sourcing exposes the same checker to dependency-free regression tests.
if ($MyInvocation.InvocationName -ne '.') {
  try {
    Invoke-WorkspaceCheck
  } catch {
    [Console]::Error.WriteLine($_.Exception.Message)
    exit 1
  }
}
