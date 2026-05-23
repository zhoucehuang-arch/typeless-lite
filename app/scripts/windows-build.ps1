$ErrorActionPreference = "Stop"

Push-Location (Join-Path $PSScriptRoot "..")
try {
  npm ci
  npm run tauri -- build --target x86_64-pc-windows-msvc

  Write-Host ""
  Write-Host "Windows installer output:"
  Get-ChildItem -Path "src-tauri\target\x86_64-pc-windows-msvc\release\bundle\nsis" -Filter "*setup.exe" -ErrorAction SilentlyContinue |
    ForEach-Object { Write-Host "  $($_.FullName)" }
} finally {
  Pop-Location
}
