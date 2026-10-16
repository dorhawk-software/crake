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
   it alone and log "probably down".

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

1. Hook plus daemon skeleton around stock openconnect for one tunnel: no
   routes, D-Bus state, log. **Done** (tested against a real gateway).
2. nftables sets with fwmark routing; manual add, pin and forget via D-Bus.
   **Done** (tested in a throwaway network namespace).
3. DNS learning via systemd-resolved; "routed before the first packet".
   **Built**, proven in a throwaway namespace (tests fail if an answer is
   released before its route is in, or if the public-failure rescue is off);
   tested on the user's real AnyConnect gateway from a clean state:
   internal sites by name and an Azure private Postgres endpoint both work
   with no configuration.
   Done: tunnel DNS + domains per link in resolved (routing-only, not default
   route); answers from tunnel DNS held in NFQUEUE (`bypass`) until their A
   records are in the learned set; public NXDOMAIN answers (any server, any
   non-tunnel interface) held while the tunnels are asked in config order,
   and replaced by the first tunnel's answer that knows the name (e.g. Azure
   private endpoints); names no tunnel knows aren't asked again for 5 min;
   resolved's cache is flushed when a tunnel goes up or down.
   Split-horizon ("public name, private address"): a public answer for a
   name under a tunnel's organisation domain (the gateway's registrable
   domain, vpn.acme.example → acme.example) is held while that tunnel's DNS is asked;
   if it knows a private address the public one doesn't, and that address
   answers through the tunnel (443 or 80), it is routed and the app gets the
   tunnel's answer. Otherwise the public answer stands; names without a
   split aren't asked again for 5 min. Found with a real customer portal
   whose public address resets clients from outside the company while the
   company's DNS gives a private one (CLAUDE.local.md). Only A queries; the policy scope, undo and never apply.
   "Keep internal names private" (Settings, on by default): the domains of
   tunnels that are off (as pushed when last up, plus configured) go to a
   dummy link `crake-hush` in resolved, whose DNS (169.254.53.53, craked)
   answers REFUSED at once (`hush.rs`; netns-tested, and verified with the
   user's systemd-resolved: looking up an internal name with its tunnel
   off fails at once); removed on shutdown. Not a domain another program
   serves (2026-10-14: a colleague's AnyConnect on a Mac failed every
   internal name while Crake refused them): `Resolved::served_elsewhere`
   (Linux: resolved's domains on links not Crake's; macOS: every network
   service's DNS in the system configuration, where VPN clients put
   theirs), looked at again every 2 s (`refresh_hush`, rewritten only when
   the list changes); the package test and tests/macos.rs check it.
   Over TCP too: a tunnel DNS answer that fits one segment (almost every A
   answer) is held like a UDP one; one spread over several segments is put
   together from them in the order they come (`dns_tcp.rs`, in the queue's
   receive loop): the earlier ones pass at once, the one that completes it
   is held until its addresses are routed, so the app still can't have it
   before then (netns-tested, fails without the reassembly).
   IPv6 addresses in answers (AAAA, from a tunnel DNS reached over IPv4)
   are routed before the first packet too, on Linux (netns-tested; the
   tunnel gets the IPv6 address openconnect hands over,
   `INTERNAL_IP6_ADDRESS`). A tunnel's DNS servers reached over IPv6
   (openconnect lists them in `INTERNAL_IP4_DNS`) are registered with
   resolved, get a /128 through the tunnel, and their answers are held
   like IPv4 ones (Linux; netns-tested). The public-NXDOMAIN rescue
   routes the AAAA records a tunnel knows too, and watches public DNS
   reached over IPv6 as well (a dual-stack network's router often hands
   one out; the rewritten answer gets the UDP checksum IPv6 requires;
   netns-tested). Still open: DNS-over-TLS upstreams (Crake can't see
   those failures).
   Split-horizon takes AAAA questions too (a unique local fc00::/7
   address counts as private), the memory answerer (7h) remembers and
   serves IPv6 addresses (an A and an AAAA answer each keep the other's),
   and "one address, two companies" covers IPv6 addresses (netns and
   lifecycle tests).
4. Connection probing and automatic learning; undo. **Built**, proven in a
   throwaway namespace (the fake gateway drops packets not from the tunnel
   address, like a real one; the test fails if the stuck flow isn't
   forgotten); tested on a real gateway: `curl https://<internal IP>` from
   a clean state got its answer in 4.3 s on the app's own connection ("It
   didn't answer directly. Through ACME it answered in 31 ms.").
   Done: conntrack events (passive, nothing held) spot new outbound TCP that
   gets no answer within `probe_wait_secs` or is refused (an unanswered entry
   destroyed early = reset); the host is tried through each routed tunnel in
   order with the tunnel's fwmark; the first that answers gets it, and the
   stuck conntrack entry is deleted so the app's next SYN retry is routed and
   NATed afresh. Never probed: own LAN/bridges (on-link), loopback etc., the
   gateway, already-routed hosts, the `never` list, hosts probed in the last
   10 min. Undo = send direct and keep direct (per tunnel) until redo; DNS
   learning respects it too. Recent public DNS answers give log lines a name.
   A router's ICMP "unreachable" (net, host, prohibited; never
   "fragmentation needed") from outside the tunnels goes to the same queue
   as DNS answers and starts the probe at once: "Unreachable directly.
   Through ACME it answered in 31 ms." (netns-tested).
   IPv6 on Linux: conntrack's IPv6 connections are probed the same way
   (passively; the probe's socket by family; the stuck entry deleted), a
   router's ICMPv6 "unreachable" (no route, prohibited, address
   unreachable, policy, reject route; never "port unreachable") starts
   the probe at once, and the first-SYN race holds IPv6 SYNs (all
   netns-tested).
   A network in trouble (2026-10-15: a wake from sleep, a flaky minute,
   and calendar.google.com "didn't answer directly. Through Globex it
   answered in 14 ms." went through a company's gateway for a week):
   while 2 other hosts answered neither directly nor through a tunnel in
   the last 30 s (`down_everywhere`), a public address isn't learned
   ("Left X alone." saying the network seems to be in trouble) and is
   tried again after a minute; private addresses are learned as before
   (netns-tested, fails without it). And a public address is tried
   directly once more, alongside the tunnels (2026-10-15: on a working
   Wi-Fi, one connection to api.anthropic.com got no answer, Initech's
   tunnel did, and the chat on it broke when that session expired):
   answering directly, it's left alone ("It answered directly when Crake
   tried again"); one a company allows only from its own addresses never
   answers directly and is learned as before, at most 3 s later
   (netns-tested, fails without it).
   UDP (Linux): with no handshake to try, a UDP host is tried with the
   app's own first datagram. The race chain copies a new UDP
   conversation's first datagram to the same addresses as the race
   (`udp dport != 53`; copied in the SYN queue and let go at once, even
   with `race_first_syn` off); conntrack reports UDP conversations
   (waiting until IPS_SEEN_REPLY); one unanswered after the probe wait is
   tried through the tunnels with that datagram (`probe::Attempt::Udp`),
   learned only on a reply through one, and its conntrack entry deleted
   so the app's next datagram is routed and NATed afresh. UDP no one
   answers (logs, metrics) is neither routed nor logged. netns-tested
   (fails without the copy rule, or with the entry deleted as TCP).
5. Multiple simultaneous tunnels, try order, conflict handling.
   **Requested:** detect tunnels that can't be up together. When switching a
   tunnel on, compare what it pushes with the tunnels already up: its own
   address/network, its DNS servers (Crake routes a /32 to each), pinned
   networks and pushed split networks. If they overlap, don't half-work:
   refuse (or stop right after connect) and say which one to switch off and
   why, e.g. "Turn ACME off to use Nordbank. Both use 10.0.0.2 for DNS."
   A single learned host both tunnels answer for is the softer case.
   **Built:** probing tries every eligible tunnel at once; "first in
   order" waits for the earlier ones, "faster" takes the first answer,
   "ask me" uses the first in order and then logs "Nordbank answers for X
   too." (LogEntry `others`) with a notification offering "Use Nordbank"
   (D-Bus `MoveHosts`: direct for the old tunnel, learned on the new).
   **Collisions: built** (lifecycle tests). When a tunnel comes up
   (`tunnel_up`, for openconnect and OpenVPN 3 alike) craked compares it
   with every tunnel that's up: a shared DNS server, overlapping own
   networks, or a network pinned to both. On a clash it doesn't half-work:
   it logs "Turn ACME off to use Nordbank." with the reason ("Both use
   10.0.0.2 for DNS."), stops an openconnect tunnel keeping its sign-in
   (SIGHUP), or refuses an OpenVPN 3 attach (the app then ends the session,
   `Update::AttachRefused`); no "Switched off" line follows. What each tunnel
   got last time (`last_dns`, `last_net` in the host store) gives the same
   sentence before switching on (ListTunnels `clash`): the app shows it and
   doesn't open sign-in.
   One address, two companies (the user's case: ACME's abcd.com and
   Nordbank's xyz.com both 10.0.0.1): an address another tunnel that's up
   already sends (learned or pinned) isn't taken by a second tunnel's DNS
   answer; craked says "xyz.com can't go through Nordbank." ("Its address,
   10.0.0.1, already goes through ACME (abcd.com)...", once per 10 min) with
   `others`, so the app offers "Use Nordbank" (MoveHosts). Adding it by hand
   is refused the same way. Routing goes by destination address, so both
   can't work at once. Later, maybe: give each such name its own stand-in
   address (fake-IP in the DNS answer, DNAT per tunnel), which would let
   both work.
6. UI: tray icon states, panel, notifications; then log, tunnel detail,
   settings, new tunnel, sign-in. **Built:** tray, panel, notifications,
   log, sign-in, Settings, New tunnel, tunnel detail.
   Tunnel detail (`tunnel_detail.rs`, 1:1 with tunnel-detail.html; opened
   by a tunnel's name or its row's chevron in Settings → Tunnels (the
   chevron was asked for: the rows didn't look clickable); `CRAKE_DEMO=1
   CRAKE_SHOW=detail:acme`): connected for (craked's `since`), address,
   protocol · transport (DTLS/ESP/TLS from openconnect's output, UDP/TCP
   for OpenVPN 3), gateway (profile for OpenVPN 3); who decides, in plain
   words for every mode (the design only shows the managed sentence);
   "Today through X" from per-tunnel byte counters (`Traffic::track`, two
   days per minute; OpenVPN 3's device now also counts as a tunnel, not
   direct) with a tick per host learned today; Learned / Pinned / From the
   gateway (its DNS servers and pushed networks) from D-Bus `HostDetails`
   (when and how each host was learned, `learned_how` in the host store;
   last used from the refreshed timeout); Pin/Unpin/Forget; "Forget all…"
   (requested 2026-10-14; design/proposals/forget-all.html) on the Learned
   tab empties what was learned and sent direct with Undo, asked in place
   first, pinned hosts kept (D-Bus `ForgetLearned`); "Add a host"
   takes an address, a network (pinned) or a name (asked of the tunnel's
   DNS, `add_name`). Refreshed every 5 s while open.
   Dismissing a stopped tunnel (requested 2026-10-15;
   design/proposals/dismiss-trouble.html, option A with the word, not an
   ×, which could read as removing the tunnel): a tunnel in trouble has a
   "Dismiss" next to its red words in the panel; D-Bus `DismissTrouble`
   makes it Off with nothing logged, its `why` kept for tunnel detail
   until it's switched on. The tray's badge sits on the head as it would
   be without it (`tray::Shape::{OffTrouble, ConnectingTrouble,
   Trouble}`): asleep when nothing is up (a tunnel that stopped in the
   night looked connected), dashed and moving while another connects,
   awake while one is up; and the routed head only while a tunnel is up.
   Why a tunnel stopped (design/proposals/trouble-reason.html, option A,
   chosen by the user): craked keeps the sentence it logged with the
   tunnel while it's in trouble (ListTunnels `why`); the panel's status
   names a reason Crake recognises in a few words ("Session expired",
   "Ended by the gateway", "Sign-in refused", "Ended by OpenVPN 3";
   `crake_proto::trouble`, from openvpn3's SESSION_EXPIRED and
   CONN_AUTH_FAILED, openconnect 9.12's own words, and a session's device
   gone), and tunnel detail shows the sentence under its title; anything
   else stays "Couldn't stay connected" (`CRAKE_DEMO=expired`).
   Traffic (`traffic_view/`, built 2026-10-14 from
   design/proposals/traffic-window.html, the graph first as a colleague
   asked; `CRAKE_DEMO=1 CRAKE_SHOW=traffic`): from the panel's footer
   ("Traffic", before Full log). This computer, the ways out (each tunnel
   that's up in try order, then direct) with their rates, the four busiest
   destinations of each ("and N more" past that), and packets along every
   path: speed and frequency from the path's rate (`graph::pace`: calm, as
   the colleague asked), a tunnel's new destination drawn in once; with
   animations off the dots stand still and thickness says the rate. Under
   it "Last 5 minutes" (a bar per 5 s, direct and tunnels stacked) and two
   tabs: "Right now" (open connections, busiest first: destination, way,
   rate, since; the rates shown follow the traffic over ~3 s, and the
   busiest places and the rows are ordered by a ~20 s average,
   `model::live` SHOWN_OVER/STANDING_OVER: the first window reshuffled
   every second, the user called it chaotic; the strip's volume in greys,
   tunnels `muted` under direct `bars`, with a teal tick where Crake routed
   and every way's box outlined `lineStrong`, critique 2026-10-15
   (design/proposals/critique-2026-10-15/): teal had meant tunnel volume;
   no dots below 1 KB/s,
   `DOTS_FROM`: a tunnel's keepalives drew dots at "0 KB/s"; an address
   keeps the name it was first seen with while fresh, `keep_name`: one
   stray wpad lookup had renamed GitLab) and "Where things go" (every tunnel's learned, pinned and
   undone hosts from HostDetails, the never list, then everything else).
   craked's D-Bus `Live` (asked once a second while the window is open,
   polkit-guarded like changes: it says where the computer connects):
   the physical and per-tunnel device counters (`Traffic::now`) and the
   open connections with their bytes (`conntrack::open_connections`):
   Linux from a conntrack dump, TCP and UDP (craked switches
   `nf_conntrack_acct` on while it's asked and off a minute after, or
   leaves it alone if it was on; AppArmor allows that one file); Windows
   from its TCP tables, each connection's extended statistics
   (Get/SetPerTcp(6)ConnectionEStats) switched on when first seen and off
   a minute after the last look; macOS from the kernel's lists
   (`net.inet.tcp.pcblist_n` and `net.inet.udp.pcblist_n`: every
   socket's `xsockstat_n` counts all along; UDP sockets connected to one
   address). Each with its way
   (`owner_of`, a full tunnel, else direct) and the name it was looked up
   by; never DNS questions, loopback or connections coming in. The app
   turns two looks into rates (`crake_proto::live::Live::rates_since`,
   `model::live`). Tested: unit, window tests, the wire test, a namespace
   test (a routed and a direct connection, their ways and bytes) and a
   journey step (a real connection through ACME seen in the window).
   A host's whole name (design/proposals/host-full-name.html, option A;
   `full_name.rs`), in tunnel detail's rows and the Traffic window's
   destinations: pointing at a name underlines it, after a moment a card
   shows it whole (only "Click to copy" when it fits: the box's drawn
   width against the text's), a click copies it and "Copied" shows after
   it for 3 s. GPUI draws no dotted underline (a thin solid one) and
   closes a card when the button goes down (so "Copied" is beside the
   name, not in the card). A name in a content-sized flex row lost its
   "…"; it takes the row's leftover room instead.
   Tunnels (Settings, first section; design/proposals/settings-tunnels.html):
   one list in the order traffic takes, drag to reorder (the try order moved
   here from Learning), full tunnels under "Everything else goes through:";
   Edit / Remove… (asks first; trash icon) for tunnels added in the app,
   "Set up by ACME IT" for managed, "From the config file" otherwise.
   Tunnels added in the app (`own.rs`, craked's `tunnels.json`; D-Bus
   AddTunnel/UpdateTunnel/RemoveTunnel, ListTunnels `source` and `own`):
   name → interface name, a free mark/table index, live, no restart.
   Routing modes: learn, ask (LogKind::Asked + "Send through X" in the
   panel and a notification; asked once per host per 10 min), list (the
   managed-scope machinery: names and networks), full; per-tunnel "forget
   after"; keep signed in (off: switching off logs off); weak ciphers
   (`--allow-insecure-crypto`), client version (`--version-string`).
   New tunnel (`new_tunnel.rs`, 1:1 with new-tunnel.html, the window fits
   each step; `CRAKE_SHOW=newtunnel[:step[:protocol]]` with CRAKE_DEMO): detection
   asks the gateway with openconnect as AnyConnect (`detect.rs`: groups
   too) and reads its certificate's expiry in a TLS handshake that checks
   nothing (`gateway_cert.rs`, rustls as GPUI already builds it, a small
   DER reader for "not after"): "Its certificate is valid until March
   2027." as in the design; within 30 days "expires on 4 November 2026";
   past it "Found a Cisco AnyConnect gateway, but its certificate expired
   on 3 October 2026." (e2e: against ocserv's), sign-in browser
   (`--external-browser=xdg-open`), password (kept in the keyring after a
   sign-in that worked, `keyring::Kind::Password`), certificate (smart card
   `pkcs11:`, keyring, or a file picked and imported into the keyring:
   7e done). "The gateway suggests" shows when editing a tunnel that has
   connected (pins its networks); "Only what I list" gets a names/networks
   list (not in the design). Removing a tunnel forgets its keyring items.
   Settings (`settings_view.rs`, 1:1 with settings.html, all six sections;
   `CRAKE_SHOW=settings:<general|learning|names|privacy|managed|about>`):
   craked's values live in `settings.json` next to the host store, over
   D-Bus `Settings`/`SetSetting`/`ClearLog` (+ `SettingsChanged`): probe
   wait, try order (drag), when two tunnels answer, forget after (or
   never), the never list (hosts, networks and names), ask the tunnels
   about unknown names (rescue + split-horizon), keep names private, log
   retention (or none on disk: memory only). The app's own in `app.json`:
   appearance (light/dark/system), notifications (none/quiet/with the
   synthesized "crex crex"), show the bird (tray off: a second start opens
   the running Crake, `single.rs`), start at login (`autostart.rs`, an
   entry in ~/.config/autostart for this binary with `--autostart`, which
   switches on the tunnels left on), hide host names in helpdesk copies.
   **Beta protocols (requested):** only protocols used against a real
   gateway are shown plain; every other one carries a "Beta" tag (New
   tunnel's list, the Tunnels rows) and New tunnel says what that means
   when one is chosen. The list is `new_tunnel::TRIED` (today: anyconnect,
   openvpn3). A protocol leaves Beta only after the user (or someone they
   trust) has signed in, connected and had hosts routed through a real
   gateway of that kind; the README's "Speaks" table follows it.
   **Built (requested; design/proposals/what-crake-did-help.html):** a "?"
   help card next to "What Crake did" in the panel (`model/kept.rs`: how
   long a host is kept comes from Learning's "Forget after", or never, and
   names tunnels that keep theirs differently; the example is the newest
   host sent through). Hover opens it, a click keeps it open (Settings' "?"
   too: a click used to close what the hover had just opened). Originally:
   the card in the panel, saying a host appears there once, when Crake first sends it
   through a tunnel, and is remembered after that (each use keeps it
   another week); tunnel detail shows when it was last used. The user
   expected every visit to show and didn't know it records only the first.
7. Managed profiles and policy enforcement; helpdesk export. **Built**,
   proven in the lifecycle test (scope) and a throwaway namespace (full
   tunnel; a mutation without the LAN exclusion fails it).
   Done: `policy.rs` reads `managed_dir` (default `/etc/crake/managed.d`)
   `*.toml` (display_name, organisation, gateway, protocol, group, user,
   sign_in = browser|password|certificate, certificate, dns_domains,
   extra_args, `learn = [names/networks]` or `full_tunnel = true`); IT's
   tunnels come first and shadow a user's one of the same name; a broken
   file is skipped. The scope holds everywhere: DNS learning, the
   public-NXDOMAIN rescue, probing, and the user's own add/pin/redo ("ACME
   IT only allows *.acme.internal and 10.40.0.0/16 through ACME."). Full
   tunnel: everything not marked by another tunnel goes through except the
   gateway, on-link networks, loopback, link-local and multicast; its DNS
   becomes resolved's default route (`~.`). IPv6 (Linux) goes through
   too when the gateway gave the tunnel an IPv6 address; without one it
   is refused rather than sent around the tunnel (TCP reset at once, so
   apps fall back to IPv4; the rest dropped), as AnyConnect does by
   default (netns-tested both ways). D-Bus ListTunnels has
   managed/policy/full_tunnel/managed_file; the panel shows the Managed tag
   and the policy line as in tray-panel.html (the demo data now matches).
   Helpdesk copies (panel, log) can leave host names out
   (`hide_hosts` in app.json, `model::scrub`); its switch arrives with the
   Settings screen, as does the "Managed by IT" settings section.
7b. **Networks that block UDP (requested). Built**, tested with a fake
   openconnect (lifecycle test). craked reads openconnect's output
   (`networks::udp_failed`: DTLS handshake timed out/failed, "using HTTPS
   instead", dead peer; ignored once DTLS/ESP has come up), remembers the
   network (default gateway address + MAC, `networks.json` next to the
   state file), SIGHUPs openconnect and starts it again with `--no-dtls` on
   the same session; next time on that network it starts with `--no-dtls`.
   Logged: "UDP is blocked here. ACME runs over TCP on this network.", then
   "ACME is back." Without a reusable session it only remembers. Tested on
   a real network that blocks UDP and it works; SSID isn't used. Original request: On
   some networks (e.g. a company's own office Wi-Fi) DTLS can't come up and
   openconnect is slow until it falls back; the user's NetworkManager
   profile for the office differs only by `disable_udp = yes` (`--no-dtls`). Crake must handle it with no settings: notice DTLS failing
   (run openconnect with `LC_ALL=C` so its output is parseable), remember the
   network (default gateway's address + MAC, SSID when known), start with
   `--no-dtls` there next time, and restart once without DTLS right away,
   keeping the session (SIGHUP disconnect doesn't log off; reuse the cookie).
   Log it: "ACME runs over TCP on this network." / "UDP is blocked here."
7d. **Gateways that only admit "AnyConnect" (requested). Built.** Sign-in
   (`signin.rs`, `turned_away`) retries once with `--useragent=AnyConnect`
   when an anyconnect gateway answers the very first request with 403/404
   (e.g. "Unexpected 404 result from server"), unless the config sets a user
   agent. The session carries `user_agent`, so craked connects the same way;
   verified on the user's real gateway (no `--useragent` in the config). The app
   remembers it per gateway in `~/.local/state/crake/app.json` and
   shows a notice under the tunnel until "Got it"
   (design/proposals/anyconnect-notice.html; `CRAKE_DEMO=anyconnect`).
   Original request: the user's gateway refuses openconnect's default user
   agent; their NetworkManager profile sets `useragent = AnyConnect`. Crake should infer it: when sign-in fails the way such
   gateways fail (rejected client, unexpected response), retry with the
   AnyConnect user agent, remember it per gateway, and say so openly, never
   behind the user's back: a notice like "Crake introduces itself to ACME as
   Cisco AnyConnect, because ACME's gateway only lets AnyConnect in." with a
   "Got it, don't show again" that is remembered per tunnel.
7e. **Certificates and their passwords, kept safely (requested). Built**
   in sign-in (keyring for both; the temporary 0600 copy in
   `$XDG_RUNTIME_DIR/crake`), and New tunnel's certificate step imports a
   file picked on disk into the keyring. The certificate's password goes
   to the GNOME keyring (Secret Service, via `oo7`), never Crake's files.
   The certificate itself can be kept in the keyring too, so the user can
   delete the file from disk: written for each sign-in to a 0600 file in
   `$XDG_RUNTIME_DIR` (tmpfs) and removed when sign-in ends.
7c. **Animations (requested). Done.** Tray: the frames in `tray.rs`
   (`transition`, `connecting_loop`), 70 ms apart, shown by `animate_tray`.
   Panel: opens rising 8 px (180 ms), new decision rows slide in (220 ms),
   a new tick grows in two beats (260 ms, second 90 ms later), switches
   slide on a critically damped spring (~150 ms). Motion state outlives a
   panel refit (`panel::Motion`), so nothing replays. Original request: The user wants Crake to feel alive, like the
   dorhawk.software website: an animated tray icon (e.g. the head drawing
   itself while connecting, the two call marks pulsing when a host is
   routed), and motion in the app (panel opening, rows arriving, switches
   sliding, the two-stroke tick growing in the chart). **No breathing or
   pulsing status dots** (the user: reads as an AI-slop tell), even though
   tray-panel.html has one. Keep "quiet by default": short,
   purposeful motion, and honour GNOME's reduce-motion setting
   (`org.gnome.desktop.interface enable-animations`).
7f. **Fortinet gateways (requested; a colleague's company uses FortiClient).**
   SSL VPN: openconnect already speaks it (`--protocol=fortinet`), so it
   needs testing rather than building: sign-in (password, OTP, SAML via
   `--external-browser`) and what a FortiGate passes to the hook. First check
   with `openconnect --protocol=fortinet --authenticate <gateway>`: a COOKIE
   means Crake can work. IPsec (FortiGate dial-up, IKEv2): openconnect can't;
   it needs a second backend, strongSwan driven over vici, with an XFRM
   interface per tunnel so the same nftables sets and fwmark routing apply;
   IKE sign-in (certificate, EAP/XAuth) instead of the cookie; the config
   payload (address, DNS, split networks) mapped to what the hook gives
   today. About a milestone of work. Crake doesn't run FortiClient's host
   check (antivirus etc.); if a gateway refuses clients without one, Crake
   won't fake a compliance report or FortiClient's vendor ID.
   **EMS (decided with the user, 2026-10-12; the colleague's reply).** A
   FortiGate managed by **FortiClient EMS** (Endpoint Management Server)
   admits only endpoints registered with EMS and running FortiClient
   (telemetry, posture, a ZTNA client certificate). openconnect/Crake is
   turned away: not a protocol gap, an admission control the gateway
   operator set. The user does **not** want to spoof a managed/compliant
   client (vendor ID, EMS serial, posture report) to get past it — that's
   circumventing the operator's control, not Crake's own auth, and it's why
   EMS-environment bugs can't go to the client's helpdesk (unsanctioned
   tool). See [[fortinet-ems-no-spoofing]].
   Two honest things instead:
   - **Say so.** When a fortinet sign-in fails the EMS/ZTNA way, Crake tells
     the user plainly: "This gateway needs FortiClient managed by EMS;
     Crake can't sign in to EMS-managed gateways," pointing at the plan
     below. **Not built yet:** the exact failure signatures need a real EMS
     FortiGate's `openconnect --protocol=fortinet --authenticate` output
     (like 7d's AnyConnect 404 and 7b's UDP-block strings were pinned to
     real output; capture in CLAUDE.local.md when the colleague can run it).
   - **FortiClient connects, Crake routes on top (planned milestone).**
     The user's idea, and it fits Crake's model: like adopting an
     externally-started OpenVPN 3 session (7g, `ovpn3::adopt`,
     `tunnel_of_device`), Crake would **adopt FortiClient's interface** —
     the user starts FortiClient and signs in (real, EMS-compliant), it
     brings up its link (Linux: a ppp/tun device) and routes; Crake takes
     that device's DNS/domains, strips the blanket default route, and
     routes only the hosts that need it. No faking: auth and compliance are
     all FortiClient's; Crake only decides what the user's own machine
     sends through. Beats the colleague's VM workaround (FortiClient in a
     Linux VM on a MacBook): FortiClient runs on the host, Crake routes
     there. To prove first: that FortiClient-Linux (or Win/macOS) is even
     admitted to that EMS gateway, and whether FortiClient re-asserts its
     routes or ZTNA watches for route changes. Per-OS, milestone-sized; the
     adopt-and-re-route plumbing exists (openvpn3), a FortiClient engine
     that finds and takes over its interface is new.
7g. **OpenVPN 3 (requested; the user has openvpn3 profiles on this PC).
   Built; used on a real session.** The app
   runs the session as the user, who owns the profile (`ovpn3.rs`, D-Bus
   net.openvpn.v3.*: LookupConfigName, NewTunnel, Ready / user-input queue
   → Crake's sign-in window, web sign-in → xdg-open, Connect, then the
   session's `status` until CONN_CONNECTED = 7: StatusChange comes from
   net.openvpn.v3.log, only after `LogForward(true)`, so a signal just
   wakes the driver and `status` is also read every second), then hands craked the device, DNS, domains
   and gateway (D-Bus `Attach`); craked routes per host through it like any
   tunnel (`tunnel_up`, shared with openconnect's hook). The session is
   watched: dropped or switched off → `Detach`. Protocol `openvpn3`, its
   "gateway" is the profile name; New tunnel lists the profiles. openvpn3
   still applies what the profile itself routes (no override to stop it).
   netcfg's properties (DNS, domains, routes) are closed to users, so
   craked takes the device's DNS and domains from what openvpn3 registered
   in resolved (`Resolved::link`) and re-registers them routing-only.
   A real server (CLAUDE.local.md) pushes its DNS (its tunnel gateway),
   two domains and a handful of routes, no redirect-gateway; openvpn3
   installs those routes and sets `~.` + default route on tun0, so with
   openvpn3 alone every public lookup goes through the company. Crake
   re-registers the domains routing-only. The device isn't `crake-<name>`
   (openvpn3 names it tun0, tun1, ...): craked maps devices to tunnels
   (`tunnel_of_device`) for DNS learning and the public-DNS watch
   (nft set `tunnel_devices`).
   Sign-in in the app counts as connecting (tray, panel) until the
   session is handed to craked (`Model::signing_in`).
   Sessions started elsewhere (openvpn3-indicator, which the user runs;
   `openvpn3 session-start`; Crake before a restart): switching the tunnel
   on takes over the profile's running session instead of failing
   ("A session with this configuration profile is already running"), and
   the app checks every 3 s for connected sessions of its tunnels'
   profiles and takes them over (`ovpn3::adopt`, `watch_openvpn3`);
   switching off in Crake then ends them. craked's Attach on a tunnel
   already up on that device is a no-op.
   openvpn3 doesn't always name a session's device: device_name "" for
   a moment after CONN_CONNECTED, or for good (an Azure VPN gateway's
   profile, 2026-10-14: device_name "", device_path "/", connected_to
   empty, while connected on tun1). The app sends the session's
   `backend_pid` with the attach, and craked takes the one tun device
   that process holds open (/proc/<pid>/fdinfo "iff:", root only;
   netns-tested); a handover the app can't make ends the session.
   openvpn3 rewrites its devices' DNS in resolved when one of its
   sessions reconnects (seen 2026-10-14: another session's reconnect set
   an attached tunnel back to its domains as search domains, "~." and
   the default route, so every name went to that company and Firefox's
   proxy auto-detect resolved `wpad` into its domain and hung). craked
   looks at every tunnel's link 5 times a second (`keep_dns_links`,
   `DNS_LINKS_EVERY`; every 2 s until 2026-10-15, when a leftover openvpn3
   process (Not now) reconnected every minute or two and Firefox, reacting
   to each network change, asked for wpad inside the 2 s;
   `Resolved::link_as_set`) and puts its routing-only setup back; the
   package test rewrites ACME's link with resolvectl and expects it back.
   A session that ends with no one to tell craked (seen 2026-10-14 with
   openvpn3 v27.1, while openvpn3-indicator ran: the session ended as the
   app quit; without the indicator a session outlived the app) left the
   tunnel "Connected ·
   its DNS doesn't answer" for good. craked now looks for each attached
   tunnel's device every 2 s (`check_devices`; netlink's link kind "tun",
   which, unlike /sys, follows craked's network namespace) and takes a
   tunnel whose device is gone down as stopped unexpectedly ("OpenVPN 3
   ended its session: tun0 is gone."; netns-tested). Missing at two looks
   in a row, not one (`MISSING` in the attached fields): switching off in
   the app ended the session before craked heard of it, and craked, looking
   in between, logged "stopped unexpectedly" (2026-10-16); the app now
   tells craked (Detach) before it ends the session.
   **0.8.0's regression (2026-10-15):** the packaged craked can't look
   into openvpn3's process (it runs as `_openvpn`; the service has no
   CAP_SYS_PTRACE, AppArmor allows no other process's fdinfo), so
   `tun_of_process` found nothing, craked took every attached session for
   reconnecting and pulled its routes a second after it came up. Now the
   device craked was handed (same name and index) settles it first; a
   device made again under the same name is followed by its name; only a
   device that's gone asks the process, and the app hands a session over
   again (`Request::Reattach`) when openvpn3 says it's connected after a
   reconnect, with the device openvpn3 names. netns-tested with a process
   craked can't see into. The no-device-name case (Azure's profile, 7g)
   needs the process too: it works with craked run by hand, not under the
   package (Not now).
   The handover keeps an attached tunnel's DNS servers and domains in its
   fields (`attach` stores what it settled on): the craked before it had
   reset the link in resolved on its way out, so the next one read no
   domains, registered the tunnel with none, and couldn't stop openvpn3
   making it every name's DNS again ("~." and the default route; Firefox's
   proxy auto-detect hung on wpad), seen after a downgrade 2026-10-15.
   Unless the session's process (`backend_pid`) still runs: openvpn3
   makes a session's device again when it reconnects it (seen 2026-10-15:
   the Wi-Fi dropped for a minute at night, a session's tun1 came back as tun0,
   craked looked in between, called it stopped, and it stayed "stopped"
   for three hours while openvpn3's session worked). Then craked logs
   "Globex is reconnecting.", goes Connecting, and routes through the
   session's device once the process holds one again, under any name; a
   device made again under the same name between two looks is told by its
   index (the kernel took the old one's routes with it). netns-tested.
   openvpn3-indicator acts on every session of the user, Crake's too: it
   calls Ready+Connect at CFG_OK and opens the web sign-in at
   SESS_AUTH_URL (22, the URL is the status message). So Ready may answer
   "Pending web authentication" (already connecting: Crake just waits),
   Connect may say it's already done, and Crake opens the sign-in URL from
   the status or the OPEN_URL input queue itself, once per URL.
   Importing a .ovpn file (requested; design/proposals/ovpn-import.html,
   option A chosen by the user over a link under the field, which "read as
   thrown there"): New tunnel's Profile label row has "Import .ovpn…"
   (with no profiles at all, the list is an "Import a .ovpn file…"
   button); the file's referenced files (ca, cert, key, tls-auth with its
   direction, tls-crypt, pkcs12 as base64, ...) are put inline
   (`ovpn_file.rs`, as `openvpn3 config-import` does), openvpn3's
   configuration service takes it (`ovpn3::import`, D-Bus Import(ssbb),
   persistent, the user's), under a name no profile has yet ("initech 2"),
   and it's the chosen profile; a missing file or openvpn3's refusal is
   said under the field. Window tests and a journey step (the picker is
   the platform's; tests import the file it would give). Original note: Next protocol after openconnect's. openvpn3 runs its sessions through
   its own D-Bus services (net.openvpn.v3.*), with profiles imported from
   .ovpn files; Crake would drive those, keep its own routing (no
   redirect-gateway, routes pushed as hints), and read the session's tun
   device and pushed DNS. To check: how much routing openvpn3 insists on
   doing itself, and whether it allows a custom tun name.
7h. **A tunnel's DNS that stops answering (requested). Built**
   (lifecycle tests; tried on the user's real OpenVPN 3 tunnel with its
   DNS blocked by an nft rule: detected in about a minute, a known name
   answered from memory via crake-keep, unknown names SERVFAIL, recovery
   said): every 30 s
   craked asks each connected tunnel's DNS for its first domain's SOA
   through the tunnel (3 s); two silences in a row mean it stopped. Then it
   logs "Initech's DNS (10.8.0.1) doesn't answer." once, reverts the
   tunnel's link in resolved (nobody waits on it) and moves the tunnel's
   domains to a dummy link `crake-keep` (169.254.53.54, `keep.rs`), where
   craked answers from the last good answers (remembered per name in the
   host store, `answers`, from every answer its DNS gave): an A query gets
   the remembered addresses (or, failing that, the hosts it routes under
   that name) that still answer through the tunnel (443/80),
   routed first, TTL 30, said once per name per outage ("Using the
   address gitlab.initech.internal had last time."); another type for a
   known name gets an empty NOERROR; anything else SERVFAIL. The panel and
   tunnel detail say "Connected · its DNS doesn't answer". When it answers
   again: "Initech's DNS answers again.", its link is registered again and
   crake-keep shrinks or goes.
   Original request: Found with an
   OpenVPN profile that set `dhcp-option DNS <address>` by hand; the DNS VM
   was restarted, got another address, and every name under the company's
   domain failed although the servers themselves hadn't moved. Crake can't find the new
   server (nothing tells it; trying addresses on the tunnel's network
   would be a scan: never). Two things it can do:
   - Say so: check each tunnel's DNS when it comes up and when its answers
     time out; log and show in the panel, e.g. "Initech's DNS (10.8.0.1)
     doesn't answer. Names under initech.internal won't resolve until it's
     fixed in the initech profile."
   - Keep known names working: remember the last good answer per name from
     each tunnel's DNS; while that DNS doesn't answer, answer a name with
     its remembered address, only if that address still answers through
     the tunnel (443/80 or the port last used), and log it: "Initech's DNS
     doesn't answer. Using the address gitlab.initech.internal had last time."
     Names never seen before can't be helped.
8. **Someday: other platforms** (see below). Not before Linux is solid.

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
- **Public hosts 0.7 learned stay learned (2026-10-15).** 0.8.0's guards
  (a network in trouble, a public address tried directly once more) stop
  new mistakes, but an upgrade doesn't forget what an older Crake learned
  by mistake: connectivity-check.ubuntu.com, learned through a company's
  tunnel during a hiccup, is used every few minutes, so it never expires.
  The user forgets it in tunnel detail. Could do: on upgrade, try each
  learned public address directly once and forget those that answer.
- **Windows: names another VPN client serves are still kept private**
  (2026-10-14). Linux and macOS leave a domain another program serves to
  it (`served_elsewhere`); Windows returns nothing yet. To do: read other
  clients' NRPT rules and adapters' DNS suffixes there.
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
- **All unsafe code behind one safe layer (asked for by the user,
  2026-10-14).** About 100 unsafe uses today (craked 68 in 13 files, the
  app 32 in 7), nearly all in the `sys/` modules (system calls, netlink,
  the Windows and macOS APIs), plus clearing WAYLAND_DISPLAY in the app's
  main. Plan: a crate of its own (e.g. `crake-os`) holding every unsafe
  call behind safe functions, each with its `// SAFETY:` reasoning and
  tests; `#![forbid(unsafe_code)]` in crake, craked, crake-proto and
  lint-literals (forbid can't be allowed again further down), and a lint
  gate check that `unsafe` appears nowhere else. Mechanical but sizeable.
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
  what a user has to click past (Windows code signing is undecided,
  see Other platforms).
- [ ] Antivirus and endpoint security: craked's service, Wintun, and the
  routes, NRPT rules and firewall rules it writes, not blocked or flagged.
- [ ] Another VPN client installed (AnyConnect, GlobalProtect, ...): its
  names still resolve with "Keep internal names private" on
  (`served_elsewhere` isn't built for Windows: Not now).
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
  installed and switched on; what a new user sees (no bird at all?), and
  how Crake should tell them.
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
  (crake 66%, craked 63%, crake-proto 98%, lint-literals 82% as of Oct 2026). The pre-commit
  hook runs it when Rust files are committed. Raise a floor as tests grow;
  never lower one to let a change through.

## Testing without installing anything

The user doesn't want development builds installed on their machine.

- `cargo test`: unit tests, plus lifecycle tests with a fake openconnect.
- `crates/crake/src/wire_tests.rs`: the app and craked together over real
  D-Bus (craked's manager on a private socket, as `--dev`; the app's own
  `client.rs` drives it and its updates feed the app's `Model`): add an
  OpenVPN 3 tunnel, attach, add a host, list hosts, change a setting, a
  refusal, detach, remove. A new D-Bus call gets a step there.
- Window tests (headless, GPUI's test platform: nothing on screen):
  `#[gpui_kit::test]` with `gpui_kit::test::TestWindowExt` (click, input,
  press, scroll), in `tests.rs` of every window: `new_tunnel/`, `panel/`,
  `settings_view/`, `tunnel_detail/`, `log_view/` and `signin_view/`
  (against a fake openconnect, given by `Spec::openconnect`; real I/O
  needs `cx.executor().allow_parking()`). An element is clickable from a
  test once it has `.test_support()` as its last call (a no-op in normal
  builds; the dev-dependency turns on gpui-kit's `test-support`). Nothing
  of this computer's in a test: `look_around` returns fixed profiles and
  engines, `Prefs { scratch: true }` writes nothing.
- **The journey suite: the regression suite. It stays green; every change
  (refactors above all) runs it, and every new entry point of the app gets
  a step in it.** `crates/crake/src/journey_tests.rs`, run by
  `scripts/e2e-test.sh` (and CI): the real app shell (`start(cx, Setup)`,
  what `main()` runs: model, request router, windows, D-Bus client) on
  GPUI's headless test platform, a real craked behind a real D-Bus socket,
  real openconnect against ocserv, real nftables. One day in the app: the
  panel's switch → sign-in (trust, password) → connected, tray included; a
  host probed → "What Crake did" → Undo/Redo down to nftables; Settings;
  tunnel detail's Add a host; the log's Copy for helpdesk; New tunnel →
  added → its sign-in cancelled; switching off; craked stopping and
  coming back under the app. Mutations it catches: Undo sending no hosts,
  logged decisions dropped. Two test-platform stand-ins, each said where
  it's done: frames are drawn by hand and a window's resize delivered as a
  window system would (`settle`), and New tunnel gets a full-HD height
  (its own fit-to-step runs on a next frame the test platform never
  sends). `CRAKE_JOURNEY_DEBUG=1` prints every click's target,
  `CRAKE_DEBUG=1` the app's own debug lines.
  **Everywhere Crake ships (requested, 2026-10-11): the journey, at least
  its basics, runs on every supported system after every change**, so an
  implementation change can't quietly break one of them: each Linux
  distribution (Ubuntu 22.04/24.04/26.04, Debian 12/13, Fedora 43–45,
  Arch, NixOS) with its own openconnect, resolved and libraries, Windows,
  and macOS when it comes. **Linux distributions: done.**
  `scripts/build-deb.sh` builds the journey's test binary on 22.04 with
  the packages (`target/deb/journey`); `scripts/deb-test.sh` runs it in
  every package test's container after the tunnel phase (`check.sh
  journey`: the service steps aside, the journey runs its own craked with
  the packaged hook and the system's openconnect, nft and ip). Passes on
  Ubuntu 22.04/24.04/26.04, Debian 12/13, Fedora 43/44/45 and Arch; NixOS in its VM test.
  Windows: done, `scripts/windows-vm-test.sh` runs `windows_journey.rs`
  (the same day: the panel's switch and sign-in, Undo/Redo, Settings,
  tunnel detail, the log, New tunnel, switching off, the service
  restarting) against the installed service, with `windows_tests.rs`
  (helpers shared through `journey_kit.rs` and `test_app.rs`). It found
  two bugs: a new Wintun adapter's address sat in duplicate-address
  detection, so the first probe through a fresh tunnel failed (addresses
  are now created Preferred, as WireGuard does), and a domain kept private
  for a tunnel that's off beat a tunnel that's up with the same domain
  (`dns_links::owner` now prefers the tunnel). Still to do: macOS
  (GitHub's runners; no gateway there).
- `scripts/netns-test.sh`: real nftables and policy routing inside a
  disposable container with no network (host filesystem read-only, with a
  `/run` of its own: until 2026-10-15 the container saw this computer's
  /run, so a test running as root reached its system bus, the user's real
  openvpn3 and systemd-resolved; the e2e containers too).
- `scripts/e2e-test.sh` (`crates/crake/src/e2e_tests.rs`, skipped unless
  `CRAKE_E2E=1`): a real gateway. ocserv in one container (`scripts/e2e/`:
  user alice, password hunter2, a self-signed certificate; ACME's network
  10.40.0.0/16 behind it: DNS 10.40.0.2, git.acme.internal 10.40.1.10,
  wiki.acme.internal 10.40.1.11), the test in another, run from the host's
  read-only filesystem with the host's openconnect, nft and ip. Neither has
  a network: their only link is a socket pair in a temporary folder
  (`relay.py`, which drops UDP 443 while the test's `block-udp` file
  exists). The app's sign-in (trust the certificate, the password), craked
  over D-Bus, probing, a name only the tunnel's DNS knows, switching off
  and stopping leaving nothing; and a network that blocks UDP (7b: noticed,
  over TCP on the same session, remembered). Keep both containers without a
  network: a gateway on a Docker network would be a service on this machine.
- CI (`.github/workflows/ci.yml`, on every push; a new push cancels the
  branch's run still going): the lint gate, `cargo test`, the coverage
  floors, Crake for Windows (checked, then built and tested on a Windows
  runner), the netns and e2e scripts, the NixOS VM test, and the packages:
  built once (`packages`), then one `package-test` job per system, all at
  once, the journey included. Caches (asked for: quick feedback): Rust's
  (rust-cache) for check and Windows; `~/crake-cache` for the packages
  (`CRAKE_CACHE`: build-deb.sh and build-arch.sh use a folder instead of
  their Docker volumes; a new key each week and Cargo.lock), about 35 s
  instead of 14 min for the .deb, 37 s instead of 10 for Arch; the Nix store
  (cache-nix-action), where crane keeps the crates.io dependencies a
  derivation of their own (packaging/nix/package.nix; the vendored GPUI
  backend stays whole in it).
  **Caches (asked by the user, 2026-10-12: runs took 35 min with
  "No cache found"):** GitHub keeps 10 GB of caches per repository and
  evicts the least used; Nix (3.3 GB), the packages (2.6 GB, a new one
  each week), check (2.5 GB), the Windows build (1.5 GB) and the rest,
  with copies per branch, didn't fit, so each run evicted what the next
  needed. Now caches are saved by `main`'s runs only (rust-cache
  `save-if`, actions/cache's restore/save split, cache-nix-action's
  `save`) and restored on every branch; CI builds carry line tables only
  (`CARGO_PROFILE_DEV_DEBUG`), not full debug info; and `check` runs the
  tests once, under coverage (the separate `cargo test` was the same
  suite again). A branch run is fast once `main` has run with the same
  Cargo.lock. Measured on main: packages 2.6 GB, the Windows build 1.4,
  check 1.3, macOS 1.1, network 1.1 and Nix 3.3 still came to ~10.8 GB
  and Nix's was evicted. **The namespace and e2e tests run in `check`
  (2026-10-12, asked for: faster CI):** they had a `network` job of their
  own, the slowest (~15 min, ~11 of them compiling: `check`'s coverage
  build is instrumented, in another folder, and a `-p crake` build unifies
  dependencies' features differently, so nothing was reused).
  `scripts/check-coverage.sh` now measures the cargo-llvm-cov "show-env"
  way (plain `cargo test --workspace`, instrumented, in
  target/llvm-cov-target, opt-level 0) and then runs the scripts it's given
  in that very build (`CRAKE_CARGO_SCOPE=--workspace`): they build nothing.
  Their binaries in containers write no coverage (`LLVM_PROFILE_FILE=
  /dev/null`). Locally the scripts work as before. **GitHub's 10 GB can't be raised** (asked 2026-10-12: same for
  public/OSS and private, no paid tier), so the Nix store (the 3.3 GB
  whale it kept evicting) moved to **Cachix** (free for open source,
  cache `crake`): the nixos job's `cachix/cachix-action` pushes every path
  it builds and substitutes from it next time; Cachix has no 10 GB cap, so
  GitHub's budget now holds only the five Rust caches (~9 GB). **Replaced
  caches are deleted (asked by the user, 2026-10-14):** after a Cargo.lock
  change the Rust caches were saved again under the new key while the old
  ones stayed, and together they pushed out the packages cache, so 0.7.0's
  .deb was built from scratch (17 min instead of 7). CI's `caches` job, on
  main after the jobs that save, runs `scripts/prune-caches.sh`: per kind
  (the key without its trailing hashes and dates) only the newest stays. Setup (done
  with the user): a Cachix account, a public cache named `crake`, an auth
  token as the repo secret `CACHIX_AUTH_TOKEN`. Cachix's free plan holds
  5 GB and then evicts the least recently used paths (a warning email at
  85%); it had reached ~3.5 GB by 2026-10-12 because every path a run built
  was pushed, Crake's own per-commit outputs included. Now `pushFilter`
  keeps those out: only the dependency derivations (crane's `crake-deps`,
  `crake-journey-deps`) go up, which is what the next run reuses. The NixOS VM-boot (~6-8
  min) is uncacheable, so nixos stays the slowest job even cached.
  When Cargo.lock changes, nixos takes ~50 min (measured on 0.2.0's
  run: crake-deps 17, crake 2½, crake-journey-deps 25, the journey 3,
  the VM 1). Not for a release's version bump (2026-10-14): the
  dependency builds never see Crake's version (packaging/nix/package.nix:
  the cleaned Cargo.tomls drop it, the dummy source gets a Cargo.lock at
  0.0.0, made at evaluation, since a derivation is hashed by its inputs),
  and `scripts/nix-deps-check.sh` (first step of the nixos job) fails if
  they would change with it. **Two layers (asked by the user,
  2026-10-14):** a dependency change no longer rebuilds them all. Each
  dependency build has a base, the dependencies of an earlier commit
  (flake input `crake-base`, pinned in flake.lock; `crake-base-deps`,
  `crake-journey-base-deps`), and the current ones built on top of it
  (crane's mkCargoDerivation with the base as `cargoArtifacts`: its
  buildDepsOnly always starts empty), so cargo recompiles only the crates
  that changed and what depends on them. Measured locally: the rustls
  update on a base from before it recompiled rustls and Crake's dummy
  crate, 12 s instead of ~6 min (both cargo passes), where the first
  attempt recompiled all 687 crates: cargo keeps a build only while its
  sources' path is the same ("PathToSourceChanged"), and crane's vendored
  sources are a store path named after the whole Cargo.lock, so every
  Crake build (`postConfigure` in package.nix) reads them through a link
  at `/build/vendor-cargo-deps`, only when the build folder is /build:
  Nix's sandbox. Without one, Nix 2.30 and later give every build a new
  folder (`/nix/var/nix/builds/nix-<pid>-<n>`), the link moved each time
  and CI recompiled everything, Crake's own build included (found on
  26bcc8f's run: 90 min); there the store path is kept, which at least
  reuses everything while Cargo.lock is unchanged. So CI's nixos job turns
  the sandbox on (`sandbox = true`; Ubuntu 24.04 needs
  `kernel.apparmor_restrict_unprivileged_userns=0` for the runner's
  user's namespaces). Moving the base forward (`scripts/nix-base.sh [COMMIT]`,
  default main's latest; commit flake.lock) costs one full build (~45 min)
  and is worth it when the layer on top has grown. This covers what a
  Cargo.lock trimmed to Linux's crates was meant to (not possible: Cargo
  resolves the lock for every platform at once, so a lock without
  Windows' crates fails `--locked`). Merging the two deps derivations was considered and left
  (2026-10-13): the journey's build turns on GPUI's test-support, so GPUI
  compiles twice either way; only small shared crates would be saved.
- Daily use, nothing installed: `sudo ./target/debug/craked --config dev/<yours>.toml --dev`
  (serves the app on `/run/user/<uid>/crake-dev.sock`), then `./target/debug/crake`.
  The app signs in as the user (`openconnect --authenticate`, questions in
  its sign-in window, certificate password/copy in the keyring) and hands
  craked only the session. `CRAKE_SHOW=signin:<tunnel>` opens sign-in at
  start; with `CRAKE_DEMO=1` and a fake `openconnect` first in `PATH` it can
  be checked against design/screens/sign-in.html without a gateway (demo
  mode never touches the keyring).
- Looking at a window without flashing the screen: `scripts/xwin.py
  <window id> out.png` reads the crake window's own pixels over X11
  (find the id with `xwininfo -root -tree | grep '("crake" "crake")'`).
- Filming the app (the brag video, the website's images; 2026-10-15): demo
  mode answers Undo and Dismiss itself (no craked), `CRAKE_DEMO=openvpn3`
  makes Nordbank an OpenVPN 3 tunnel (tunnel detail's "Edit…", a long
  profile name), `CRAKE_SHOW=profile:<tunnel>` opens its profile editor
  (a sample profile in demo mode), `CRAKE_APPEARANCE=dark|light` pins the
  theme. Rendered headless on Xvfb (`GPUI_X11_SCALE_FACTOR=2`), each window
  cut into sections with its rounded corners made transparent.
- Motion: `CRAKE_SHOW=panel` opens the panel at start; `CRAKE_DEMO=arrive`
  adds a decision and flips the last switch every 8 s; `CRAKE_MOTION=slow`
  runs every animation ten times slower, `=off` turns them off (as GNOME's
  "Animations" switch does). Screenshot-portal captures flash the user's
  screen: take one or two, never a burst; use headless Chrome for proposals.
- Real gateway from a terminal: `sudo ./target/debug/craked --config dev/<yours>.toml --connect <tunnel>`
  in a separate terminal. Signs in on the terminal, then takes commands
  (`add`, `pin`, `unpin`, `forget`, `list`). Ctrl+C removes everything; state
  lives in `/tmp/crake-dev`. `dev/` is git-ignored.

## Releases (decided with the user, 2026-10-13)

`CHANGELOG.md`, written by hand for the people using Crake, in its copy
voice (not generated from commits, which are written for developers):
**every user-facing change adds a line under "Unreleased" in the same
commit.** To release, `scripts/release.sh <version>` (asked for by the
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

Crake drives VPN engines it doesn't contain: openconnect (AnyConnect,
GlobalProtect, Pulse, Juniper, Fortinet, F5, Array) and OpenVPN 3. Rule:
**where the platform has a package manager, depend on its packages; where
it doesn't, bundle, and own the updates.**

- **Debian/Ubuntu (.deb), first:** never bundle openconnect or other system
  software (a bundled copy misses the distro's security fixes; Debian policy
  discourages it). Rust crates (GPUI etc.) are linked in as usual; system C
  libraries come from `${shlibs:Depends}`.
  ```
  Depends:    ${shlibs:Depends}, nftables, iproute2, systemd-resolved,
              polkitd, dbus, openconnect | openvpn3
  Recommends: gnome-shell-extension-appindicator, gnome-keyring
  Suggests:   openvpn3
  ```
  At least one engine is required (`openconnect | openvpn3`): an
  OpenVPN-3-only user pulls nothing extra, a user with neither gets
  openconnect. Not plain Recommends: `--no-install-recommends` would leave
  Crake with no engine. openvpn3 isn't in Ubuntu's archive (OpenVPN's own
  repository), so it's never pulled in, only suggested. The tray extension
  and keyring are Recommends (KDE needs neither).
- **First idea: a PPA on Launchpad** (superseded by our own apt repository,
  see the toolchain finding below), so Ubuntu users
  `sudo add-apt-repository ppa:…/crake && sudo apt install crake`.
  Launchpad builds from a *source* package, on its own builders, without
  network access: the Rust crates must be vendored into the source package
  (`cargo vendor`, `.cargo/config.toml` pointing at the vendor dir), and
  the build must use a Rust toolchain from Ubuntu's archive (versioned
  `rustc-1.xx`/`cargo-1.xx` packages; check the series we target has one
  new enough for edition 2024 and GPUI). debian/ with rules, control (the
  Depends above), changelog, copyright; signed with the user's GPG key,
  uploaded with `dput`. Test the built .deb in a throwaway container
  first, never on the user's machine.
- **Toolchain finding (built in containers):** GPUI needs Rust 1.95 in
  practice (`std::hint::cold_path`, `slice::as_array`), whatever the
  crates declare. Ubuntu's archive has rustc-1.93 at most in 26.04 and 1.91
  in 24.04/22.04; only 26.10 has 1.95+. So a Launchpad PPA could build
  Crake for 26.10 and newer only. **Decided with the user** (who runs
  26.04 LTS on purpose): build the .deb ourselves (GitHub Actions, current Rust via
  rustup) on the oldest Ubuntu supported (22.04), so one .deb runs on
  22.04, 24.04, 26.04, 26.10 and recent Debian, and publish it in our own
  signed apt repository (e.g. apt.dorhawk.software on Cloudflare) so
  `apt install crake` and updates work everywhere. Depends need
  alternatives for older releases: `systemd-resolved | systemd (<< 252)`,
  `polkitd | policykit-1`.
- **The .deb: built.** `scripts/build-deb.sh` builds it in an Ubuntu 22.04
  container with rustup's stable Rust (`packaging/deb/`: control with the
  Depends above plus `dpkg-shlibdeps`' libraries, maintainer scripts that
  enable and start `craked.service`, a desktop entry, a comment-only
  `/etc/crake/craked.toml` conffile; purge removes `/var/lib/crake`).
  Layout: `/usr/bin/crake`, `/usr/sbin/craked`, `/usr/libexec/crake/hook`,
  the unit, D-Bus policy and polkit action under `/usr/lib` and
  `/usr/share`. `scripts/deb-test.sh` installs it in throwaway containers
  that boot systemd (22.04, 24.04, 26.04; the container's own cgroups made
  writable, real `ip_unprivileged_port_start`): apt resolves the Depends,
  craked runs as the service, polkit refuses users without an active
  session, then, cut off from every network, a real tunnel through the
  installed service (ocserv in a namespace of its own): probing, DNS via
  systemd-resolved, names kept private by craked's DNS on port 53; remove
  and purge leave nothing. It found the unit lacking
  `CAP_NET_BIND_SERVICE` (craked's own DNS couldn't bind port 53), and that
  a systemd-resolved that doesn't answer kept tunnels from coming up (the
  hook timed out); resolved now gets 3 s per call (`resolved.rs`).
  **AppArmor:** `packaging/apparmor/usr.sbin.craked` confines craked (its
  files, netlink, its D-Bus name and calls to resolved and polkit, ip, nft,
  openconnect and the hook); the package loads it before craked starts and
  unloads it on removal. openconnect runs under craked's profile, not one
  of its own: the unit's NoNewPrivileges forbids the switch ("no new
  privs"). Containers can't test it (loading a profile there would load it
  into this machine's kernel), so `scripts/vm-test.sh [--complain]
  [release]` runs the same checks in a throwaway Ubuntu VM (qemu, KVM, no
  display, no ports; cloud image cached in `target/vm/`) with the profile
  enforced: craked and openconnect confined, nothing refused. Passes on
  22.04, 24.04 and 26.04. Not in CI (needs KVM). Still
  to do, with the user: the signed apt repository (their key) and its
  publishing.
- **Upgrades keep tunnels up (requested).** postinst, upgrading a running
  craked, leaves `/run/crake/handover` and restarts it; craked, stopping,
  then hangs up each openconnect tunnel without logging off (SIGHUP) and
  writes their sessions, and the OpenVPN 3 devices the app attached, to
  `/run/crake/resume.json` (0600, tmpfs); the new craked reads it once,
  deletes it and switches them on again ("Crake was updated. Your tunnels
  are reconnecting with your sign-ins."; no "Switched off/on" lines). A
  flag older than 2 min or a file older than 10 min is ignored. The unit
  needs `RuntimeDirectoryPreserve=restart` and `KillMode=mixed` (without
  it systemd SIGTERMs openconnect itself, which logs off). Tested: lifecycle
  (`an_upgrade_hands_the_tunnels_to_the_new_craked`) and the package test
  (`dpkg -i` over a connected ACME: back, signed in, learned hosts still
  routed), containers and AppArmor VMs.
- **Ubuntu 22.04 (decided with the user): supported, with password and
  certificate sign-in only.** Its openconnect is 8.20, without
  `--external-browser` (openconnect 9+). `engines.rs` asks openconnect's
  `--help` for it (and `--version` for the number); New tunnel tags the
  browser card "Not available", says why ("…needs openconnect 9 or newer;
  this computer has 8.20. Use a username and password or a certificate."),
  won't continue with it, and starts new tunnels on the password there.
  Sign-in turns openconnect 8's "unrecognized option '--external-browser'"
  into the same sentence. Checked against 22.04's real openconnect.
- **At runtime (built, `engines.rs`):** the app checks which engines exist
  (`engines::openconnect()`: `CRAKE_OPENCONNECT`, openconnect on PATH,
  or /usr/sbin and /sbin: Debian installs it in /usr/sbin, which its
  users' PATH leaves out, so the app called it missing there; found in a
  Debian 13 VM, the package test runs as root and missed it; openvpn3's D-Bus
  configuration service file). New tunnel shows every protocol; one whose
  engine is missing is tagged "Not installed", says how to get it ("Needs
  openconnect, which isn't installed: sudo apt install openconnect") and
  can't be continued with. Sign-in that can't start openconnect says
  "openconnect isn't installed" with the same command.
- **Fedora/RPM: built.** `scripts/build-rpm.sh` packs the files
  `build-deb.sh` staged (`target/deb/root`: the same binaries, built on
  22.04, run on Fedora's newer glibc) with `packaging/rpm/crake.spec`:
  `Requires: (openconnect or openvpn3)`, nftables, iproute,
  systemd-resolved, polkit, dbus; `Recommends:` as above; no AppArmor
  profile (Fedora confines services with SELinux; craked runs as an
  unconfined service there: `scripts/vm-test.sh fedora:44` runs the whole
  package test in a Fedora VM with SELinux enforcing, a real tunnel
  included, and SELinux refuses craked nothing). The test's gateway enters
  its namespace with `nsenter`: `ip netns exec` / `ip -n` remount /sys,
  which SELinux refuses to a test script there. %post enables craked, or on an upgrade leaves the handover flag and
  restarts it; removing takes /var/lib/crake (rpm has no purge).
  `scripts/deb-test.sh fedora:43 fedora:44 fedora:45` runs the same
  package test with dnf and rpm.
- **Arch: built.** `packaging/arch/PKGBUILD` (for the AUR; builds a
  release's source with Arch's Rust; `options=('!lto')`: makepkg's -flto
  makes C parts of crates GCC bitcode lld can't link) and `crake.install`.
  pacman has no alternatives: both engines are `optdepends`, and the app's
  engine check says which a tunnel needs. Arch layout: everything in
  /usr/bin, the hook in /usr/lib/crake (the shipped craked.toml sets
  `hook_path`). Arch enables no services on install: the install message
  says to point /etc/resolv.conf at resolved and `systemctl enable --now
  systemd-resolved craked`; post_upgrade hands tunnels over as elsewhere.
  `scripts/build-arch.sh` builds it from the current commit;
  `scripts/deb-test.sh archlinux:latest` tests it, its gateway in a second
  container wired by `relay.py` (ocserv is AUR-only). Publishing to the
  AUR needs the user's AUR account.
- **Not Flatpak or Snap:** craked needs root-level network control on the
  host (nftables, policy routing, resolved), which sandboxes don't allow.
- **NixOS: built.** `flake.nix`: `packages.x86_64-linux.default`
  (`packaging/nix/package.nix`, buildRustPackage from Cargo.lock; GPUI's
  run-time libraries added to the app's rpath), `nixosModules.default`
  (`services.crake.enable`: craked's service as packaging/craked.service
  with nftables, iproute2 and openconnect on its path, D-Bus, polkit,
  resolved, /etc/crake/craked.toml with the hook's store path; dhcpcd and
  NetworkManager told to leave `crake-*` alone: dhcpcd took crake-acme and
  the DNS craked had handed resolved for it was gone). `checks.x86_64-linux.vm`
  (`packaging/nix/test.nix`): two VMs on the test's own network, ocserv
  behind which ACME's network sits, and a client with the module: sign-in,
  connect over D-Bus, probing, DNS, names kept private.
  `scripts/nix-test.sh` runs it in the nixos/nix container (no Nix on this
  machine), CI with nix-quick-install-action. Upgrades keep tunnels signed
  in: the module reloads craked on `nixos-rebuild switch`
  (`reloadIfChanged`), and its reload leaves the handover flag and
  restarts it, as the packages' upgrade scripts do. The VM test also
  checks polkit's refusal, the upgrade (a specialisation with craked's
  unit changed, switched to with ACME up), and runs the journey (Nix
  builds its test binary: the flake's `packages.journey`). Locally the
  container makes its own /dev/kvm open to all (Nix's build users lose
  their groups; without it the VMs ran emulated).
- **macOS and Windows (someday):** no package manager to pull engines in,
  so the installer bundles them: openconnect (Windows: its DLL plus Wintun;
  macOS: inside the signed, notarized .app). Bundling makes Crake
  responsible for their security updates: a Crake release follows each
  openconnect security release. openconnect is LGPL-2.1: ship it as a
  separate library with its licence and a pointer to its source. OpenVPN 3
  there: its core library, or OpenVPN Connect when installed; to decide.
  A Homebrew formula could depend on Homebrew's openconnect instead.

## Other platforms: Windows first (decided with the user, 2026-10-11)

The user asked for macOS and Windows after Linux. Decided: **Windows
first** (testable here: a Windows VM under KVM next to a Linux gateway VM,
wired by a socket file, no network); **macOS after**, with no Mac (the
user has none): built and tested on GitHub's macOS runners only, real
gateway tests by someone with a Mac. **Signing:** the user has the paid
Apple Developer Program (ask for the details when a macOS build is signed
and notarized); no Windows code signing yet: development builds stay
unsigned, and a certificate (e.g. Azure Trusted Signing) is decided with
the user before a Windows release. Phases: (1) craked's OS-specific parts behind
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
opens for the user at the screen, which registers it. **Labels of their
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
