# Crake

A smart VPN client for Linux (Ubuntu first), written in Rust. It connects to
AnyConnect-compatible gateways (and the other protocols openconnect speaks),
**routes nothing by default**, and learns, per host, what needs a tunnel: when a
host doesn't answer directly but answers through a tunnel, Crake routes just that
host through it and tells the user. Several tunnels can be up side by side.

Everything Crake decides happens on the user's machine. Nothing is sent anywhere.

Part of dorhawk.software (siblings: tern, Obálka, KNOT). Each app has its own
visual identity; Crake's is documented in `design/DESIGN.md`.

## Git rules

- Commit as `ondrej@dorhawk.software` (set as `user.email` in this repo;
  never use another address).
- Never include a Claude session ID or session URL in commit messages or PR
  descriptions. Never, under any circumstances.
- The repository is public: github.com/dorhawk-software/crake. The user
  allows pushing once work is done and tested (2026-10-11); push with
  `git push` (this repository's credential helper is gh's, the user's
  personal account).
- Nothing customer-specific is ever committed: no real gateways, company
  names, hosts or addresses of the user's clients, in code, tests, demo
  data, designs or these notes. Use ACME, Nordbank, Globex, Initech,
  `*.example` / `*.internal` names and documentation addresses
  (192.0.2.x, 198.51.100.x, 203.0.113.x). What was learned on the real
  gateways lives in `CLAUDE.local.md` (git-ignored). `scripts/check-private.sh`
  (run by `scripts/lint.sh` and the pre-commit hook) refuses the names
  listed in `dev/private-terms` (git-ignored); install the hooks with
  `ln -s ../../scripts/pre-commit.sh .git/hooks/pre-commit` and
  `ln -s ../../scripts/check-private.sh .git/hooks/commit-msg`.

## Read first

- `design/DESIGN.md`: identity, tokens, type, icons, chart rules, copy voice,
  and the directions that were rejected. Follow it; don't redesign.
- `design/screens/*.html`: the approved screens. Open in a browser
  (`?dark=true` for dark). They are the spec for the UI.
- `design/icons/`: tray icon states as symbolic SVGs.
- `docs/engineering/`: the detail CLAUDE.md summarises (milestones,
  platforms, packaging, testing). Read the part for the area you change.

## Keeping this file small (the user's rule, 2026-10-20)

CLAUDE.md stays under 150k characters (what an agent's context takes in
whole; `wc -c CLAUDE.md`). When it nears that, decompose it: move a long
section's detail into the documentation it belongs to (a file of its own
under `docs/engineering/`, or next to the code), leave here the rules and
a short summary with a pointer, and say so in the moved file. Nothing is
dropped in the move. New detail goes to those files from the start;
CLAUDE.md gets the rule or the one line that points there.

## Architecture (plan)

Two processes, split by privilege:

1. **`craked`**: system daemon (systemd service, `CAP_NET_ADMIN`). Runs
   openconnect, owns routing, DNS, probing, policy and the log. Exposes a D-Bus
   API on the system bus, e.g. `software.dorhawk.Crake1`, guarded by polkit.
2. **`crake`**: user-session UI. Tray icon (StatusNotifierItem), the drop-down
   panel, the windows (log, settings, tunnel detail, new tunnel, sign-in),
   notifications, keyring access. Talks to `craked` over D-Bus.

### Tunnels: wrapping openconnect

Start with the stock `openconnect` binary, one process per tunnel. Don't
reimplement the protocols.

- `openconnect --protocol=<anyconnect|gp|pulse|nc|fortinet|f5|array>
  --interface=crake-<name> --script=/usr/libexec/crake/hook <gateway>`
- The hook replaces vpnc-script. openconnect calls it with `reason`
  (pre-init, connect, disconnect, reconnect, attempt-reconnect) and env vars
  such as `TUNDEV`, `VPNGATEWAY`, `INTERNAL_IP4_ADDRESS`,
  `INTERNAL_IP4_NETMASK`, `INTERNAL_IP4_DNS`, `CISCO_DEF_DOMAIN`,
  `CISCO_SPLIT_DNS`, and the `CISCO_SPLIT_INC_*` / `CISCO_SPLIT_EXC_*` lists.
  The hook forwards them to `craked` over a Unix socket and exits.
- `craked` assigns the address and installs **no routes** except a /32 to the
  tunnel's DNS servers. Pushed split-include networks and DNS domains are
  **hints**, shown in the UI and used as seeds, routed up front only if the user
  or policy says so.
