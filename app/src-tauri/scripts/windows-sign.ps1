param(
  [Parameter(Mandatory = $true, Position = 0)]
  [string]$FilePath
)

$ErrorActionPreference = "Stop"

$certPath = $env:WINDOWS_CERTIFICATE_PATH
$certPassword = $env:WINDOWS_CERTIFICATE_PASSWORD

if ([string]::IsNullOrWhiteSpace($certPath) -or [string]::IsNullOrWhiteSpace($certPassword)) {
  Write-Host "Windows signing skipped: WINDOWS_CERTIFICATE_PATH or WINDOWS_CERTIFICATE_PASSWORD is not configured."
  exit 0
}

if (-not (Test-Path -LiteralPath $certPath)) {
  throw "Windows signing certificate was not found at: $certPath"
}

$timestampUrl = $env:WINDOWS_SIGNING_TIMESTAMP_URL
if ([string]::IsNullOrWhiteSpace($timestampUrl)) {
  $timestampUrl = "http://timestamp.digicert.com"
}

$signtoolCommand = Get-Command "signtool.exe" -ErrorAction SilentlyContinue
if ($signtoolCommand) {
  $signtool = $signtoolCommand.Source
} else {
  $windowsKits = Join-Path ${env:ProgramFiles(x86)} "Windows Kits\10\bin"
  $signtool = Get-ChildItem -Path $windowsKits -Recurse -Filter "signtool.exe" -ErrorAction SilentlyContinue |
    Where-Object { $_.FullName -match "\\x64\\signtool\.exe$" } |
    Sort-Object FullName -Descending |
    Select-Object -First 1 -ExpandProperty FullName
}

if ([string]::IsNullOrWhiteSpace($signtool)) {
  throw "signtool.exe was not found. Install the Windows SDK or use a GitHub windows-latest runner."
}

Write-Host "Signing: $FilePath"
& $signtool sign /f $certPath /p $certPassword /fd SHA256 /tr $timestampUrl /td SHA256 $FilePath
if ($LASTEXITCODE -ne 0) {
  exit $LASTEXITCODE
}

& $signtool verify /pa /v $FilePath
if ($LASTEXITCODE -ne 0) {
  exit $LASTEXITCODE
}
