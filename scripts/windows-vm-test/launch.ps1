# In scripts/windows-vm-test.sh's base image, at every sign-in (elevated):
# run the test disc's run.ps1 when a disc labelled CRAKETEST is in. The
# base image itself has none, so it only signs in.
$vol = Get-Volume -FileSystemLabel CRAKETEST -ErrorAction SilentlyContinue | Select-Object -First 1
if ($vol) {
    & "$($vol.DriveLetter):\run.ps1" "$($vol.DriveLetter):"
}
