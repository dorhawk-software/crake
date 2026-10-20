# Crake for Windows on ARM64, installed as a person would install it (CI's
# windows-arm64-install job, on a windows-11-arm runner): the programs are
# ARM64, the service runs and serves the app, openconnect and OpenVPN
# start, OpenVPN's drivers are installed, and removing it leaves nothing.
# No gateway: the x64 jobs carry those (CLAUDE.md, Windows on ARM64).
#
#   scripts/windows-arm64-install-test.ps1 <crake-setup-X-arm64.exe>
param([Parameter(Mandatory)] [string] $Setup)
$ErrorActionPreference = 'Stop'
$dir = Join-Path $env:ProgramFiles 'Crake'
# craked's port for the app (sys/windows: 127.0.0.1:47366).
$appPort = 47366

function Wait-Until([scriptblock] $what, [string] $saying, [int] $seconds = 60) {
  $until = (Get-Date).AddSeconds($seconds)
  while (-not (& $what)) {
    if ((Get-Date) -gt $until) { throw "After $seconds s: $saying" }
    Start-Sleep -Seconds 1
  }
}

function Assert-Arm64([string] $path) {
  $b = [System.IO.File]::ReadAllBytes($path)
  $pe = [BitConverter]::ToInt32($b, 0x3c)
  $machine = [BitConverter]::ToUInt16($b, $pe + 4)
  if ($machine -ne 0xAA64) { throw "$path is for machine 0x$('{0:X}' -f $machine), not ARM64" }
}

Write-Host "Installing $Setup"
$p = Start-Process $Setup -ArgumentList '/S' -Wait -PassThru
if ($p.ExitCode -ne 0) { throw "the installer exited $($p.ExitCode)" }

foreach ($f in 'crake.exe', 'craked.exe', 'crake-hook.exe', 'openconnect\openconnect.exe', 'openvpn\bin\openvpn.exe') {
  Assert-Arm64 (Join-Path $dir $f)
}

Wait-Until { (Get-Service craked -ErrorAction SilentlyContinue).Status -eq 'Running' } 'the craked service isn''t running'
Wait-Until { (Test-NetConnection 127.0.0.1 -Port $appPort -WarningAction SilentlyContinue).TcpTestSucceeded } "craked doesn't serve the app on 127.0.0.1:$appPort"
Write-Host 'craked runs and serves the app'

$oc = & (Join-Path $dir 'openconnect\openconnect.exe') --version 2>&1 | Out-String
if ($oc -notmatch 'OpenConnect version') { throw "openconnect didn't start: $oc" }
# openvpn --version exits 1 by design; what it says counts.
$ov = & (Join-Path $dir 'openvpn\bin\openvpn.exe') --version 2>&1 | Out-String
if ($ov -notmatch 'OpenVPN 2\.') { throw "openvpn didn't start: $ov" }
Write-Host 'openconnect and openvpn start'

$drivers = pnputil /enum-drivers | Out-String
foreach ($inf in 'ovpn-dco.inf', 'oemvista.inf') {
  if ($drivers -notmatch [regex]::Escape($inf)) { throw "OpenVPN's driver $inf isn't installed:`n$drivers" }
}
Write-Host 'OpenVPN''s drivers are installed'

Write-Host 'Removing Crake'
# The uninstaller copies itself away and returns at once: wait for its work.
Start-Process (Join-Path $dir 'uninstall.exe') -ArgumentList '/S' -Wait
Wait-Until { -not (Get-Service craked -ErrorAction SilentlyContinue) } 'the craked service is still there'
Wait-Until { -not (Test-Path (Join-Path $dir 'craked.exe')) } "$dir still holds craked.exe"
Write-Host 'Installed, ran and removed on Windows for ARM64'
