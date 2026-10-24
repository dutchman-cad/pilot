$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$root = Join-Path ([IO.Path]::GetTempPath()) ('pilot-install-test-' + [Guid]::NewGuid().ToString('N'))
$originalPath = $env:PATH
function Assert-Same([string]$Left, [string]$Right) {
    if ((Get-FileHash -LiteralPath $Left).Hash -ne (Get-FileHash -LiteralPath $Right).Hash) {
        throw "Files differ: $Left and $Right"
    }
}
function Invoke-Install([string]$Name) {
    & (Join-Path $root 'archive\install.ps1') -Prefix (Join-Path $root $Name)
}
try {
    [void][IO.Directory]::CreateDirectory((Join-Path $root 'archive'))
    [void][IO.Directory]::CreateDirectory((Join-Path $root 'path'))
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot '..\install.ps1') -Destination (Join-Path $root 'archive')
    $fixture = Join-Path $PSScriptRoot 'install\fixture.rs'
    foreach ($kind in @('new', 'full', 'old')) {
        $flags = @()
        if ($kind -eq 'full') { $flags = @('--cfg', 'full_dutchman') }
        if ($kind -eq 'old') { $flags = @('--cfg', 'old_launcher') }
        & rustc --edition=2021 $fixture @flags -o (Join-Path $root "$kind.exe")
        if ($LASTEXITCODE -ne 0) { throw "Cannot build $kind fixture" }
    }
    Copy-Item -LiteralPath (Join-Path $root 'new.exe') -Destination (Join-Path $root 'archive\dutchman.exe')
    Copy-Item -LiteralPath (Join-Path $root 'new.exe') -Destination (Join-Path $root 'archive\dutchman-pilot.exe')
    # All tools used below are PowerShell commands or absolute paths.
    $env:PATH = Join-Path $root 'path'
    Invoke-Install 'empty'
    Assert-Same (Join-Path $root 'new.exe') (Join-Path $root 'empty\dutchman.exe')
    Assert-Same (Join-Path $root 'new.exe') (Join-Path $root 'empty\dutchman-pilot.exe')
    Invoke-Install 'empty'
    Assert-Same (Join-Path $root 'new.exe') (Join-Path $root 'empty\dutchman.exe')
    foreach ($kind in @('full', 'old')) {
        [void][IO.Directory]::CreateDirectory((Join-Path $root $kind))
        Copy-Item -LiteralPath (Join-Path $root "$kind.exe") -Destination (Join-Path $root "$kind\dutchman.exe")
        Invoke-Install $kind
        $expected = if ($kind -eq 'full') { 'full.exe' } else { 'new.exe' }
        Assert-Same (Join-Path $root $expected) (Join-Path $root "$kind\dutchman.exe")
        Assert-Same (Join-Path $root 'new.exe') (Join-Path $root "$kind\dutchman-pilot.exe")
    }
    Copy-Item -LiteralPath (Join-Path $root 'full.exe') -Destination (Join-Path $root 'path\dutchman.exe')
    Invoke-Install 'path-full'
    if (Test-Path -LiteralPath (Join-Path $root 'path-full\dutchman.exe')) { throw 'Shadowed full Dutchman on PATH' }
    Assert-Same (Join-Path $root 'full.exe') (Join-Path $root 'path\dutchman.exe')
    Copy-Item -LiteralPath (Join-Path $root 'old.exe') -Destination (Join-Path $root 'old\dutchman.exe') -Force
    Invoke-Install 'old'
    Assert-Same (Join-Path $root 'old.exe') (Join-Path $root 'old\dutchman.exe')
    Remove-Item -LiteralPath (Join-Path $root 'path\dutchman.exe')
    $userPathBefore = [Environment]::GetEnvironmentVariable('Path', 'User')
    & (Join-Path $root 'archive\install.ps1') -Prefix (Join-Path $root 'dry run') --dry-run --add-to-path
    if ([Environment]::GetEnvironmentVariable('Path', 'User') -cne $userPathBefore) { throw 'Dry run changed user PATH' }
    if (Test-Path -LiteralPath (Join-Path $root 'dry run')) { throw 'Dry run wrote files' }
    Invoke-Install 'with spaces'
    Assert-Same (Join-Path $root 'new.exe') (Join-Path $root 'with spaces\dutchman.exe')
    Write-Output 'PowerShell installer tests passed.'
} finally {
    $env:PATH = $originalPath
    if (Test-Path -LiteralPath $root) { Remove-Item -LiteralPath $root -Recurse -Force }
}
