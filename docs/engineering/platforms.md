# Crake on Windows and macOS

<!-- Moved out of CLAUDE.md on 2026-10-20 (CLAUDE.md had grown past the
     150k characters an agent's context takes in). CLAUDE.md keeps the
     rules and a summary with a pointer here; this keeps the detail. Edit
     it as CLAUDE.md was edited: what was decided, built and found, dated. -->

## Other platforms: Windows first (decided with the user, 2026-10-11)

The user asked for macOS and Windows after Linux. Decided: **Windows
first** (testable here: a Windows VM under KVM next to a Linux gateway VM,
wired by a socket file, no network); **macOS after**, with no Mac (the
user has none): built and tested on GitHub's macOS runners only, real
gateway tests by someone with a Mac. **Signing:** the user has the paid
Apple Developer Program (ask for the details when a macOS build is signed
and notarized); no Windows code signing yet: development builds stay
unsigned, and a certificate (e.g. Azure Trusted Signing) is decided with
the user before a Windows release. **Applied to SignPath Foundation
(2026-10-16):** free signing for open-source projects, the certificate
SignPath Foundation's (it shows as the publisher), needing an OSI licence,
a fully automated build (release.yml) and a code signing policy page on
the website. The user emailed support@signpath.io (the apply form is
signpath.org/apply) and asked whether bundling Wintun's prebuilt DLL
(WireGuard's own licence, already signed by its authors) is acceptable,
offering to download it at install time instead. Waiting for their
answer. Fallbacks, researched the same day: Certum's open-source
certificate (cheap, in the user's name, a cloud key that's awkward from
CI); Azure Artifact Signing ($9.99/month) takes individuals from the US
and Canada only, organisations in the EU. Microsoft has no free tier for
open source. None of them avoids SmartScreen's warning at first: trust
builds with downloads. The WFP driver would still need its own EV
certificate. Phases: (1) craked's OS-specific parts behind
interfaces, Linux unchanged and all its tests green; (2) build both
binaries for Windows in a container; (3) Windows implementations:
openconnect + Wintun with crake-hook as its script, per-host routes (IP
Helper API), tunnel DNS via NRPT rules to craked's own DNS forwarder
(which routes an answer's addresses before replying), probing from the
TCP table (GetTcpTable2), craked as a Windows service, the app over a local
socket, the tray (notification area); (4) the VM test; (5) a signed
installer that bundles openconnect and Wintun.

**Phase 1: done.** craked's OS parts live in `crates/craked/src/sys/`:
`linux/` (conntrack, dns_queue, net, resolved, routing, and `host.rs`:
interfaces, fwmark and bound sockets, openconnect's process group and
stop, root-only files) and `windows/` (the same names, each returning
"… isn't built for Windows yet"); lib.rs re-exports them, so the rest
of craked says `crate::routing` etc. on every system. Portable types moved
out: `flows.rs` (connection events), `plan.rs` (marks and tables). The
transports are gated too: the hook's socket and the app's private socket
are `cfg(unix)`, with Windows stubs (named pipes in phase 3; `crake-hook`
already opens a pipe like a file there); `main.rs` stops on SIGTERM or
Ctrl+C (`Stop`). The integration tests are Linux-only.
`scripts/windows-check.sh` (CI job `windows`) keeps the workspace
compiling, clippy-clean, tests included, for x86_64-pc-windows-gnu;
locally in a MinGW container (`scripts/windows/Dockerfile`).

**Phase 2: done.** The app has the same layout, `crates/crake/src/sys/`:
`linux/` (the StatusNotifierItem tray, the GNOME keyring, X11's
monitors, one Crake per session, private files, the dev socket) and
`windows/`, real rather than stubs (a stub left the portable code that
feeds it dead, and lints are errors): the notification area's icon and
menu (`tray-icon`; the head drawn dark on a light taskbar), Credential
Manager (2560 bytes a credential, so a certificate is kept in parts
`Crake/<tunnel>/<kind>/<n>`), monitors in logical pixels, a named pipe
for one Crake per session, private files in LOCALAPPDATA. Only the
connection to craked waits for phase 3. The tray's art stays portable
(`tray::State`, `tray::Icon`). Tests see no monitors on any system (the
Windows runner's small screen hid buttons the window tests click). The
.exe files are built on a Windows runner (CI job `windows-build`, which
also runs the portable tests there and uploads `crake-windows`): GPUI's
release shaders need the Windows SDK's fxc (vkd3d can't compile them:
no `SV_ClipDistance`; GPUI's debug path reads the shader source from the
build machine at run time). Not yet tried on a real Windows.

**Phase 3: in progress** (tried in the Windows 11 VM against the test
gateway VM, see below). Built: craked as the service `craked` (logs to
ProgramData\Crake\craked.log; `--console` to run by hand); the hook over
a named pipe through `packaging/windows/crake-hook.js` (openconnect runs
scripts with cscript.exe, 10 s at most); the app on 127.0.0.1:47366
(craked lets the console session change things; the app checks the port
is the service's); the tunnel's address, MTU and DNS route, learned and
pinned hosts as routes (IP Helper); openconnect stopped with Ctrl+Break /
Ctrl+C sent by a copy of craked (`--console-event`: whoever sends it gets
it too, and it ended the service); sockets bound to a tunnel
(IP_UNICAST_IF); openconnect from Crake's folder; openconnect for Windows
built by `scripts/build-openconnect-windows.sh` (unbuffered output, else
its questions never reach sign-in); the app: no console windows, plain
DWM frames (no square outline), the panel at the tray. Sign-in, switch
on and off, and DNS through the tunnel work in the VM.
Learning, built and tried in the VM: **connection probing** (the TCP
table polled four times a second, `flows::changes` makes conntrack's
events of it; the probe's socket bound to the tunnel, which finds the
tunnel's own default route, metric 9000, never taken by normal traffic;
a stuck connection reset with SetTcpEntry) and **craked's own DNS server**
on 127.53.0.1:53, where NRPT rules (volatile registry keys) send each
tunnel's domains: it asks the tunnel's DNS through the tunnel, routes the
answer, then replies (routed before the first packet), and answers
craked's own links (crake-hush refuses: names kept private while a
tunnel is off; crake-keep answers from memory). Over TCP too (a question that
came over TCP is asked of the tunnel's DNS over TCP). While a tunnel is
up, an NRPT rule for the root (`Crake public`) sends every other name to
craked's server too: it asks Windows' own DNS servers (the adapters' that
are up, Crake's left out) and hands the answer to the manager's
`rescue_answer` (the public-NXDOMAIN rescue and split-horizon, shared
with Linux, which wraps it for NFQUEUE's packets). A full tunnel: 0/1
and 128/1 through the tunnel (more specific than the LAN's default route
whatever its metric), each exception that went by the default route (the
gateway) routed the way it went before (GetBestRoute2), and every name
to its DNS (an NRPT rule for the root). Nothing on Windows says "isn't
built for Windows yet" any more. Then the installer (phase 5). A fresh
probe's first connection fails on Windows (its retry works): Linux's
retransmitted SYN takes the new route and is NATed to the tunnel's
address; Windows' stuck connection keeps the LAN's source address, so
craked resets it. Making the first one work would need a WFP callout
driver (holding the SYN), not planned.
IPv6 (2026-10-12): learned and pinned IPv6 hosts are routes too (IP
Helper, SOCKADDR_IN6), the tunnel gets the IPv6 address openconnect
hands over and its own IPv6 default route for probes (metric 9000),
sockets bind with IPV6_UNICAST_IF (host byte order, unlike IPv4's),
probing reads the IPv6 TCP table, a full tunnel adds ::/1 and 8000::/1
when the tunnel has an IPv6 address. Without one, IPv6 is refused by a
Windows Firewall rule (`firewall.rs`, group "Crake", via PowerShell):
outbound on the machine's own adapters, to everything but loopback,
link-local, multicast, the LAN and the gateway (`plan::uncovered6`
ranges); a connection fails at once. Routing the halves to the tunnel
didn't work: Windows skips a route whose interface has no address to
send from. A marker file (ProgramData\Crake\firewall-rules) is there
while a rule may exist; only then does a start clean up (PowerShell's
firewall module takes ~10 s to load the first time, too long for a
service starting). IPv6 DNS servers are asked by craked's DNS server
(through the tunnel, a /128 to each), in the gateway's order. Windows
can't reset another program's IPv6 connection (SetTcpEntry is IPv4's;
raw sockets can't send TCP): a stuck one runs out and the app's next
goes through; only a WFP callout driver could do better. The VM test
proves it: its gateway hands out fd40:ff::/64 addresses, ACME's
fd40:1::10 and a DNS server at fd40::2 (listed first, knowing a name
the IPv4 one doesn't), the LAN has IPv6 with an "internet" host
2001:db8:77::1, and a second ocserv on 8443 gives no IPv6: a name known
only by its IPv6 address is routed before the first packet, the IPv6
DNS server is asked, a full tunnel with IPv6 takes IPv6, and one without
refuses it at once (and IPv6 goes direct again when it's off).

**Phase 4: done** (`scripts/windows-vm-test.sh`; passes: about 2½
minutes a run, the image's unattended install about 25 minutes once). A Windows 11 VM with
Crake installed as a person would have it (Program Files\Crake, the
craked service, openconnect beside it) and a gateway VM (Ubuntu's cloud
image, ocserv, ACME's network behind it, and the LAN's own DNS, which
knows no name), wired by a socket file only (qemu's stream netdev); the
gateway's second card reaches Ubuntu's archive for ocserv, nothing is
forwarded. The Windows image is installed once, unattended, from
Microsoft's evaluation ISO (`scripts/windows-vm-test/autounattend.xml`:
no network, no TPM or Secure Boot asked for, a sign-in task that runs the
test disc's `run.ps1`), cached in target/vm (delete it after the 90-day
evaluation). `run.ps1` installs Crake, waits for the gateway, runs the
Windows test (`crates/crake/src/windows_tests.rs`, the app's client and
sign-in against the installed service: probing, a name only ACME's DNS
knows, over TCP too, a public name rescued, the charts' counters, names
refused at once when off, a full tunnel), the Windows journey
(`windows_journey.rs`), and checks craked stops
leaving no adapters or NRPT rules; it reports on the serial port. The
test binary and craked are cross-built with MinGW in
`crake-windows-build`. Not in GitHub's CI (KVM).

**Windows on ARM64 (planned with the user 2026-10-20, for Windows 11 for
ARM in a VM on an Apple Silicon Mac).** Windows on ARM runs x64 programs
emulated but never x64 drivers, and Crake needs drivers (Wintun,
ovpn-dco, tap-windows6), so it gets a build of its own:
- crake, craked and the hook built natively on GitHub's `windows-11-arm`
  runner (`aarch64-pc-windows-msvc`, the CRT linked in as on x64), their
  portable tests run there (CI's `windows-arm64-build`).
- openconnect built natively there too, in MSYS2's CLANGARM64
  environment (`scripts/build-openconnect-windows-arm64.sh`: Fedora's
  MinGW, which builds the x64 one, has no ARM64), the same version and
  one change, Wintun's ARM64 DLL beside it, MSYS2's DLLs it needs
  collected with llvm-objdump; cached while its script doesn't change.
- OpenVPN from the same release's ARM64 MSI, tap-windows6's ARM64 build
  (`scripts/fetch-openvpn-windows.sh arm64`, pinned by checksum).
- One installer per architecture (`CRAKE_WINDOWS_ARCH=arm64
  scripts/build-windows-installer.sh` → `crake-setup-<version>-arm64.exe`);
  each refuses the other's Windows and names the right file
  (`${IsNativeARM64}`). The NSIS stub itself is x86 (emulated on ARM64,
  `Sysnative` reaches the native pnputil).
- `scripts/windows-arm64-install-test.ps1` (CI's `windows-arm64-install`,
  and the release's `windows-arm64` before publishing): installed
  silently, every program ARM64, the service running and serving the
  app, openconnect and openvpn starting, OpenVPN's drivers in the driver
  store, removed leaving nothing.
- The gateway tests on ARM64: by hand, through the Doctor (doctor.yml,
  system `windows-arm64`: main's ARM64 installer and test binary, the
  `crake-windows-arm64-tests` artifact, on windows-11-arm). Not in every
  CI run: uncached, the ARM64 builds take ~25 minutes, and the shared
  Cloudflare gateways would be held that much longer. First run
  2026-10-21 (Doctor run 39508002820, AnyConnect): crake-acme up on
  Wintun's ARM64 driver, a host learned (its route), ACME's domain sent
  to craked's DNS by an NRPT rule, and everything gone after. Not yet:
  anyone using it on a real Windows on ARM (the checklist).

**macOS (started 2026-10-11, after Windows' phase 4, as the user
suggested).** Same layout as Windows. craked's `sys/macos/`: the
tunnel's address and routes with `ifconfig` and `route` (`cmd.rs`;
learned and pinned hosts, its DNS, a default route scoped to the device
for probes, a full tunnel's halves and kept gateway), /etc/resolver
files (one per domain, marked as Crake's and whose; others' files left
alone) sending the tunnels' domains to craked's own DNS server, which is
now shared with Windows (`sys/dns_server.rs`, on 127.53.0.1:53 there, an
address craked adds to lo0;
whose a name is: `sys/dns_links.rs`), sockets bound with IP_BOUND_IF,
byte counters from getifaddrs. openconnect can't name utun devices, so
on macOS craked doesn't pass `--interface` and takes the utunN it
reports (`sys::NAMES_DEVICES`, `sys::tunnel_device`; `device_of`).
Probing reads the kernel's TCP list (`net.inet.tcp.pcblist_n`, what
netstat reads) four times a second into `flows::changes`, as Windows
polls. macOS can't reset another process's connection, so what leaves a
tunnel with another source address (a stuck connection's next SYN) is
NATed to the tunnel's address by pf, in Crake's anchor
`com.apple/250.Crake` (pf on with a token while tunnels are up), as Linux
masquerades: the first connection can work, as on Linux. craked serves
the app on /var/run/crake.sock (peer to peer; changes need the user at
the console or root, `Guard::ConsoleUser`); paths: hook socket in
/var/run/crake, hook in /usr/local/libexec/crake (the system's /usr is
sealed), state in /var/db/crake; openconnect from Homebrew
(/opt/homebrew/bin, /usr/local/bin: launchd's and Finder apps' PATH
lack it); `packaging/macos/software.dorhawk.crake.daemon.plist` runs craked as
a launchd daemon. IPv6 (2026-10-12, as on Windows): `route`/`ifconfig`
with `-inet6` (the tunnel's address as an alias), a scoped IPv6 default
route, IPV6_BOUND_IF, the kernel list's IPv6 rows (the address unions
start 4 and 20 bytes after inp_vflag), pf NAT for inet6, a full
tunnel's IPv6 halves (without an IPv6 address: reject routes to ::1,
refused at once), IPv6 DNS servers asked by craked's DNS server;
tests/macos.rs checks the routes and that a reject route refuses at once,
as root on CI.
Public names (2026-10-13): /etc/resolver can't name a resolver for
every name, so while a tunnel is up craked's server stands in for the
primary network service's DNS servers in the dynamic store
(`sys/macos/primary_dns.rs`, system-configuration: `State:` DHCP's,
`Setup:`'s when the user typed servers), as vpnc-script does, and asks
the servers it displaced (`system_dns`): the public-name rescue,
split-horizon, and a full tunnel's DNS for every name, as on Windows. A
watch on those keys takes their place again after a DHCP renewal or a
change of network; what was displaced is kept in
/var/db/crake/system-dns.json until it's given back, so a craked that
died puts it back when it starts. tests/macos.rs checks macOS really asks
craked's server for a public name and gets its own servers back.
Several servers (2026-10-26, the user's Mac: the network's first DNS
server never answered, its second did, and every new name took
`WAIT`'s three seconds, so pages loaded slowly): the one that answered
last is asked first, and the next is asked too after 400 ms of quiet
or at once when one fails (`dns_ask`, also for a tunnel's servers).
Windows on macOS (2026-10-13, a colleague's first Mac run): opening one
makes Crake the active app first (`windows::to_front`; a menu bar app's
non-activating panel left them behind the app in front); the system draws
the window buttons, top left, and Crake's headers leave them room and drop
their own × (`chrome.rs`; design/proposals/macos-window-buttons.html,
option A, chosen by the user, as on Windows); craked's launchd plist and
the login agent name Crake.app (`AssociatedBundleIdentifiers`), or System
Settings' background items list them under the certificate's owner. Each
has a test that failed first (asked for: desired state, fail, fix, pass).
The panel under the menu bar (2026-10-13): tray-icon reports a click
and the icon's rect in pixels (points times the icon's display's scale,
from the main display's top-left); `sys::tray_click` turns the icon's
bottom centre into points (each display's scale from CoreGraphics'
display mode), and the panel hangs a gap below it (`place::Bar::EDGE`),
kept inside that display (`monitors`: CGDisplayBounds, global points,
which is also how GPUI places a window relative to the main display). A
pop-up window there is a non-activating NSPanel above other windows on
every Space. Not yet seen on a real Mac.
The installer (2026-10-13): `scripts/build-macos-pkg.sh` makes
target/macos/Crake-<version>.pkg on a Mac: Crake.app in /Applications
(packaging/macos/Info.plist: a menu bar app, LSUIElement; the icon
`assets/icons/app/crake.icns`, from `scripts/make-icons.sh` at 824 of
1024 points as macOS' icons are), the hook and the uninstaller
(`sudo /usr/local/libexec/crake/uninstall`) in /usr/local/libexec/crake.
**craked is Crake.app's own service (asked for by the user 2026-10-14,
from 0.7):** `Crake.app/Contents/MacOS/craked`, its launchd file in
`Contents/Library/LaunchDaemons` (`BundleProgram`), registered by the app
with SMAppService (`sys/macos/service.rs`, each start), so System
Settings lists it under Crake: a daemon a package puts in
/Library/LaunchDaemons is listed under the signing certificate's name
("ONDŘEJ ŠIMON"; macOS curates names only for known vendors, Cisco's
included, as a colleague's `sfltool dumpbtm` showed). macOS has the user
switch it on once (Login Items & Extensions); until then the panel says
"Crake's service needs your OK." with Open Login Items
(design/proposals/macos-service-approval.html, option A; "switched off"
once it had run, `Prefs::service_was_on`). "Start at login" is the app's
login agent in `Contents/Library/LaunchAgents` (with `--autostart`),
registered the same way. `postinstall`: an older craked (its own daemon,
0.6 and before) hands its tunnels over and goes with its files; a
registered one hands over and is restarted (`kickstart -k`); then Crake
opens for the user at the screen, which registers it. Homebrew's cask
upgrade takes craked out of launchd first, and macOS still says
"enabled": the app registers it again when it stays out of reach
(`service::revive`; Not now has the detail). **Labels of their
own:** `software.dorhawk.crake.daemon` and `software.dorhawk.crake.login`,
not 0.6's `software.dorhawk.craked` / `software.dorhawk.crake`: macOS keeps
its record of the old daemon after its file is gone, and SMAppService,
asked about the same label, answered "enabled" for it and registered
nothing (CI's Mac: craked never ran); the old record's "disallowed" would
have carried over too. Universal (Apple Silicon and Intel:
`CRAKE_UNIVERSAL=1` joins both builds with lipo; releases and CI runs
started by hand build it, pushes only Apple Silicon); openconnect from
Homebrew (the app says `brew install openconnect` when it's missing), not
bundled yet. **Signing (set up with the user, 2026-10-13):** the user's
personal Apple Developer account (team 5CTH49SXZ6): a Developer ID
Application certificate (Crake.app, craked, the hook: hardened runtime,
secure timestamp) and a Developer ID Installer certificate (the .pkg),
both made from signing requests created here (no Mac needed), and an App
Store Connect API key with Developer access for notarization (its own,
not the account's Admin key used for another app). The two .p12
bundles (each certificate with its key) with their passwords and the .p8
are kept in the user's password manager only (not on disk); the
certificates are the user's, not Crake's: other dorhawk.software Mac apps
may be signed with them too; CI has them as the secrets
MACOS_APP_P12(_PASSWORD), MACOS_INSTALLER_P12(_PASSWORD), APPLE_API_KEY,
APPLE_API_KEY_ID, APPLE_API_ISSUER_ID, only in the steps that use them
(never in a build: crates' build scripts run there).
`scripts/macos-signing.sh setup` makes a keychain of its own on the
runner; `build-macos-pkg.sh` signs when CRAKE_KEYCHAIN names it (ad hoc
otherwise); `macos-signing.sh notarize` submits the package, staples
the ticket and checks Gatekeeper accepts it: on tags and runs started
by hand only (it takes minutes). Every main push signs. Never revoke
the certificates unless a key leaked: revoking makes macOS refuse every
Crake already signed with them.
`scripts/macos-pkg-test.sh` (CI job macos) installs it on the runner,
checks craked serves /var/run/crake.sock under launchd, installs again
over it (craked restarted) and uninstalls (nothing left); CI uploads the
package as `crake-macos`. CI packs the development build its tests just
built (`CRAKE_PROFILE=dev`): a release build took 9 minutes and a bigger
cache; releases (signed, later) build release. The app's `sys/macos/`: the Keychain (security-framework,
labelled items), the menu bar through `sys/menu_tray.rs` (the tray-icon
code Windows used, now shared), the private folder in the user's
$TMPDIR, plain notifications (notify-rust has no actions there),
craked's socket. What Linux and macOS do alike lives in
`sys/unix.rs` in both crates. craked type-checks for macOS in a plain
Linux container (`scripts/macos-check.sh`); the app needs a Mac's
toolchain, so CI job `macos` (macos-latest) runs clippy, the
portable tests, and `crates/craked/tests/macos.rs` as root: a utun
device the test makes itself, its address, DNS route, learned and pinned
routes, a stuck connection's SYN read off the device NATed to the
tunnel's address, and an /etc/resolver file that macOS really follows
to a stand-in server; without root, a connection nobody answers seen
stuck in the kernel's list. A real gateway test needs someone with a Mac (Homebrew
has openconnect, not ocserv).

**OpenVPN on Windows (open decision, noted for the user):** OpenVPN 3
Linux (the D-Bus service Crake drives there) doesn't exist on Windows, so
Crake on Windows speaks only openconnect's protocols today. Options:
OpenVPN Connect when installed (Crake can't drive it), OpenVPN's community
openvpn.exe (2.6, bundled like openconnect), or OpenVPN 3's core library.
If Crake starts OpenVPN itself, the Linux `tun0` problem (openvpn3 names
its own devices, so craked maps them back with `tunnel_of_device`) goes
away: on Windows OpenVPN lets the starter name its Wintun adapter
(`--dev-node crake-<name>`).
**When OpenVPN comes to Windows or macOS:** today `attach` refuses OpenVPN
3 sessions there (`sys::ADOPTS_OPENVPN3`, false on both; a test says so).
If craked starts OpenVPN itself, it watches the process like openconnect's
and needs none of this. If it adopts sessions it doesn't run (OpenVPN
Connect, FortiClient as in 7f), that system needs its own
`net::is_tun` (a stub returning false today) so `check_devices` notices
a session that ended behind the app's back (see 7g), and then
`ADOPTS_OPENVPN3` true.

**A WFP callout driver for Windows (someday; decided with the user,
2026-10-12: "not now, but write it down").** Two things Windows can't do
from user mode, and Linux does: the first connection to a host only a
tunnel reaches fails there (its retry works), and a stuck IPv6
connection can't be reset (SetTcpEntry is IPv4's; raw sockets can't send
TCP), so it runs out (~21 s) before the app's next one takes the route.
A kernel driver of Crake's own would fix both:
- At the ALE connect-redirect layer (FWPM_LAYER_ALE_CONNECT_REDIRECT_V4/
  V6) pend a new connection to an address worth racing (the same
  syn_watch/syn_skip ranges as Linux's first-SYN race), hand it to
  craked (an IOCTL or a shared queue, like NFQUEUE), and when a tunnel
  takes the host, rewrite the connection's local address to the
  tunnel's (FWPS_CONNECT_REQUEST0) before its first packet: the route
  lookup then goes through the tunnel. Direct or down: let it go as is.
- An IOCTL to abort a flow craked names (FwpsFlowAbort0 with a flow
  context, or a RST injected with FwpsInjectTransportReceiveAsync0),
  IPv4 and IPv6, replacing SetTcpEntry.
In C with the WDK (or windows-drivers-rs), built on GitHub's Windows
runner; tested in the Windows VM with test signing
(`bcdedit /set testsigning on`, a self-signed driver; the VM has no
Secure Boot). **Shipping it** needs Microsoft's signature: an EV
code-signing certificate for dorhawk.software (~$300–400/yr, on a
hardware token or cloud HSM; a sole-proprietor EV tier exists), a free
Microsoft Partner Center account, and attestation signing for each
driver build (check Microsoft Learn's current rules before buying).
Considered and not chosen: WinDivert (an existing signed packet driver,
LGPL-3.0/GPL-2.0): no signing cost, but antivirus engines often flag
WinDivert64.sys as a hacker tool, a problem on the company laptops
Crake is for, and Crake would have to NAT those connections in user
mode.

**Also noted (user):** the panel has no shadow on
Windows (Linux draws one in its transparent margin), and a few white
pixels show outside the rounded top corners; both look like GPUI's
DirectX drawing of transparent windows (the renderer anti-aliases the
card's `rounded(px(10.))` corners against the transparent backdrop and
leaves a white fringe). **Done (2026-10-12, `76d8d29`):** on Windows the
panel window is opaque and the size of its card, Windows rounds it
(`sys::rounded_frame`, DWMWCP_ROUND) and draws its hairline in the
theme's line colour; the card doesn't round itself or draw a border
there. Checked in the user's Windows 11 VM (CI's build, demo panel,
`virsh screenshot`): no fringe, a clean hairline; but that VM has the
Microsoft Basic Display Adapter (no GPU), where Windows draws every
window square and without effects, so the rounding and whether Windows
adds a shadow to the borderless pop-up still need a Windows with a GPU.
If it has no shadow there: DwmExtendFrameIntoClientArea with 1 px
margins is the usual fix. **Decided with the user (2026-10-12): Windows
draws the panel's frame, the native way.** Where Windows rounds windows
(Windows 11 with a GPU) the panel is rounded; where it doesn't (Windows
10, a VM without a GPU, perhaps Remote Desktop) it's square like every
other window there. Don't bring back Crake's own rounding on Windows
(drawn, or clipped with SetWindowRgn): "If most of Windows is
rectangular, I will not introduce a rounded concept."
The installer now uses NSIS Modern UI 2 (`packaging/windows/crake.nsi`,
done 2026-10-12): welcome page, Crake's icon, a finish page that can open
Crake; it was the classic 90s wizard before.

## Someday: other platforms

Moved here from CLAUDE.md on 2026-10-21 (the user's rule on keeping CLAUDE.md small: detail lives in the documentation it belongs to; CLAUDE.md keeps the rule and a pointer). Nothing was dropped in the move; add to this file from now on.

Wanted one day, not planned yet. Keep the split that makes it possible:
decisions (host store, expiry, policy, log, probing logic) stay portable;
everything that touches the OS stays behind small modules (`net`, `routing`,
DNS, tray) that can get a per-OS implementation.

- **Other Linux distributions:** mostly as is. Watch strict reverse-path
  filtering (`rp_filter=1`, e.g. Fedora/RHEL): replies from routed hosts
  arrive on the tunnel while the unmarked route points elsewhere, so they'd
  be dropped. Ubuntu uses loose mode (2). **Fixed:** craked sets loose
  mode on each tunnel's own interface when it comes up, over netlink
  (RTM_SETLINK; works where /proc/sys is read-only); the kernel takes the
  looser of `all` and the interface. The netns tests run with rp_filter=1
  everywhere and fail without it. Debian 12/13: tested, same .deb. Needs openconnect, nftables,
  iproute2, systemd, D-Bus and polkit. DNS learning assumes systemd-resolved;
  distros without it need a fallback. GNOME needs the AppIndicator extension
  for the tray; KDE has it built in. Packaging: .deb first, then .rpm / AUR.
- **Windows:** openconnect has Windows builds (Wintun). Routing would be
  per-host routes on the tunnel interface (IP Helper API) instead of fwmark;
  per-domain DNS via the Name Resolution Policy Table; the daemon becomes a
  Windows service, the tray uses the notification area. Testable in a VM.
- **macOS:** openconnect runs there (utun). Per-host routes via the routing
  socket, per-domain DNS via `/etc/resolver/<domain>`, a privileged launchd
  helper, a menu bar item. Needs Apple code signing and notarization, and real
  testing needs a Mac (macOS VMs are only allowed on Apple hardware).
- Per-host routes are the common denominator. Crake already keeps expiry in
  its own store, not only in nftables timeouts, so that part ports.
