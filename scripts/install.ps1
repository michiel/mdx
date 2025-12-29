# MDX installer for Windows
# Usage: iwr -useb https://raw.githubusercontent.com/michiel/mdx/main/install.ps1 | iex

$ErrorActionPreference = 'Stop'

Write-Host "Downloading latest mdx release for Windows..." -ForegroundColor Cyan

# Get latest release info
$latestUrl = "https://api.github.com/repos/michiel/mdx/releases/latest"
try {
    $release = Invoke-RestMethod -Uri $latestUrl
} catch {
    Write-Host "Error: Could not fetch latest release information" -ForegroundColor Red
    Write-Host $_.Exception.Message -ForegroundColor Red
    exit 1
}

# Find Windows asset
$asset = $release.assets | Where-Object { $_.name -eq "mdx-windows-x86_64.exe" }
if (-not $asset) {
    Write-Host "Error: Could not find Windows binary in latest release" -ForegroundColor Red
    exit 1
}

$downloadUrl = $asset.browser_download_url
Write-Host "Downloading from: $downloadUrl" -ForegroundColor Gray

# Determine install directory
$installDir = "$env:LOCALAPPDATA\mdx"
if (-not (Test-Path $installDir)) {
    New-Item -ItemType Directory -Path $installDir -Force | Out-Null
}

$installPath = Join-Path $installDir "mdx.exe"

# Download binary
try {
    Invoke-WebRequest -Uri $downloadUrl -OutFile $installPath
} catch {
    Write-Host "Error: Download failed" -ForegroundColor Red
    Write-Host $_.Exception.Message -ForegroundColor Red
    exit 1
}

Write-Host ""
Write-Host "✓ mdx installed successfully to $installPath" -ForegroundColor Green
Write-Host ""

# Check if install dir is in PATH
$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($userPath -notlike "*$installDir*") {
    Write-Host "Adding $installDir to PATH..." -ForegroundColor Yellow
    [Environment]::SetEnvironmentVariable(
        "Path",
        "$userPath;$installDir",
        "User"
    )
    Write-Host "✓ PATH updated. Restart your terminal for changes to take effect." -ForegroundColor Green
    Write-Host ""
    Write-Host "To use mdx immediately in this session, run:" -ForegroundColor Cyan
    Write-Host "  `$env:Path += `;$installDir`" -ForegroundColor White
} else {
    Write-Host "Run 'mdx --help' to get started" -ForegroundColor Cyan
}