- Sign-in: browser SSO, password plus one-time code, or certificate
  (PKCS#11 / keyring / file). Check what the installed openconnect supports for
  SSO (`--external-browser` in recent versions) before building helpers;
  `--authenticate` plus `--cookie-on-stdin` lets the UI handle auth separately
  from the connection.
- Later, optionally: link libopenconnect via FFI for tighter control (its
  auth-form callbacks would let Crake's own sign-in window render the
  gateway's prompts). Reimplementing the protocols natively is not planned:
  Crake's value is the routing, and openconnect covers seven protocols on
  Linux, Windows and macOS.

### Routing

- nftables table `inet crake`. Per tunnel: an address set with per-element
  timeouts (`flags timeout`) for learned hosts, a static set for pinned ones,
  and a rule that sets a per-tunnel `meta mark` for destinations in either set.
- Per tunnel: `ip rule fwmark <mark> lookup <table>`, and that table's default
  route goes via the tunnel interface.
- Learning adds an element; forgetting removes it; expiry is the set timeout
  (default 7 days without traffic, refreshed on use).
- Policy decides which destinations may be learned for a tunnel (e.g. only
  `*.acme.internal` and `10.40.0.0/16`); outside that scope Crake leaves the
  host direct even if it doesn't answer.

### Detecting "needs a tunnel"

Two signals, in order of value:

1. **DNS.** Tunnel DNS servers are registered with systemd-resolved per link,
   with the pushed domains as routing domains (D-Bus, `org.freedesktop.resolve1`).
   Unknown names that fail publicly are asked of each tunnel's DNS in try-order.
   A name only a tunnel knows is routed **before the first packet**. With
   "keep internal names private" on, internal suffixes never go to public DNS.
2. **Connection probing.** Watch new outbound TCP connections (conntrack events
   or NFQUEUE). If a SYN gets no answer within the configured wait (1/2/3/5 s,
   default 3), or gets refused or ICMP-unreachable, probe the same IP and port
   through each eligible tunnel (socket bound with `SO_BINDTODEVICE`). If one
   answers, add the host to that tunnel's set and log it. If none answer, leave
   it alone and log "No answer just then, directly or through ACME.
   Nothing was changed." (until 2026-10-16 "Probably down.", which read as
   a verdict on a host that answered fine a moment later).

When two tunnels answer: ask the user (default), use the faster, or use the
first in order. Never route hosts on the user's "never send through a tunnel"
list.

**First-SYN race (built, Linux).** Even the first connection to a host that
needs a tunnel succeeds: craked holds the first SYN in NFQUEUE (an `syn_race`
`type route hook output` chain, queue 7301, `bypass`), races a direct attempt
against the tunnels (`race_held`), learns the host if a tunnel takes it, then
lets the SYN through — the output chain marks it and re-routes it through the
tunnel (the re-injected SYN really does re-route; netns-proven, and a mutation
makes the first connection take the probe wait again). Direct is preferred when
it answers within a round trip (`DIRECT_WAIT` 400 ms); past that only a tunnel
might reach it. Only SYNs worth racing are held: a tunnel's `Only` scope, its
pushed split-include networks, or the private ranges for a learn-anything
tunnel (fc00::/7 for IPv6), minus the local networks and the gateways
(`sync_syn_race` keeps the `syn_watch`/`syn_skip` sets, and `syn_watch6`/
`syn_skip6`, in step as tunnels come and go, and once more when a tunnel's
pushed networks are known; the sets `auto-merge`, since nft refuses
overlapping intervals and a pushed 10.40.0.0/16 inside 10.0.0.0/8 had
silently kept the race's update out). IPv6 SYNs are raced too. Off with
`race_first_syn = false` (default on); passive conntrack probing still handles
anything not held. Windows' equivalent is a WFP callout driver (not built; the
first probed connection there is still reset). The tunnel race and learn/log
are shared with passive probing (`try_tunnels`).

### Managed (IT) profiles

- Read-only TOML in `/etc/crake/managed.d/*.toml`: gateway, protocol, group,
  sign-in method, and a learning policy (`learn = ["*.acme.internal",
  "10.40.0.0/16"]`, or `full_tunnel = true`).
  An OpenVPN tunnel names its `.ovpn` profile instead of a gateway (7i).
- The user can switch a managed tunnel on and off, but can't edit it or widen
  its scope. The UI shows the policy in plain words.

### Log and privacy

- Append-only JSON lines in `~/.local/share/crake/log` (per user), retention
  configurable (default 30 days). Each entry is a decision: routed, left alone,
  undone, or a system event, with the reason and timing.
- "Copy for helpdesk" produces a readable plain-text summary; optionally with
  host names replaced by tunnel plus timing.
- No telemetry. Ever.

## Suggested crates (verify before committing)

tokio, serde + toml, tracing, zbus (D-Bus: own API, resolved, notifications),
rtnetlink (addresses, rules, routes), nftables (JSON via `nft`) or a netlink
nftables crate, a conntrack/netlink crate or `nfq` for NFQUEUE, socket2
(`bind_device`), hickory-resolver / hickory-server (DNS), ksni (tray),
gtk4 + relm4 or libadwaita (UI), oo7 (Secret Service keyring).

## UI implementation notes

- Build the UI from `design/screens/`. They're HTML mockups, not code to ship;
  translate them into the chosen toolkit faithfully (spacing, sizes, copy).
- The tray panel is Crake's own popover window under the tray icon. The native
  AppIndicator menu stays as a plain fallback.
- **How the panel is placed (decided with the user):** GNOME Wayland won't let
  an app place windows, so `crake` runs through XWayland (it unsets
  `WAYLAND_DISPLAY`; `CRAKE_WAYLAND=1` opts out). The tray extension's
  `Activate(x, y)` gives the click position; `place.rs` opens the panel next
  to it: upward from a bottom bar (the user runs Dash to Panel), downward from
  a top bar, within that monitor. It closes on focus loss. Placement needs a
  small patch to GPUI's X11 backend, vendored in `vendor/gpui-pre-linux`
  (see its `CRAKE-PATCH.md`; it also makes GPUI follow Xft.dpi while
  running, found 2026-10-14: a laptop at 100% docked to a HiDPI monitor
  opened every window at half size; `scripts/x11-test.sh` proves it on
  Xvfb). Crake's own XRandR monitors are divided by that scale
  (`sys::monitors`): the tray's click comes in logical pixels. Kept current (requested, 2026-10-12: no
  stale dependencies): the patch is `vendor/gpui-pre-linux.patch`,
  `scripts/update-gpui.sh` re-applies it to the newest gpui-kit's GPUI
  (and drops the vendored copy once upstream has the change), the weekly
  `.github/workflows/gpui.yml` opens a pull request or an issue, and the
  lint gate fails if Cargo stopped using the patched copy
  (`scripts/check-gpui-patch.sh`: a mismatched `[patch]` is only a Cargo
  warning). Dependabot (`.github/dependabot.yml`) keeps every other crate
  and the actions current, weekly, grouped; it leaves GPUI alone. A GNOME Shell extension would be the most native
  option; not now.
