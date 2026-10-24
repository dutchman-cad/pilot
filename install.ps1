# Install the two commands from an extracted Pilot archive.
[CmdletBinding()]
param(
    [string]$Prefix = (Join-Path $env:LOCALAPPDATA 'Programs\dutchman-cad\bin'),
    [Alias('dry-run', '-dry-run')][switch]$DryRun,
    [Alias('add-to-path', '-add-to-path')][switch]$AddToPath
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$Prefix = [IO.Path]::GetFullPath($Prefix)
foreach ($name in @('dutchman-pilot.exe', 'dutchman.exe')) {
    if (-not (Test-Path -LiteralPath (Join-Path $PSScriptRoot $name) -PathType Leaf)) {
        throw "Missing executable beside installer: $name"
    }
}
function Test-Launcher([string]$Path) {
    $process = [Diagnostics.Process]::new()
    $process.StartInfo.FileName = $Path
    $process.StartInfo.Arguments = '--version'
    $process.StartInfo.UseShellExecute = $false
    $process.StartInfo.CreateNoWindow = $true
    $process.StartInfo.RedirectStandardOutput = $true
    $process.StartInfo.RedirectStandardError = $true
    try {
        if (-not $process.Start()) { return $false }
        # Bound the first line to 4096 characters and the entire probe to three seconds.
        $clock = [Diagnostics.Stopwatch]::StartNew()
        $line = [Text.StringBuilder]::new()
        $buffer = [char[]]::new(1)
        while ($line.Length -lt 4096) {
            $read = $process.StandardOutput.ReadAsync($buffer, 0, 1)
            $remaining = 3000 - [int]$clock.ElapsedMilliseconds
            if ($remaining -le 0 -or -not $read.Wait($remaining)) { return $false }
            if ($read.Result -eq 0 -or $buffer[0] -eq "`n") { break }
            [void]$line.Append($buffer[0])
        }
        $remaining = 3000 - [int]$clock.ElapsedMilliseconds
        if ($remaining -le 0 -or -not $process.WaitForExit($remaining)) { return $false }
        return $process.ExitCode -eq 0 -and $line.ToString().StartsWith('dutchman launcher', [StringComparison]::Ordinal)
    } catch {
        return $false
    } finally {
        try { if (-not $process.HasExited) { $process.Kill() } } catch { }
        $process.Dispose()
    }
}
$keepFull = $false
$destination = Join-Path $Prefix 'dutchman.exe'
if (Test-Path -LiteralPath $destination) {
    $keepFull = -not (Test-Launcher $destination)
}
$entries = @($env:PATH -split ';')
if ($entries.Count -gt 256) { throw 'PATH exceeds 256 entries' }
foreach ($entry in $entries) {
    if ([string]::IsNullOrWhiteSpace($entry)) { continue }
    $candidate = Join-Path ($entry.Trim('"')) 'dutchman.exe'
    if ((Test-Path -LiteralPath $candidate) -and -not (Test-Launcher $candidate)) {
        $keepFull = $true
    }
}
foreach ($name in @('dutchman-pilot.exe', 'dutchman.exe')) {
    if ($name -eq 'dutchman.exe' -and $keepFull) {
        Write-Output 'Existing Dutchman kept; the existing Dutchman will run dutchman pilot.'
        continue
    }
    $source = Join-Path $PSScriptRoot $name
    $target = Join-Path $Prefix $name
    if ($source -eq $target) {
        Write-Output "Already installed: $target"
    } elseif ($DryRun) {
        Write-Output "Would install: $target"
    } else {
        [void][IO.Directory]::CreateDirectory($Prefix)
        if (Test-Path -LiteralPath $target) { Remove-Item -LiteralPath $target -Force }
        Copy-Item -LiteralPath $source -Destination $target
        Write-Output "Installed: $target"
    }
}
if ($AddToPath) {
    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    if (@($userPath -split ';') -notcontains $Prefix) {
        if ($DryRun) {
            Write-Output "Would add to user PATH: $Prefix"
        } else {
            $updated = if ([string]::IsNullOrEmpty($userPath)) { $Prefix } else { "$userPath;$Prefix" }
            [Environment]::SetEnvironmentVariable('Path', $updated, 'User')
            Write-Output "Added to user PATH: $Prefix (open a new terminal)."
        }
    } else {
        Write-Output "Already on user PATH: $Prefix"
    }
}
Write-Output "PATH hint: add $Prefix to PATH, or use -AddToPath and open a new terminal."
