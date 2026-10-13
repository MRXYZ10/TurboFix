param(
    [switch]$BuildInstaller
)

$ErrorActionPreference = 'Stop'
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Push-Location $RepoRoot

function Invoke-Check {
    param(
        [Parameter(Mandatory = $true)][string]$Label,
        [Parameter(Mandatory = $true)][string]$Executable,
        [Parameter(Mandatory = $true)][string[]]$Arguments
    )

    Write-Host "`n==> $Label" -ForegroundColor Cyan
    & $Executable @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "$Label failed with exit code $LASTEXITCODE. Fix the reported error before continuing."
    }
}

try {
    foreach ($tool in @('node', 'npm', 'cargo', 'rustc', 'rustfmt')) {
        if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
            throw "Required tool '$tool' was not found on PATH. Install Node.js 22 and the Rust stable MSVC toolchain, then reopen PowerShell."
        }
    }

    Invoke-Check 'Install locked frontend dependencies' 'npm' @('ci')
    Invoke-Check 'Frontend lint' 'npm' @('run', 'lint')
    Invoke-Check 'Frontend type check' 'npm' @('run', 'typecheck')
    Invoke-Check 'Frontend tests' 'npm' @('test')
    Invoke-Check 'Rust formatting check' 'cargo' @('fmt', '--all', '--', '--check')
    Invoke-Check 'Rust Clippy' 'cargo' @('clippy', '--workspace', '--all-targets', '--', '-D', 'warnings')
    Invoke-Check 'Rust tests' 'cargo' @('test', '--workspace')

    if ($BuildInstaller) {
        Invoke-Check 'Build unsigned Windows installer' 'npm' @('run', 'tauri', '--', 'build')
        Write-Host "`nInstaller build completed. Check target\release\bundle\nsis\ (or the target directory reported by Cargo)." -ForegroundColor Green
    }
    else {
        Write-Host "`nAll verification commands completed. Run this script with -BuildInstaller to build the NSIS installer." -ForegroundColor Green
    }
}
finally {
    Pop-Location
}
