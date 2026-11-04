# Lekh (lk) — One-Step Windows Installer (PowerShell + cmd.exe)
# Usage:
#   irm https://github.com/khajumsanjog/Lekh/releases/latest/download/install.ps1 | iex

$ErrorActionPreference = 'Stop'

$Repo       = 'khajumsanjog/Lekh'
$InstallDir = Join-Path $env:LOCALAPPDATA 'Programs\Lekh'
$ZipUrl     = "https://github.com/$Repo/releases/latest/download/lekh-windows-x86_64.zip"
$TmpZip     = Join-Path $env:TEMP "lekh-windows-x86_64-$PID.zip"
$TmpExtract = Join-Path $env:TEMP "lekh-extract-$PID"

Write-Host "==> Downloading Lekh (lk + lekh) from GitHub Releases..." -ForegroundColor Cyan
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
Invoke-WebRequest -Uri $ZipUrl -OutFile $TmpZip -UseBasicParsing

if (Test-Path $TmpExtract) { Remove-Item -Recurse -Force $TmpExtract }
Expand-Archive -Path $TmpZip -DestinationPath $TmpExtract -Force

New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
Copy-Item (Join-Path $TmpExtract 'lk.exe')   $InstallDir -Force
Copy-Item (Join-Path $TmpExtract 'lekh.exe') $InstallDir -Force
if (Test-Path (Join-Path $TmpExtract 'LICENSE'))   { Copy-Item (Join-Path $TmpExtract 'LICENSE')   $InstallDir -Force }
if (Test-Path (Join-Path $TmpExtract 'README.md')) { Copy-Item (Join-Path $TmpExtract 'README.md') $InstallDir -Force }

# Ensure SmartScreen Mark-of-the-Web is cleared on extracted binaries
Unblock-File -Path (Join-Path $InstallDir 'lk.exe')   -ErrorAction SilentlyContinue
Unblock-File -Path (Join-Path $InstallDir 'lekh.exe') -ErrorAction SilentlyContinue

# Cleanup temp files
Remove-Item -Force $TmpZip -ErrorAction SilentlyContinue
Remove-Item -Recurse -Force $TmpExtract -ErrorAction SilentlyContinue

# Add permanently to User PATH (works in PowerShell, Windows Terminal, AND cmd.exe)
$UserPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if ([string]::IsNullOrWhiteSpace($UserPath)) {
    $NewUserPath = $InstallDir
} else {
    $Parts = $UserPath.Split(';', [StringSplitOptions]::RemoveEmptyEntries)
    if ($Parts -notcontains $InstallDir) {
        $NewUserPath = "$UserPath;$InstallDir"
    } else {
        $NewUserPath = $UserPath
    }
}
if ($NewUserPath -ne $UserPath) {
    [Environment]::SetEnvironmentVariable('Path', $NewUserPath, 'User')
    Write-Host "==> Added $InstallDir to Windows User PATH." -ForegroundColor Green
}

# Also add to the current PowerShell session immediately
if (($env:Path.Split(';') -notcontains $InstallDir)) {
    $env:Path = "$env:Path;$InstallDir"
}

Write-Host ""
Write-Host "Lekh installed successfully to $InstallDir!" -ForegroundColor Green
Write-Host "You can now run BOTH commands in PowerShell, Windows Terminal, and cmd.exe:" -ForegroundColor Yellow
Write-Host "    lk index.html"
Write-Host "    lekh index.html"