- UI is GPUI via `gpui-kit` 0.7, like the sibling app tern
  (`~/RustroverProjects/tern`); follow its patterns. `CRAKE_DEMO=1` shows the
  design's sample data for side-by-side checks with `design/screens/`
  (render those with headless Chrome; capture the app via the screenshot
  portal, crop to the panel, delete full-screen captures).
- Follow the system light/dark preference; tokens for both are in DESIGN.md.
- Copy voice is part of the spec. Reuse the sentences from the screens.

## Milestones

What each milestone built, how it's proven and what was found on real
gateways: **docs/engineering/milestones.md** (read the milestone's part
before changing that area; add to it as CLAUDE.md was added to). In
short:

1. Hook plus daemon around stock openconnect: no routes, D-Bus state,
   log. Done (real gateway).
2. nftables sets with fwmark routing; add, pin, forget. Done.
3. DNS learning via systemd-resolved, routed before the first packet;
   the public-NXDOMAIN rescue, split-horizon, keep names private
   (crake-hush), over TCP, IPv6. Built, netns- and real-gateway-tested.
4. Connection probing and learning; Undo; the first-SYN race; ICMP
   unreachable; a network in trouble; public hosts rechecked; UDP. Built.
5. Several tunnels: try order, "when two answer", collisions, one
   address for two companies. Built.
6. UI: tray, panel, notifications, log, sign-in, Settings, New tunnel,
   tunnel detail, Traffic, help cards, no tray, Beta protocols. Built.
7. IT's managed tunnels and their scope; helpdesk copies; reports
   (Settings → Diagnostics). Built.
7b UDP-blocking networks (over TCP, remembered) · 7c animations ·
7d AnyConnect-only gateways · 7e certificates in the keyring ·
7f Fortinet (SSL-VPN sign-in; EMS: never spoof, see
   [[fortinet-ems-no-spoofing]]) · 7g OpenVPN 3 · 7h a tunnel's DNS that
   stops answering (crake-keep) · 7i OpenVPN with `openvpn` on every
   system · 7j real gateways in CI through Cloudflare, the Doctor ·
   7k IPsec (IKEv2) with strongSwan, Linux.
8. Someday: other platforms (Windows and macOS are built: below).

## A newer Crake (built 2026-10-14; design/proposals/new-version.html)

