#!/bin/sh
# What the Doctor workflow (.github/workflows/doctor.yml) looks at, at each
# phase of a tunnel's life: the system's DNS and routes, its firewall
# rules, a public name looked up and fetched, and craked's logs, written to
# OUT/<label>.txt.
#   scripts/doctor-snapshot.sh macos|windows LABEL OUT
set -u
SYSTEM=$1 LABEL=$2 OUT=$3
mkdir -p "$OUT"
F="$OUT/$LABEL.txt"
say() { printf '\n===== %s\n' "$*" >> "$F"; }
run() { say "$*"; sh -c "$*" >> "$F" 2>&1; }
ps() { say "$*"; powershell -NoProfile -Command "$* | Out-String -Width 250" >> "$F" 2>&1; }
date -u '+%Y-%m-%dT%H:%M:%SZ' > "$F"
if [ "$SYSTEM" = macos ]; then
  run "scutil --dns"
  run "netstat -rn"
  run "ifconfig"
  run "ls -la /etc/resolver; head -50 /etc/resolver/* 2>/dev/null"
  run "sudo pfctl -s nat -a com.apple/250.Crake 2>&1; sudo pfctl -s info 2>&1 | head -5"
  run "dscacheutil -q host -a name one.one.one.one"
  run "curl -sS -m 10 -o /dev/null -w '%{http_code} in %{time_total}s\n' https://one.one.one.one/"
  run "dscacheutil -q host -a name wiki.acme.internal"
  run "route -n get 10.40.1.10"
else
  ps "ipconfig /all"
  ps "route print"
  ps "Get-DnsClientNrptRule | Format-Table Namespace,NameServers,Comment -AutoSize"
  ps "Get-NetIPInterface | Sort-Object InterfaceMetric | Format-Table ifIndex,InterfaceAlias,AddressFamily,InterfaceMetric,ConnectionState -AutoSize"
  ps "Get-NetFirewallRule -Group Crake -ErrorAction SilentlyContinue | Format-Table DisplayName,Direction,Action,Enabled -AutoSize"
  ps "Resolve-DnsName one.one.one.one -ErrorAction Continue"
  ps "try { (Invoke-WebRequest https://one.one.one.one/ -UseBasicParsing -TimeoutSec 10).StatusCode } catch { \$_.Exception.Message }"
  ps "Resolve-DnsName wiki.acme.internal -ErrorAction Continue"
  ps "Find-NetRoute -RemoteIPAddress 10.40.1.10 | Select-Object -Last 1 | Format-List InterfaceAlias,DestinationPrefix,NextHop"
  ps "Get-Content C:\\ProgramData\\Crake\\log -Tail 40"
  ps "Get-Content C:\\ProgramData\\Crake\\craked.log -Tail 120"
fi
echo "$LABEL: $F"
