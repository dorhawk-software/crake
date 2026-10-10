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
- Nothing is pushed until the user asks; the code isn't public yet.
- Nothing customer-specific is ever committed: no real gateways, company
  names, hosts or addresses of the user's clients, in code, tests, demo
  data, designs or these notes. Use ACME, Nordbank, Globex, Initech,
  `*.example` / `*.internal` names and documentation addresses
  (192.0.2.x, 198.51.100.x, 203.0.113.x). What was learned on the real
  gateways lives in `CLAUDE.local.md` (git-ignored). `scripts/check-private.sh`
  (run by `scripts/lint.sh` and the pre-commit hook) refuses the names
  listed in `dev/private-terms` (git-ignored); install the hook with
  `ln -s ../../scripts/check-private.sh .git/hooks/pre-commit`.

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

Later improvement: race the direct and tunnel attempts while holding the first
SYN in NFQUEUE, so even the first connection succeeds.

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
  (see its `CRAKE-PATCH.md`). A GNOME Shell extension would be the most native
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
   off fails at once); removed on shutdown.
   Still open: DNS over TCP, DNS servers reached over IPv6, DNS-over-TLS upstreams (Crake
   can't see those failures), IPv6 addresses in answers.
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
   Still open: UDP, IPv6, ICMP-unreachable is only noticed after the wait.
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
   last used from the refreshed timeout); Pin/Unpin/Forget; "Add a host"
   takes an address, a network (pinned) or a name (asked of the tunnel's
   DNS, `add_name`). Refreshed every 5 s while open.
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
   too; no certificate expiry yet), sign-in browser
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
   **To add (requested):** a "?" help card next to "What Crake did" in the
   panel, saying a host appears there once, when Crake first sends it
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
   becomes resolved's default route (`~.`). D-Bus ListTunnels has
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
   "ACME is back." Without a reusable session it only remembers. Not yet
   tested on the real office Wi-Fi; SSID isn't used. Original request: On
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
   `$XDG_RUNTIME_DIR/crake`). Still to come: the import file picker, with
   the New tunnel screen. The certificate's password goes to the GNOME
   keyring (Secret Service, via `oo7`), never Crake's files. The certificate
   itself can be kept in the keyring too, so the user can delete the file
   from disk: written for each sign-in to a 0600 file in `$XDG_RUNTIME_DIR`
   (tmpfs) and removed when sign-in ends. A file picker to import one comes
   with the New tunnel screen.
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
   openvpn3-indicator acts on every session of the user, Crake's too: it
   calls Ready+Connect at CFG_OK and opens the web sign-in at
   SESS_AUTH_URL (22, the URL is the status message). So Ready may answer
   "Pending web authentication" (already connecting: Crake just waits),
   Connect may say it's already done, and Crake opens the sign-in URL from
   the status or the OPEN_URL input queue itself, once per URL.
   Not yet: importing a .ovpn file. Original note: Next protocol after openconnect's. openvpn3 runs its sessions through
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

## Lints are errors

Any lint stops the build; fix it and build again, never silence it.
- `Cargo.toml` `[workspace.lints]`: every rustc warning (unused imports
  included) and every clippy lint is `deny`.
- `scripts/lint.sh`: clippy (all targets), `cargo deny check` (`deny.toml`:
  known vulnerabilities anywhere, unmaintained crates Crake uses directly,
  licences compatible with GPL-3.0-or-later, crates.io only) and
  `cargo machete` (unused dependencies). Tools in `~/.cargo/bin`.
- `.claude/settings.json`: a PostToolUse hook runs `scripts/lint.sh` after
  every `cargo build` / `cargo test` from Claude Code and blocks on failure.

## Testing without installing anything

The user doesn't want development builds installed on their machine.

- `cargo test`: unit tests, plus lifecycle tests with a fake openconnect.
- `scripts/netns-test.sh`: real nftables and policy routing inside a
  disposable container with no network (host filesystem read-only).
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
- Motion: `CRAKE_SHOW=panel` opens the panel at start; `CRAKE_DEMO=arrive`
  adds a decision and flips the last switch every 8 s; `CRAKE_MOTION=slow`
  runs every animation ten times slower, `=off` turns them off (as GNOME's
  "Animations" switch does). Screenshot-portal captures flash the user's
  screen: take one or two, never a burst; use headless Chrome for proposals.
- Real gateway from a terminal: `sudo ./target/debug/craked --config dev/<yours>.toml --connect <tunnel>`
  in a separate terminal. Signs in on the terminal, then takes commands
  (`add`, `pin`, `unpin`, `forget`, `list`). Ctrl+C removes everything; state
  lives in `/tmp/crake-dev`. `dev/` is git-ignored.

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
- **Goal: a PPA on Launchpad** (the user's request), so Ubuntu users
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
  Crake for 26.10 and newer only. Proposed instead (to decide with the
  user): build the .deb ourselves (GitHub Actions, current Rust via
  rustup) on the oldest Ubuntu supported (22.04), so one .deb runs on
  22.04, 24.04, 26.04, 26.10 and recent Debian, and publish it in our own
  signed apt repository (e.g. apt.dorhawk.software on Cloudflare) so
  `apt install crake` and updates work everywhere. Depends need
  alternatives for older releases: `systemd-resolved | systemd (<< 252)`,
  `polkitd | policykit-1`.
- **At runtime (built, `engines.rs`):** the app checks which engines exist
  (openconnect on PATH, or `CRAKE_OPENCONNECT`; openvpn3's D-Bus
  configuration service file). New tunnel shows every protocol; one whose
  engine is missing is tagged "Not installed", says how to get it ("Needs
  openconnect, which isn't installed: sudo apt install openconnect") and
  can't be continued with. Sign-in that can't start openconnect says
  "openconnect isn't installed" with the same command.
- **Fedora/RPM:** the same with rich dependencies,
  `Requires: (openconnect or openvpn3)`, `Recommends:` as above.
- **Arch:** no alternatives in pacman: `optdepends` for both engines plus
  the runtime check.
- **Not Flatpak or Snap:** craked needs root-level network control on the
  host (nftables, policy routing, resolved), which sandboxes don't allow.
- **macOS and Windows (someday):** no package manager to pull engines in,
  so the installer bundles them: openconnect (Windows: its DLL plus Wintun;
  macOS: inside the signed, notarized .app). Bundling makes Crake
  responsible for their security updates: a Crake release follows each
  openconnect security release. openconnect is LGPL-2.1: ship it as a
  separate library with its licence and a pointer to its source. OpenVPN 3
  there: its core library, or OpenVPN Connect when installed; to decide.
  A Homebrew formula could depend on Homebrew's openconnect instead.

## Someday: other platforms

Wanted one day, not planned yet. Keep the split that makes it possible:
decisions (host store, expiry, policy, log, probing logic) stay portable;
everything that touches the OS stays behind small modules (`net`, `routing`,
DNS, tray) that can get a per-OS implementation.

- **Other Linux distributions:** mostly as is. Watch strict reverse-path
  filtering (`rp_filter=1`, e.g. Fedora/RHEL): replies from routed hosts
  arrive on the tunnel while the unmarked route points elsewhere, so they'd
  be dropped. Ubuntu uses loose mode (2). Fix when needed: restore the mark
  from conntrack on input plus `src_valid_mark`. Needs openconnect, nftables,
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