`versions.rs`. (1) craked's D-Bus `Version` is newer than the app (a
package updated it; the app reads it when it connects, and on the system
bus it watches craked's name change owner (`client::owner_changes`) and
reads everything again: until 2026-10-15 a craked restarted by the
package went unnoticed, since the bus connection stays up, and the notice
never came; the tests' private socket drops, so they hadn't seen it):
the panel says "Crake was updated to X. Restart it to
finish."; Restart starts the new binary (Linux: the path without
" (deleted)") with `CRAKE_RESTARTING`, which waits up to 10 s for the old
one's single-instance socket or pipe to go (`single::taken`), and quits.
(2) Where nothing else updates Crake (`updated_elsewhere`: not the
dorhawk apt/dnf repository, pacman, the Nix store, Homebrew's cask),
once a day (looked at hourly, the time in app.json) the app asks
https://packages.dorhawk.software/versions.json with the system's curl and
says "Crake X is out. You have Y." with Download (the release page) and
Not now (`version:X`, so the next version is said again). On by default
(decided by the user: "automatic updates are amazing"); Settings → General
"Tell me when there's a new version" switches it off and is only shown
where it applies.

## Not now (written down to come back to)

**The rule (the user, 2026-10-14): whatever we decide not to do right now
is written down, here or next to the code it concerns, in the same
commit.** A doc comment on a stub isn't enough; prefer code that fails
loudly over a silent stub. Done items leave this list. Others are
recorded where they belong: the WFP driver and OpenVPN on Windows
(Other platforms), Windows code signing, bundling openconnect on macOS,
"Start at login" on macOS (Releases), distros without systemd-resolved,
the AUR account, Fortinet's tests (7f, CLAUDE.local.md).

- **Zed upstream, GPUI change 3 (Xft.dpi at runtime; 2026-10-14).** The
  branch is ready (github.com/ondrej-simon/zed, `x11-follow-xft-dpi`, one
  commit); the user files the issue and opens the PR in their own words
  (Zed's AI policy: no agent-written PRs or descriptions). Related, not a
  duplicate: zed-industries/zed#21851. See CRAKE-PATCH.md.
- **Two sign-in tabs, and sessions stopped before sign-in (2026-10-14).**
  Likely openvpn3-indicator, which acted on every session (Ready,
  Connect, the sign-in page): the user removed it the same day, and a
  session Crake 0.6.1 started then stayed up when the app quit (the
  morning's "quitting ends the session" was probably the indicator too;
  7g's note that sessions outlive the app holds). Left here until a
  web sign-in without the indicator shows one tab and no early stop.
  Signing in to an OpenVPN 3 web sign-in profile often opens two browser
  tabs; probably Crake and openvpn3-indicator each opening the same
  session's page (7g). One morning four sessions in a row were stopped
  5 to 50 s after starting, before the sign-in finished, so the browser
  said "login success" with no session left to take it; the logs don't
  say who stopped them. To do with the user: watch one sign-in live
  (sessions, their sign-in pages, who calls Disconnect), and try once
  with openvpn3-indicator quit. If the indicator opens the page too,
  Crake could leave it to the indicator while it runs.
- **openvpn3 leaves a session's process running (2026-10-14/15, v27.1).**
  Cause found: openvpn3's session manager gives a new session's process
  5 s to answer Connect; when two front-ends told the same new session to
  connect at once (Crake and openvpn3-indicator, which acts on every
  session), the call ran out ("Async call failed … Connect … Timeout was
  reached" in the journal), openvpn3 later stopped and dropped the
  session, and the process (openvpn3-service-client, user `_openvpn`)
  kept running: invisible to `sessions-list`, reconnecting on its own for
  days, each reconnect rewriting the other tunnels' DNS ("~." and the
  default route). Of 25 sessions in two days, exactly the two with that
  timeout were left over; none since the indicator was removed. Built
  (2026-10-15): Crake doesn't send Connect to a session already under way
  (`ovpn3/connect.rs`), waits out the timeout instead of giving up, and
  ends a session it started whenever it can't follow it; the app notices a
  process it saw belong to one of the user's sessions running a minute
  after its session went (`ovpn3/leftover.rs`) and craked logs it with
  `sudo kill -9 <pid>` (D-Bus `LeftRunning`, a notification too), and the
  panel shows it until the process ends or "Got it"
  (design/proposals/leftover-openvpn3.html, A2: the command with Copy and
  Got it on its line; leftover-warning.html, 3: tinted amber, the bird's
  head with an amber triangle, also on the tray's head, where a red trouble
  badge wins; `panel/leftover.rs`, `tray::Badge`; `CRAKE_DEMO=leftover`).
  Crake doesn't end the process itself (the user: not Crake's to kill;
  craked could, with an AppArmor rule to signal outside its profile). Not now: a
  process left from before the app started isn't recognised (telling it
  from another user's session needs root: craked can't see the user's
  sessions, the app can't see others'); worth reporting to OpenVPN (a
  Connect that times out shouldn't orphan the process).
- **macOS: what CI can't check of craked registered by the app.** The
  user's approval (no runner can click it), so craked under launchd from
  Crake.app, and an upgrade's restart of a registered craked
  (`launchctl kickstart -k`, the tunnels handed over) are tried on a real
  Mac only: the colleague's, first with 0.7.
- **The journey's first connection reset once in the NixOS VM
  (2026-10-14, CI run 38620915461 on 6e8149c, a commit that changed only
  CI files).** Step 2's GET of 10.40.1.10 failed with "Connection reset by
  peer" (journey_tests.rs, `get`), where the first-SYN race should hold
  the SYN until ACME takes the host; the same journey passed on f5a4efc
  and on every distribution's package test. Not reproduced. If it comes
  back: craked's log around the race (`race_held`: did direct "answer"
  with a reset from the gateway VM, which is also the client's default
  route?), and whether the app saw a reset or a refused connect.
  **Again on 2026-10-18** (run 39135556942 on 8cba704, step 2 again,
  "Connection reset by peer" 0.3 s after ACME connected). The gateway VM
  drops what the LAN sends to ACME's network (no reset from there), and
  its own dhcpcd had just taken ocserv's new vpns0 ("waiting for 3rd
  party to configure IP address"), as dhcpcd once took the client's
  crake-acme. The gateway VM now keeps dhcpcd off vpns* (test.nix). If
  it comes back after that, the cause is elsewhere: the gateway's side
  (ocserv's routes to the client right after connect) is next to look at.
- **All unsafe code behind one safe layer (asked for by the user,
  2026-10-14). Done 2026-10-19.** `crates/crake-os` holds every unsafe
  call Crake makes, each behind a safe function whose contract the crate
  keeps itself, with its `// SAFETY:` reasoning: Unix and Linux
  (getifaddrs, interface names and indexes, signals, socket options,
  netlink sockets, tun devices for tests, tokio's `AsyncFd::register` only
  for `OwnsFd` types, WAYLAND_DISPLAY unset only while /proc says the app
  has one thread), macOS (sysctl, link counters, CoreFoundation's arrays,
  dictionaries and run loop, utun devices for tests; with `app`,
  SMAppService and the displays), Windows (IP Helper, adapters, interface
  counters, the TCP tables and their statistics, the registry, Credential
  Manager, the service manager, console events and sessions; with `app`,
  monitors, window placement and frames by the handle's value). crake,
  craked, crake-hook, crake-proto and lint-literals `#![forbid(unsafe_code)]`;
  `scripts/check-unsafe.sh` (lint gate) fails on `unsafe` anywhere else,
  tests included (they no longer set the environment: fake scripts get
  their paths written in). Not yet: unit tests of crake-os's Windows and
  macOS parts on their CI runners beyond what the journeys exercise.
- **Small screens (audit 2026-10-19, the user's question; fixed then:
  Settings capped to its screen, the panel's middle scrolls with its
  header and footer fixed, the sign-in card stops at its screen's bottom
  and scrolls, New tunnel's dropdowns scroll; `panel::tests::
  many_tunnels_keep_the_panel_on_its_screen`).** Not yet: tunnel detail
  builds every host row at once and puts "Add a host" after them (fine
  on screen, slow and far down with hundreds of hosts: `uniform_list`,
  and "Add a host" above the list, a design question); no window but the
  profile editor can be resized (`is_resizable: false`); the room left
  for bars is a fixed 120 px off the primary monitor, not the work area
  of the monitor the window opens on (X11's work area was unreliable
  across monitors, place.rs); Traffic and the profile editor open as
  tall as a big screen allows (`max_window_height_or` ignores its
  fallback when a monitor is known); the panel's scrolling middle shows
  no sign it scrolls but its cut-off last row.
- **Display scale changes on Windows and macOS: not tried.** GPUI follows
  them natively there (WM_DPICHANGED, backing scale), and Crake's monitor
  lists are logical; no one has docked a running Crake to a monitor with
  another scale on either.
- **winget for Windows.** The in-app "Crake X is out" notice is built
  (`versions.rs`); Windows users would rather be updated by winget, like
  Homebrew's cask on a Mac. Then `versions::updated_elsewhere` should
  know a winget install.
- **Undo in macOS notifications.** notify-rust has no actions there, so
  "Use Nordbank" / Undo exist only in the panel on a Mac.
- **OpenVPN 3 sessions whose device openvpn3 doesn't name, under the
  package (2026-10-15).** craked finds such a device from the session's
  process (`/proc/<pid>/fdinfo`), which the packaged service can't read
  (openvpn3 runs as `_openvpn`; no CAP_SYS_PTRACE, and AppArmor would need
  a rule). Seen only with an Azure gateway's profile, which the user's
  company is dropping. Options if it matters: the capability and an
  AppArmor rule (a wide permission for one case), or matching the
  session's address to a tun device's.
- **A shared practices repository for dorhawk.software (the user's idea,
  2026-10-16; written down so it isn't lost, to do once Crake is stable).**
  What the user has in mind, in their words' spirit: Crake grew a lot of
  hard-won engineering practice (lints as hard errors, a magic-number lint,
  file and function size limits, coverage floors that only rise, a journey
  regression suite on every supported system, tests that never touch the
  developer's machine, split and cached CI, pruned caches, two-layer Nix
  dependency builds, release scripts that refuse an empty changelog, a
  hand-written changelog in the product's voice, design proposals before UI
  changes, "write down what we don't do now", nice'd builds, never
  `nix path:` on a repo with target/...). That knowledge should outlive
  Crake and help the sibling apps: tern (a Rust/GPUI mail client, on hold
  until Crake is stable) and Obálka (TypeScript, React Native, which has
  tried good patterns of its own). The plan:
  1. A new **public** repository under dorhawk-software describing the
     practices: each one with what it is, why it's worth it (the incident
     or cost that taught it, e.g. the 328 GB disk fill, the cache
     evictions), how to apply it, and how to check it's applied.
     Actionable, not essays.
  2. **Structure: common plus per language.** Practices that hold anywhere
     (changelog discipline, deferrals written down, design proposals,
     release gates, CI caching ideas, privacy rules for public repos) in a
     common part; Rust specifics (clippy deny, lint-literals, cargo deny,
     machete, llvm-cov floors, GPUI patterns) and TypeScript/React Native
     specifics (from Obálka) in their own parts, with room for more.
  3. **Central management:** project repositories register with it (or
     hook into it), and when the practices change, the change reaches every
     registered project (e.g. a workflow opening a pull request there), so
     the practices are available to future agents in each repository as
     skills, CLAUDE.md sections and checks, not just as a document.
  4. **Usable beyond dorhawk.software:** any repository could adopt it as
     its baseline, like a runbook of what was learned. How the hook works
     (a template, a GitHub Action, a CLI, a skills package) is to be
     designed when we start; this note is the reminder of the intent.
- **IPv6 hosts on a network without IPv6 (Linux; found 2026-10-20 by
  the scenario suite).** Crake routes by mark, after the system's own
  route lookup; on a network with no IPv6 route at all (much office
  Wi-Fi), an app's connection to an IPv6 address fails at once
  ("Network is unreachable"), so an IPv6-only internal host (a name with
  only an AAAA record) can't be reached through a tunnel there, and
  there's no packet to probe from. macOS' scoped default route has the
  same effect, and macOS' resolver asks no AAAA at all there (CI's Mac:
  an IPv6-only name came back empty); Windows' tunnel default route
  (metric 9000) is taken by all IPv6 there instead. Fixing it on Linux means a main-table route of
  Crake's (e.g. IPv6 default via the tunnel, unmarked packets refused),
  which goes against "no routes but the DNS's". **Decided with the user
  (2026-10-20): left as it is** (IPv6-only internal hosts are rare;
  dual-stack names work). If a real gateway shows one: a lowest-priority
  IPv6 default route into the tunnel only while the network has no IPv6
  of its own, with anything unmarked refused at once. The e2e network has
  IPv6 (as most do); the probe scenario says `not_here` where the
  runner's network has none.
- **Windows on ARM64 (planned with the user, 2026-10-20).** The user's
  coming Apple Silicon Mac runs Windows 11 for ARM in a VM, which runs
  x64 programs emulated but never x64 drivers, and Crake for Windows is
  x64 throughout (its programs, openconnect, Wintun, ovpn-dco,
  tap-windows6). An ARM64 build: `aarch64-pc-windows-*` for crake,
  craked and the hook, openconnect built for ARM64
  (scripts/build-openconnect-windows.sh), Wintun's and OpenVPN's ARM64
  drivers (both ship them), an installer per architecture (or one
  choosing at install), and CI building and testing it (GitHub has
  Windows ARM64 runners). Detail in docs/engineering/platforms.md when
  started.
- **Browser sign-in on Windows.** The openconnect Crake builds for
  Windows has no `--external-browser`; New tunnel and sign-in say so
  (2026-10-20). Building it in (and an opener there: `BROWSER` is
  xdg-open on every system but macOS) is for when someone needs it.
- **Untried on real systems:** see "Tried by real people" below, a
  checklist of its own.
- **Performance: never measured.** Believed negligible (NFQUEUE holds
  only DNS answers and racing SYNs); measure before claiming numbers.
- **A colleague's requests (they run OpenVPN 3 profiles daily,
  2026-10-14).** For every user, not only experts, except where said:
  1. *See the traffic, animated* ("packets ticking by"), their first and
     main wish: a live graph of what goes direct and what through each
     tunnel, with tables of hosts and connections only as the detail.
     **Built 2026-10-14** (Traffic, milestone 6; the user and the
     colleague liked design/proposals/traffic-window.html).
     The colleague found the first dots too fast and distracting; the
     proposal now moves them as fast as each path's traffic (a trickle
     crawls, a big download about twice as fast) and sends one per
     ~250 KB, between one every four seconds and two a second.
  2. *See the split as a whole*: every rule in one place (learned,
     pinned, never, direct by Undo, everything else), the "tree" they'd
     otherwise keep in a config file (that proposal's "Where things go"
     table: built with it).
  3. *"How is it secured, can it break my traffic?"*: **built
     2026-10-15** as "Is it safe?" in Settings → About
     (design/proposals/how-secure.html, A, chosen by the user; four
     questions and answers between the links and the coffee); the same
     words can go on the website with the explainers below.
  4. **The power user feature:** *edit an OpenVPN 3 profile in Crake*:
     **built 2026-10-15** (design/proposals/edit-ovpn-profile.html,
     option A, chosen by the user; `profile_editor/`, `ovpn3/profile.rs`).
     Tunnel detail's Profile fact has "Edit…" on its label row ("Locked"
     when openvpn3's `locked_down` hides the text; a long name is cut and
     shown whole on hover, as the Gateway's). The editor window shows the
     text as openvpn3's Fetch gives it, line numbers, each inline block
     folded into a muted line ("<ca> certificate, 31 lines, kept as they
     are </ca>", put back byte for byte; deleting the line drops the
     block, changing it is refused before openvpn3 sees anything),
     changed lines tinted. "Save and reconnect" imports the new text under
     the same name (persistent as the old one), sets the old one's tags
     and overrides on it, then removes the old one; openvpn3's refusal is
     said under the text and nothing changes. A tunnel that's on is
     switched off and on again (`Request::Reconnect`). GPUI's editor: the
     folded lines are text, not chips; the tint covers a line's text, not
     the whole line. Tests: fold/unfold units, window tests with a
     stand-in for openvpn3, a journey step (its container has no openvpn3:
     the window says it couldn't read the profile), and
     `ovpn3::profile::tests` against a real openvpn3 with a throwaway
     profile of its own (CRAKE_OPENVPN3_TEST=1; not in CI).
- **The Traffic window's open ends (built 2026-10-14; left for later):**
  - *UDP on Windows, and unconnected UDP on macOS:* Windows' UDP table
    has local endpoints only, so UDP conversations (QUIC, HTTP/3) aren't
    listed there; on macOS only sockets connected to one address are.
    Their bytes still count in each way's rate. Windows would need a
    packet-level view (ETW's network events or the WFP driver noted under
    Other platforms).
  - *Windows and macOS' connection lists: built 2026-10-15, tested by CI
    only* (a loopback connection's bytes, on the runners); not yet seen
    on a real PC or Mac (the checklist under "Tried by real people").
  - *Which app opened a connection* (the proposal's "App" column): a
    connection to its process (Linux: the socket's inode in /proc/*/fd,
    root only); not built.
  - *Pointing at a path or destination* to name it and its rate, and
    clicking one to find it in the table (the proposal's notes): not built.
  - *AppArmor:* the profile now allows `nf_conntrack_acct`; not yet run in
    `scripts/vm-test.sh` (AppArmor enforced, KVM; not in CI).
- **The explainers** (docs/explainers.md): written down, not yet drawn or
  published anywhere.
- **The website's version number** (dorhawk-website): visitors get the
  live one from versions.json; the page's fallback is bumped by hand at
  each release. Automating it (the packages workflow updating the page, or
  Cloudflare filling it in) was offered, not decided.
## Tried by real people (a checklist, ticked one by one)

Asked for by the user, 2026-10-14. The tests prove the mechanics on every
system (containers, VMs, CI's runners), but nearly every macOS fix of
October came from a colleague using Crake on a real Mac: System Settings'
names, the service's approval, windows behind other apps, another VPN
client's names kept private, a panic. Windows and the other Linux
distributions haven't met a real person yet. Each item is ticked when
someone used Crake for it on a real machine (not a VM made for tests),
with the date and who (a role, not a name), and what was found and fixed.
Ubuntu 26.04 with GNOME and a bottom bar is the user's own notebook, used
daily.

**Windows** (a real PC, ideally a company laptop):
- [ ] Installing: SmartScreen's warning for the unsigned installer, and
  what a user has to click past (Windows code signing: applied to SignPath,
  see Other platforms).
- [ ] Antivirus and endpoint security: craked's service, Wintun, and the
  routes, NRPT rules and firewall rules it writes, not blocked or flagged.
- [ ] Another VPN client installed (AnyConnect, GlobalProtect, ...): its
  names still resolve with "Keep internal names private" on
  (`served_elsewhere`, built 2026-10-18: other programs' NRPT rules and
  the adapters' DNS suffixes; the VM test adds another client's NRPT rule).
- [ ] Group policy that owns NRPT or the firewall: Crake's rules work, or
  it says plainly that they can't.
- [ ] Finding the bird: Windows 11 puts new icons in the notification
  area's overflow; what a new user sees, and whether Crake should say how.
- [ ] The panel with a GPU: Windows' rounding and shadow (only seen in a
  VM without one).
- [ ] A day of real use: sign-in, hosts learned, Undo; whether the first
  connection to a new host failing (no WFP driver) is noticed.
- [ ] Display scale changes with Crake running (also in Not now).
- [ ] The Traffic window: connections listed with their ways and rates
  (extended TCP statistics switched on by the service).

**Linux distributions** (a real desktop, not the package test's container):
- [ ] Fedora with GNOME: no tray icons without the AppIndicator extension
  installed and switched on. Crake now tells them (built 2026-10-16,
  `no_tray.rs`, `tray_host.rs`, `sys/linux/gnome_tray.rs`;
  design/proposals/no-tray-after.html): to check on a real Fedora that
  "Switch on AppIndicator" (EnableExtension) brings the bird at once, that
  a just-installed extension marked in enabled-extensions is on after one
  login, and that the panel opened by hand gets focus there.
- [ ] KDE Plasma, X11 and Wayland: the tray, and the panel placed at it.
  The KDE VM was removed for disk space (2026-10-14); a VM can be made
  again (`~/.cache/crake-vms/make-vm.sh`) or a real machine used.
- [ ] Debian's desktop, which doesn't use systemd-resolved: installing
  Crake brings it in; the rest of name lookup (NetworkManager, other VPNs,
  `.local` names) still works afterwards.
- [ ] Fedora's firewalld: its own reverse-path filter (IPv6_rpfilter, in
  nftables) doesn't drop replies through a tunnel, as strict `rp_filter`
  would have (handled).
- [ ] Arch: resolved isn't enabled on install; following the install
  message gets a working Crake.
- [ ] Native Wayland (`CRAKE_WAYLAND=1`).

**macOS** (beyond what the colleague has tried):
- [ ] The service approved by the user, craked running under launchd from
  Crake.app, and an upgrade restarting it with tunnels kept (0.7; also in
  Not now).
- [ ] The panel under the menu bar, placed at the icon on each display.
- [ ] The Traffic window: connections listed with their ways and rates.

**Everywhere:**
- [ ] Importing a real .ovpn file (New tunnel's Import .ovpn…).
- [x] OpenVPN run by Crake itself against a real gateway (2026-10-19,
  the user, on 0.11.0: browser sign-in, the company's internal network
  reached).
- [ ] Each Beta protocol against a real gateway (only AnyConnect and
  OpenVPN 3 have met one; the Beta tag comes off as each is ticked).

## Lints are errors

Any lint stops the build; fix it and build again, never silence it.
- `Cargo.toml` `[workspace.lints]`: every rustc warning (unused imports
  included) and every clippy lint is `deny`.
- `scripts/lint.sh`: `cargo fmt --check` (run `cargo fmt`), clippy (all
  targets), `cargo deny check` (`deny.toml`:
  known vulnerabilities anywhere, unmaintained crates Crake uses directly,
  licences compatible with GPL-3.0-or-later, crates.io only) and
  `cargo machete` (unused dependencies). Tools in `~/.cargo/bin`.
- Size (requested): at most 700 lines of code per source file (its tests
  not counted) and 1000 per integration test file (`scripts/check-size.sh`),
  at most 200 lines per function (`clippy::too_many_lines`, `clippy.toml`).
  Only an inline test module (`#[cfg(test)]` over `mod x {`) isn't
  counted: until 2026-10-19 any `#[cfg(test)]` was taken as the start of
  the tests, and new_tunnel/mod.rs's `mod tests;` near the top had let it
  reach 736 lines unseen.
  Past that, split by topic: a folder module with one file per part (the
  pattern of `manager/`, `new_tunnel/`, `settings_view/`, `panel/`), or a
  function's parts into named helpers.
- Magic numbers (requested 2026-10-12): `crates/lint-literals`, run by
  `scripts/lint.sh`, fails on a number in a crate's own code (not its
  tests) that has no name. Name it (a const, a static, an enum
  discriminant), or keep it with `// literal: <reason>` on its line or
  alone on the line above; a reason alone above an item (fn, impl,
  `mod x;`) keeps the whole item, for data copied from elsewhere (the
  design's drawings, demo data, a protocol's table). Free without a name:
  0, 1, 2, and UI layout inside `px(..)`/`rgb(..)`/`rgba(..)` (it mirrors
  design/screens and DESIGN.md's tokens). It follows each crate's module
  tree from its roots, so `#[cfg(test)]` modules are left out. A crate
  is in the list in lint.sh: every crate (crake-proto, craked, crake and
  the tool itself) is clean.
- `.claude/settings.json`: a PostToolUse hook runs `scripts/lint.sh` after
  every `cargo build` / `cargo test` from Claude Code and blocks on failure.
- Coverage (`scripts/check-coverage.sh`, cargo-llvm-cov, about a minute):
  each crate's share of lines run by the tests stays at or above its floor
  (crake 73%, craked 63%, crake-proto 98%, lint-literals 82% as of Oct 2026; crake's
  without the tests that need a real gateway, the scenario suite, e2e tests,
  journeys and their helpers: test code measured by running it, 2026-10-20). The pre-commit
  hook runs it when Rust files are committed. Raise a floor as tests grow;
  never lower one to let a change through.

## Testing without installing anything

The user doesn't want development builds installed on their machine.
How each kind of test works, the scenario suite's design and what it
found, the e2e containers, CI's jobs and caches:
**docs/engineering/testing.md**. The rules:

- **The journey suite is the regression suite: it stays green; every
  change (refactors above all) runs it, and every new entry point of the
  app gets a step in it.** It runs on every supported system after every
  change (every Linux distribution's package test, Windows, macOS).
- **The scenario suite (`crates/crake/src/scenarios/`): one scenario per
  test, written once, run on every system against every kind of gateway**
  (AnyConnect, OpenVPN, IPsec on Linux; Globex, certificates, codes,
  groups, IT's tunnels, IPv6). A scenario a system can't run says so
  (`not_here`), never passes quietly. New behaviour gets a scenario.
- Tests never touch this computer: e2e and netns tests run in containers
  with no network (`scripts/e2e-test.sh`, `CRAKE_E2E_FILTER`,
  `CRAKE_E2E_KEEP`; `scripts/netns-test.sh`); AppArmor only in VMs
  (`scripts/vm-test.sh`); Windows in its VM (`scripts/windows-vm-test.sh`).
- A new D-Bus call gets a step in `wire_tests.rs`; every window has
  headless window tests (`.test_support()` makes an element clickable).
- Daily use, nothing installed: `sudo ./target/debug/craked --config
  dev/<yours>.toml --dev`, then `./target/debug/crake`; or `--connect
  <tunnel>` from a terminal. Demo mode: `CRAKE_DEMO=1`, `CRAKE_SHOW=…`.
- Screenshots: `scripts/xwin.py` (no flash); the portal flashes the
  screen, so one or two, never a burst.
- CI caches are saved by main only and pruned; GitHub's 10 GB can't be
  raised; Nix goes to Cachix (`crake`).

## Releases (decided with the user, 2026-10-13)

`CHANGELOG.md`, written by hand for the people using Crake, in its copy
voice (not generated from commits, which are written for developers):
**every user-facing change adds a line under "Unreleased" in the same
commit.** Dependency updates too (asked by the user, 2026-10-19): a
merged Dependabot update, or one fixed by hand, gets a line saying what
was updated and what it means for the people using Crake, since a
security fix in a dependency warrants at least a hotfix release, and
release.sh refuses an "Unreleased" with nothing in it. To release, `scripts/release.sh <version>` (asked for by the
user, 2026-10-13; `--dry-run` first shows the changes and pushes
nothing): it refuses an empty "Unreleased", dates it
(`## <version> (<YYYY-MM-DD>)`), puts the version into Cargo.toml (and
Cargo.lock), packaging/nix/package.nix and packaging/arch/PKGBUILD,
commits "Crake <version>" and pushes, waits for CI on that commit and
pushes the tag `v<version>`. Interrupted, or after a fix CI asked for, run
it again with the same version: it waits on main's latest commit and tags
it. Don't push anything else to main while it waits (a push cancels the
CI run it's waiting for).
`.github/workflows/release.yml` refuses a tag without its section, with a
version that disagrees, or on a commit CI hasn't passed
(`scripts/release-notes.sh`); it builds the .deb and RPM, the Windows
installer (openconnect included, unsigned) and the macOS package (signed,
notarized, installed and removed on the runner) in release mode with the
same scripts CI uses, and publishes them with SHA256SUMS as the GitHub
release, its text the changelog's section. Then the Homebrew cask
(`packaging/macos/crake.rb.in`, the version and the package's checksum
filled in) is pushed to **dorhawk-software/homebrew-tap** (public, created
with the user's go-ahead 2026-10-13; `brew install --cask
dorhawk-software/tap/crake` installs Homebrew's openconnect with it) with
a write deploy key of that repository only (secret
HOMEBREW_TAP_DEPLOY_KEY; the organisation had deploy keys off, the user
switches them on). **The apt/dnf repository (built 2026-10-13; moved to its own
repository the same day, asked for by the user: no app should own what
the apps share):** `packages.dorhawk.software` is published by
**dorhawk-software/packages** (local: ~/Programming/Personal/dorhawk-packages):
the R2 bucket `dorhawk-packages` (the user's Cloudflare account; custom
domain, TLS 1.2 minimum, the r2.dev URL off; `/` serves index.html
through a URL rewrite rule on the dorhawk.software zone), its signing key
("dorhawk.software packages", B17A7118E97736C202AD5EAFF1D152DA1BBD1C97,
RSA 4096, signing only, no expiry; private half only in the user's
password manager and that repository's secret), the R2 token, the start
page and the scripts all live there, with `apps.json` listing the apps
(Crake for now). Its workflow fetches each app's latest release, tests
the repository in no-network containers with a throwaway key, signs the
RPMs, builds and signs the indexes, uploads them in order and installs
from the live site; it runs when an app's release asks
(`repository_dispatch` "release"), on a push, by hand, and hourly (stopping
at once when nothing is new). Crake's release only asks, in its
`packages` job, with the optional secret PACKAGES_DISPATCH_TOKEN (a
fine-grained token of the user's: Contents read/write on that repository
only; without it the hourly run finds the release). Crake's own release
RPM is no longer signed; the repository signs what it serves. The
README's "Installing" has the commands. **The AUR (2026-10-13):** the
release's `aur` job runs `scripts/aur-publish.sh <version>` (the PKGBUILD
with the version and the tag's source tarball's real checksum, its install
script and a .SRCINFO made in an Arch container, where makepkg checks the
checksum; pushed to ssh://aur@aur.archlinux.org/crake.git with the secret
AUR_SSH_KEY; `--dry-run` pushes nothing). Skipped while there's no secret:
AUR registration was closed that day (bot sign-ups); once the user has an
account with the key, publish the current release by hand with the
script, then releases keep it current. "Start at login" on macOS is
the app's own login agent, registered with SMAppService (from 0.7; a
LaunchAgent in ~/Library/LaunchAgents before).

## Packaging and dependencies (decided with the user)

Rule: **where the platform has a package manager, depend on its packages;
where it doesn't, bundle, and own the updates.** Never bundle openconnect
or other system software in Linux packages. The .deb (built on 22.04 for
every Ubuntu and Debian), RPM, Arch, NixOS, AppArmor, SELinux, upgrades
that keep tunnels up, Ubuntu 22.04's limits, the engine checks:
**docs/engineering/packaging.md**. Not Flatpak or Snap (craked needs the
host's network).

## Other platforms: Windows and macOS

Windows first, then macOS (decided with the user, 2026-10-11); both built
and tested in CI. The phases, how each OS part works (Wintun, IP Helper,
NRPT and craked's DNS server, the service, the installer; utun, routes,
/etc/resolver, pf, the dynamic store, SMAppService, signing), what can't
work there without a WFP driver, and the open decisions:
**docs/engineering/platforms.md**. Rules from it that hold everywhere:
Windows draws its own window frames (never Crake's rounding there);
no Windows code signing yet (SignPath applied for); never revoke the
Apple certificates unless a key leaked.

## Someday: other platforms

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
