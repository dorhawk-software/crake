# Inside scripts/windows-vm-test.sh's Windows VM, started at sign-in by
# launch.ps1 (elevated, in the signed-in session as the app would be):
# install Crake as a person would have it (Program Files\Crake, the craked
# service, openconnect beside it), wait for the gateway VM, then run the
# Windows test (crates/crake/src/windows_tests.rs). Everything said goes to
# the first serial port; the last line says how it went. Then off.
param([string]$Disc)
$ErrorActionPreference = 'Continue'
$port = New-Object System.IO.Ports.SerialPort COM1, 115200
$port.Open()
function Say([string]$text) { foreach ($l in ($text -split "`r?`n")) { $port.WriteLine($l) } }
function Check([string]$what, [scriptblock]$test) {
    $ok = $false
    try { $ok = [bool](& $test) } catch { Say "        $_" }
    if ($ok) { Say "  ok    $what" } else { Say "  FAIL  $what"; $script:fail = 1 }
}
$fail = 0
$os = Get-CimInstance Win32_OperatingSystem
Say "== $($os.Caption) $($os.Version)"

# The wire to the gateway VM: the LAN, whose router (the gateway) doesn't
# know ACME's network either.
$nic = Get-NetAdapter | Where-Object Status -eq Up | Select-Object -First 1
Check "the LAN (198.51.100.20)" {
    New-NetIPAddress -InterfaceIndex $nic.ifIndex -IPAddress 198.51.100.20 -PrefixLength 24 `
        -DefaultGateway 198.51.100.10 -ErrorAction Stop | Out-Null
    Set-DnsClientServerAddress -InterfaceIndex $nic.ifIndex -ServerAddresses 198.51.100.10 -ErrorAction Stop
    $true
}

# Crake, installed: with its installer when the disc has one (as a person
# would), otherwise its files copied and the service made by hand.
$dir = "$env:ProgramFiles\Crake"
$setup = Test-Path "$Disc\crake-setup.exe"
if ($setup) {
    Check "the installer installs Crake" {
        $p = Start-Process "$Disc\crake-setup.exe" -ArgumentList '/S' -Wait -PassThru
        $p.ExitCode -eq 0 -and (Test-Path "$dir\openconnect\openconnect.exe") -and (Test-Path "$dir\crake.exe")
    }
    Check "listed in Installed apps" {
        (Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Crake').DisplayName -eq 'Crake'
    }
    Check "openconnect's and Wintun's licences beside them" {
        (Test-Path "$dir\openconnect\openconnect-LICENSE.txt") -and (Test-Path "$dir\openconnect\wintun-LICENSE.txt")
    }
} else {
    Check "installed in Program Files\Crake" {
        New-Item -ItemType Directory -Force $dir | Out-Null
        Copy-Item -Recurse -Force "$Disc\crake\*" $dir -ErrorAction Stop
        Test-Path "$dir\openconnect\openconnect.exe"
    }
    New-Service -Name craked -DisplayName 'Crake' -BinaryPathName "`"$dir\craked.exe`"" `
        -StartupType Automatic | Out-Null
}
Check "the craked service runs" {
    # craked's debug lines in its log, for when something goes wrong.
    Set-ItemProperty HKLM:\SYSTEM\CurrentControlSet\Services\craked -Name Environment `
        -Type MultiString -Value 'RUST_LOG=craked=debug,info'
    Restart-Service craked -ErrorAction Stop
    (Get-Service craked).Status -eq 'Running'
}
Check "it serves the app on 127.0.0.1:47366" {
    for ($i = 0; $i -lt 30; $i++) {
        if (Get-NetTCPConnection -LocalPort 47366 -State Listen -ErrorAction SilentlyContinue) { return $true }
        Start-Sleep 1
    }
    $false
}

# The gateway VM installs its software first: give it time.
Check "the gateway answers (198.51.100.10:443)" {
    for ($i = 0; $i -lt 120; $i++) {
        $c = New-Object Net.Sockets.TcpClient
        try { $c.Connect('198.51.100.10', 443); $c.Close(); return $true } catch { Start-Sleep 5 }
    }
    $false
}

# The test, as the app's side.
$env:CRAKE_E2E = '1'
$env:CRAKE_E2E_GATEWAY = 'https://198.51.100.10'
$env:CRAKE_OPENCONNECT = "$dir\openconnect\openconnect.exe"
Say "== the Windows tests (windows_tests.rs, windows_journey.rs)"
# Through cmd: PowerShell would wrap each line on stderr as an error.
$out = cmd /c "`"$Disc\crake-tests.exe`" windows_ --nocapture --test-threads=1 2>&1" | Out-String
Say $out
if ($LASTEXITCODE -ne 0) { $fail = 1 }

Check "craked stops cleanly" {
    Stop-Service craked -ErrorAction Stop
    -not (Get-Process openconnect -ErrorAction SilentlyContinue)
}
Check "no crake adapters left" { -not (Get-NetAdapter -Name 'crake-*' -ErrorAction SilentlyContinue) }
Check "no DNS rules left" {
    -not (Get-ChildItem 'HKLM:\SYSTEM\CurrentControlSet\Services\Dnscache\Parameters\DnsPolicyConfig' -ErrorAction SilentlyContinue |
        Where-Object PSChildName -like 'Crake *')
}
if ($setup) {
    Check "the uninstaller removes Crake" {
        Start-Process "$dir\uninstall.exe" -ArgumentList '/S' -Wait
        # It runs on from a copy of itself: wait for it to finish.
        for ($i = 0; $i -lt 60 -and (Test-Path $dir); $i++) { Start-Sleep 1 }
        -not (Test-Path $dir) -and -not (Get-Service craked -ErrorAction SilentlyContinue)
    }
    Check "and keeps the tunnels and log in ProgramData" { Test-Path "$env:ProgramData\Crake\log" }
}
Say "  craked.log:"
Get-Content "$env:ProgramData\Crake\craked.log" -ErrorAction SilentlyContinue |
    Where-Object { $_ -notmatch 'Windows Script Host|Microsoft Corporation' } |
    ForEach-Object { Say "        $_" }
Say "  what craked decided (its log, last lines):"
Get-Content "$env:ProgramData\Crake\log" -Tail 40 -ErrorAction SilentlyContinue | ForEach-Object { Say "        $_" }
Say "VM-TEST-RESULT: $fail"
$port.Close()
Stop-Computer -Force
