# Crake: milestones in detail

<!-- Moved out of CLAUDE.md on 2026-10-20 (CLAUDE.md had grown past the
     150k characters an agent's context takes in). CLAUDE.md keeps the
     rules and a summary with a pointer here; this keeps the detail. Edit
     it as CLAUDE.md was edited: what was decided, built and found, dated. -->

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
   company's DNS gives a private one (CLAUDE.local.md). Since 2026-10-26
   the company's DNS may give another *public* address too (a company that
   numbers its inside from its own public range; the public address asked
   outsiders to sign in): taken when it answers through the tunnel and not
   directly within DIRECT_WAIT (as the first-SYN race), so a name that only
   resolves differently there (a CDN's) stays direct (`route_own`; the
   scenario `a_public_name_with_another_public_address_goes_through`,
   ACME's 192.0.2.40 in companies.sh). Only A queries; the policy scope, undo and never apply.
   "Keep internal names private" (Settings, on by default): the domains of
   tunnels that are off (as pushed when last up, plus configured) go to a
   dummy link `crake-hush` in resolved, whose DNS (169.254.53.53, craked)
   answers REFUSED at once (`hush.rs`; netns-tested, and verified with the
   user's systemd-resolved: looking up an internal name with its tunnel
   off fails at once); removed on shutdown. The link gets its domains only
   once craked holds 169.254.53.53:53 (2026-10-17: with a DNS server on
   every address, e.g. some dnsmasq setups, the bind failed, was logged
   once and never retried, and the domains went to that server); when it
   can't, the link goes, Settings → Names says why
   (`keep_private_trouble` in the Settings map, `hush::why_not`) and the
   2 s look tries again (netns-tested with a squatter on port 53). Not a domain another program
   serves (2026-10-14: a colleague's AnyConnect on a Mac failed every
   internal name while Crake refused them): `Resolved::served_elsewhere`
   (Linux: resolved's domains on links not Crake's; macOS: every network
   service's DNS in the system configuration, where VPN clients put
   theirs; Windows: the NRPT rules that aren't Crake's, local and group
   policy's, and the suffixes of the adapters that are up and aren't
   Crake's, built 2026-10-18), looked at again every 2 s (`refresh_hush`, rewritten only when
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
   **Short names** (requested 2026-10-30: https://expert/ worked with plain
   openconnect, whose vpnc-script makes the gateway's default domain a
   search domain, and not with Crake, which sets none). Not a search
   domain: that completes every short name with the company's (`wpad`
   included, the proxy auto-detection that hung a browser, 7g) and sends
   every word typed in an address bar to its DNS for good. Instead
   (`manager/short_names.rs`): a short name public DNS has no address for
   ("expert" is a top-level domain: "no such record", not NXDOMAIN) is
   asked of each tunnel that's up under each of its domains, once a day
   per name, and the log asks which tunnel's it means (`LogEntry::short`;
   "When two tunnels answer" applies). Chosen (`UseShortName`), craked
   answers the short name itself, a CNAME to the full name and the
   tunnel's records, routed first; Forget takes it back. Chosen and
   forgotten are System entries, so no chart or count takes them for
   routes. How the short name reaches craked: macOS, its own DNS server
   standing in for the system's (macOS sends "expert." as it is, seen on
   the user's Mac); Linux, the hush link's search domain `crake.internal`
   while a tunnel is up (resolved never sends a single label to unicast
   DNS), answered by the hush answerer; Windows, `crake.internal` as each
   tunnel adapter's connection-specific suffix and an NRPT rule (`Crake
   short`) sending it to craked's server. Windows, too, never sent "git"
   by itself to DNS (CI, 2026-10-30, the first try: craked's server
   standing in for every name never saw it). Scenario
   `a_short_name_means_the_tunnels_once_chosen` and a journey step on
   every system (Linux's journey container has no resolved: it asks
   craked as the hush link would; the scenarios use resolved).
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
   NATed afresh. A retry in the moment between the two (the host routed,
   the entry not yet deleted) kept its first packet's NAT (none) and left
   the tunnel with the LAN's address; a gateway that answers that address
   over the LAN completed the connection, and the next packet, NATed once
   the entry was gone, was reset. That was the NixOS journey's "Connection
   reset by peer" (2026-10-14, 18 and 21; reproduced in netns.rs). Since
   2026-10-21 nothing leaves a tunnel with an address other than its own
   (`leave_n`, after NAT): such a retry is dropped and retried again. Never probed: own LAN/bridges (on-link), loopback etc., the
   gateway, hosts a tunnel that's up routes (until 2026-10-20 any
   tunnel's, so a host learned by one that was off, or by an IT tunnel
   whose file was gone, could be learned by no other: found by the
   scenario suite on CI's Windows; a tunnel going down now also forgets
   its hosts' probes, so another tunnel tries them at once), the `never` list, hosts probed in the last
   10 min (forgotten whenever a tunnel comes up, again or new: what found
   no answer may answer through it now; and a probe whose tunnels were
   restarting or changed while it ran doesn't count; both found on CI's
   Windows through Cloudflare, 2026-10-18). Undo = send direct and keep direct (per tunnel) until redo; DNS
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
   Public hosts learned by mistake (2026-10-16; Windows and macOS
   2026-10-18; hourly since 2026-10-28): every hour, for each tunnel
   that's up, craked tries every public address it learned by probing
   directly (`manager/recheck.rs`), on the port it was learned on
   (`learned_on`; 443 then 80 for hosts learned before it was kept; a
   UDP host isn't tried), and forgets those that answer
   ("connectivity-check.ubuntu.com goes direct again."). Until
   2026-10-28 it ran once per version: on the user's Mac a public site
   learned on a lossy morning (the direct second try lost too, while
   github.com's took 2 s) stayed in Initech's tunnel for days. Hourly
   can't flip a host back and forth: probing learns only what doesn't
   answer directly, the recheck forgets only what does; the port keeps a
   public database a company reaches from its own addresses from being
   forgotten because its web server answers (netns-tested). The try carries
   `plan::DIRECT_MARK` (0x43520000): on Linux every tunnel's chain returns
   on it and no `ip rule` matches (netns-tested, fails if the try goes
   through the tunnel); on Windows and macOS, where a learned host is a
   route craked's own socket follows too, the socket is bound to the
   default route's interface (the best route to 0.0.0.0, IP_UNICAST_IF;
   `route -n get default`, IP_BOUND_IF; none while that's one of Crake's
   tunnels). Proven on CI's runners as administrator and root: a public
   address routed into loopback or a utun nobody reads, a plain
   connection gets nowhere, a direct one through (tests/windows_direct.rs,
   tests/macos.rs).
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
   **The badge, readable (2026-10-18; design/proposals/tray-badge.html,
   B, chosen by the user, who found the old one "extremely teeny tiny"):**
   at the bottom right over the breast, 14/32 of the icon, on each size's
   whole pixels with its "!" fitted to them (`art::Place`, `badge_at`,
   `bang`; at 16 px a 7 px circle, a 1 px stem, a gap, a dot), and on a
   light bar the light theme's red and amber (`tray::for_light_bar`,
   `light_bar_pixel`). The proposal's icons are the app's own
   (`write_badge_proposal` writes tray-badge/art.js).
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
   **The audit of 2026-10-17** (design/proposals/audit-2026-10-17.html,
   every window captured and read against DESIGN.md by two assessments;
   the user took it all): the graph's packets are grey on every way and
   the two-stroke tick marks each destination Crake decided on (pinned later or not), in the graph and
   the table's "Way" column (teal had come to mean "through a tunnel"
   there); Settings' Remove…/Clear log… are quiet until pointed at; the
   "?" only where its card says more; tunnel detail's buttons and
   columns as its screen; the log's empty day said once; sign-in asks
   "for your group" from a field-name table; the light amber #BF7E12;
   DESIGN.md's Colour section states the teal rule, the notice shape and
   the one title row. Headless captures of every window: the x11 test
   image plus xwd (see the memory note design-proposals-impeccable).
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
   **No tray (requested 2026-10-16; design/proposals/no-tray-after.html,
   after an impeccable critique of no-tray.html):** ksni runs with
   `assume_sni_available`, so a missing StatusNotifierWatcher is heard
   (`watcher_offline`/`watcher_online`, `tray_host::Event::Host`) instead
   of the tray failing to start. While there's none, the app asks GNOME
   every 3 s what would help (`sys::gnome_tray::look`: org.gnome.Shell.
   Extensions' GetExtensionInfo for Ubuntu's and upstream's ids,
   UserExtensionsEnabled, the extension's files on disk, whether
   enabled-extensions is writable; a newly installed one is put in
   enabled-extensions for the next login) and `no_tray::decide` picks the
   case: switch on, refused (extensions off, locked, broken: Open
   Extensions), next login, install (the command from /etc/os-release),
   no tray. Settings' bird row says it with the remedy under it; started by
   hand the panel opens near the top with a note ("Not now" until next
   start); at login (`--autostart`) one notification after 10 s, once per
   case (`dismissed`, "no-tray:<case>"). Nothing with the bird hidden.
   `CRAKE_DEMO=notray:<a|refused|b|c|d>`. Tried in a container with a
   session bus and no tray (the panel near the top, no window at login);
   not on a real Fedora yet (the checklist).
   **Beta protocols (requested):** only protocols used against a real
   gateway are shown plain; every other one carries a "Beta" tag (New
   tunnel's list, the Tunnels rows) and New tunnel says what that means
   when one is chosen. The list is `new_tunnel::TRIED` (today: anyconnect,
   openvpn, openvpn3; openvpn since 2026-10-19: the user connected
   0.11.0's own OpenVPN to their company's real gateway, browser sign-in,
   and reached its internal network). A protocol leaves Beta only after the user (or someone they
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
   What's new (2026-10-21; design/proposals/whats-new.html, chosen by the
   user; `whats_new/`): CHANGELOG.md is built in (`include_str!`; the Nix
   package's file set has it) and read into releases, kinds and changes
   (`changelog.rs`; early releases keep their own headings or none). The
   version seen is app.json's `seen_version`: no app.json at start is a
   fresh install (seen at once, nothing said), app.json without it an
   update from before Crake kept it ("Crake is now 0.15.0."). The panel's
   notice comes after Restart and "X is out", counts the changes since the
   version seen (Underneath's left out; a release with only those says
   nothing), and goes on What's new or Not now (seen); a downgrade keeps the
   highest. The window: three versions in full, "Show earlier versions",
   Escape, the arrow and page keys; About's first link opens it at this
   version. Tests: `whats_new::tests` (the rules, the window), panel and
   Settings tests, the journey's step 3b; `CRAKE_DEMO=whatsnew` (seen
   0.13.1) with `CRAKE_SHOW=whatsnew` or `panel`.
7. Managed profiles and policy enforcement; helpdesk export. **Built**,
   proven in the lifecycle test (scope) and a throwaway namespace (full
   tunnel; a mutation without the LAN exclusion fails it).
   Done: `policy.rs` reads `managed_dir` (default `/etc/crake/managed.d`)
   `*.toml` (display_name, organisation, gateway, protocol, group, user,
   sign_in = browser|password|certificate, certificate, dns_domains,
   extra_args, `learn = [names/networks]` or `full_tunnel = true`); IT's
   tunnels come first and shadow a user's one of the same name; a broken
   file is skipped. Read with or without a craked.toml
   (`Config::without_file`; until 2026-10-20 only with one, and Windows'
   and macOS' installers ship none, so IT's files did nothing there).
   The scope holds everywhere: DNS learning, the
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
   **Reports (requested 2026-10-18; design/proposals/report.html, A,
   plumbing kept; moved 2026-10-19 to Settings → Diagnostics,
   design/proposals/report-in-settings.html, A: the user, "a dump from
   someone ... i dont think the report is a standard feature which should
   be so visible", and beside Copy for helpdesk it read as a second answer
   to "what do I send?"):** Settings' Diagnostics section
   (`settings_view/diagnostics.rs`) has "Save a report for someone helping
   you": what's in the file, that passwords, sign-ins and keys never are,
   whether the places you connected to are (Privacy & log's switch), that
   it stays on the computer to be read first; then "Save a report…" (asks
   where; the button keeps its words while craked is asked, and a box
   under the row says "Saving the report…" with the dashed head,
   `head::under_way`, seconds after 2 s; then "Saved <file> in <folder>."
   with Open and Show in folder, ", names and addresses left out" when
   they were, or "Couldn't save the report." with the system's reason),
   and the terminal command for
   that system (crake isn't on the PATH on Windows or macOS). The log's
   Export is gone for good. The line goes when you move on
   (design/proposals/report-saved-line.html, A, chosen by the user
   2026-10-19): choosing another section clears a result
   (`Report::left`), never a save under way, which keeps its result for
   when you come back; Open, Show in folder, other apps and clicks keep
   it; a save that ends with Settings closed is said in a notification
   (`notify::said`, the Notifications setting read then). `crake report [file]
   [--hide-hosts|--show-hosts]` writes the same file without a window
   (`report.rs`; Windows attaches to the console it was run from). Always
   the same parts: craked's tunnels, "This computer" (D-Bus `Report`,
   authorized like Live: one.one.one.one looked up and connected to on
   443/80 with the way it took, kept from probing so a report never
   teaches a tunnel a public host; each tunnel's last-used named host, at
   its learned address when the name doesn't resolve), the system's DNS,
   routes and Crake's rules (`sys::<os>::report`: resolvectl status, ip
   route/rule, nft; scutil --dns, netstat -rn, pf's anchor; the NRPT and
   adapters' DNS, route print, the firewall group), lines whose
   destination is Crake's marked "← Crake: ACME's DNS" / "learned
   through ACME", the last 24 hours of decisions, and craked's own log
   (the last hour, at most 200 lines, kept in memory by a tracing layer,
   `own_log`). Never in it: lines with sign-in material, web addresses,
   file paths wherever they start (after "=", a quote, a tab; a Windows
   path's rest after a space), e-mail addresses, F5's and Pulse's
   cookies, OpenVPN's CRV1 challenge, hardware (MAC) addresses (a Mac's
   netstat -rn names the
   router's and other devices', Windows' route print each adapter's)
   (`crake_proto::report::private`, which every line of the file goes
   through, Crake's own decisions and tunnel lines too: a stopped tunnel's
   reason can carry openconnect's last words; a line naming sign-in
   material becomes "(a line left out: it named a sign-in)"). Hidden, `scrub_keeping`
   removes names and IPv4/IPv6 addresses but the computer's plumbing
   (craked's `keep`: its addresses and networks, the tunnels' addresses
   and DNS servers, the routers, 127.53.0.1) and what's never a host
   (loopback, link-local, multicast, netmasks), and the tunnels' names
   (`Keep::keeping_words`). Hidden as a whole after it's put together:
   names and addresses found anywhere in a line (with a port, after
   "host=", in lists and nft ranges, IPv4-mapped IPv6), a company domain
   ending in a digit (found 2026-10-19 on the user's real gateway, its
   host kept whole with hiding on), one-word domains where DNS settings
   list domains (`DOMAIN_KEYS`, "~routing"), a host's first label alone
   once its name is in the file (5 letters or more, not a tunnel's name);
   files and reverse-DNS identifiers aren't names. The Mac's netstat runs
   with -W and its short networks ("10.40/16") are written out. An
   adversarial audit (2026-10-19) found each of these; the tests in
   `model/scrub.rs` hold its lines. The setting is now "Hide
   names and addresses when sharing". Without craked the file says why
   and holds the rest. "Hide names and addresses when sharing" stays off
   by default (decided by the user, 2026-10-19): a report names the hosts
   reached unless it's switched on, and its row says so before saving.
   Tested: units, window
   tests (Settings', a stand-in craked, `report::Gatherer`), the wire test, e2e against ocserv, both journeys,
   the Mac and Windows gateway tests (each part on the real system), and
   the package test (resolvectl under craked's AppArmor profile, which now
   allows it; proven by vm-test.sh only).
7b. **Networks that block UDP (requested). Built**, tested with a fake
   openconnect (lifecycle test). craked reads openconnect's output
   (`networks::udp_failed`: DTLS handshake timed out/failed, "using HTTPS
   instead", dead peer; ignored once DTLS/ESP has come up), remembers the
   network (default gateway address + MAC, `networks.json` next to the
   state file), SIGHUPs openconnect and starts it again with `--no-dtls` on
   the same session; next time on that network it starts with `--no-dtls`.
   Logged: "UDP is blocked here. ACME runs over TCP on this network.", then
   "ACME is back." (or "ACME is up." when DTLS failed before the first
   openconnect was up: the usual order, on Linux too). While the first
   is being replaced the tunnel is Connecting, and a "connect" that one's
   script still sends is ignored (`Entry::replacing`; found by the scenario
   suite on CI's Windows, 2026-10-20: the app said connected for ~3 s
   while the device was being replaced, and names asked then failed).
   Without a reusable session it only remembers. Tested on
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
   crake-keep shrinks or goes. The order matters (found by the scenario
   suite, 2026-10-20): the answerer's socket is bound, then crake-keep
   takes the domains, and only then is the tunnel's link reverted (the
   other way round, a name asked in between went to public DNS, or to a
   link nobody answered on yet); and craked removes a crake-keep left by
   one that ended during an outage when it starts (`apply_keep` in
   `init`), or that company's names failed until the next outage.
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
7i. **OpenVPN with the `openvpn` program, on every system (decided with
   the user, 2026-10-18; in progress).** OpenVPN on Windows and macOS, and
   in place of OpenVPN 3 on Linux: craked runs the community `openvpn`
   (2.5 to 2.7) itself, one process per tunnel, as it runs openconnect, and
   drives it over its management interface. Why: OpenVPN 3 exists only on
   Linux, and most of 7g's trouble was openvpn3 running sessions itself
   (leftover processes rewriting other tunnels' DNS, its routes and DNS
   fought 5 times a second, tun0/tun1 renamed on reconnect, devices it
   doesn't name, a process the packaged craked can't look into). `openvpn`
   is in every distribution's archive (Ubuntu 22.04 2.5.11, 24.04 2.6.19,
   26.04 2.7.0, Debian 12/13 2.6.14, Fedora 43 2.6.23, 44/45 2.7.8, Arch
   2.7.8, looked up 2026-10-18), OpenVPN 3 only in OpenVPN's own.
   **Proven first, on the user's real gateways** (CLAUDE.local.md):
   Initech's browser sign-in works with plain `openvpn` 2.7 over the
   management interface (`setenv IV_SSO webauth,crtext`, then
   `>INFOMSG:WEB_AUTH::<url>`, the user signs in, `>PASSWORD:Auth-Token:`,
   CONNECTED; the server is openvpn-auth-oauth2-like, which refuses clients
   without webauth); an Azure VPN gateway's certificate profile fails with
   2.6+ ("Server did not request DATA_V2 packet format required for data
   channel offload ... Use --disable-dco", forever: why the user had to
   install OpenVPN 3, which leaves DCO off) and connects with
   `--disable-dco`.
   How it runs (`openvpn --config <craked's copy> --dev crake-<name>
   --dev-type tun --route-noexec --ifconfig-noexec --script-security 2
   --up <hook> --down <hook> --up-restart --management <socket> unix
   --management-hold --management-query-passwords --auth-retry interact
   --setenv IV_SSO webauth,crtext --pull-filter ignore block-outside-dns
   --ignore-unknown-option dns-updown --dns-updown disable`, LC_ALL=C):
   openvpn sets no address, route or DNS; the hook (`crake-hook`, which
   tells openvpn's call by `script_type`) hands craked the device, address,
   MTU, gateway (`trusted_ip`), pushed routes (`route_network_N`, hints as
   always), DNS and domains (`foreign_option_N` "dhcp-option DNS/DNS6/
   DOMAIN/DOMAIN-SEARCH", and 2.6's `dns_server_N_address_M`/
   `dns_search_domain_N`), and `script_context` init or restart (connect or
   reconnect); craked assigns the address and routes per host, as for
   openconnect (all seen in a lab run, 2026-10-18: a local server pushing
   routes, IPv6, DNS, domains, redirect-gateway and block-outside-dns).
   Sign-in: craked answers the management interface: a kept auth-token
   (`>PASSWORD:Auth-Token:`, Crake's equivalent of openconnect's cookie:
   switching on again, a restart without DCO and an upgrade's handover
   reuse it), else the profile's inline credentials, else it asks the app
   (D-Bus signal `SignInAsks`, answered with `Answer`): a password, a
   private key's password, a static or dynamic (CRV1) challenge, a code
   (`CR_TEXT`, answered with `cr-response`), or a page to open in the
   browser (`WEB_AUTH`/`OPEN_URL`). The sign-in window shows them as it
   shows openconnect's.
   The profile: imported in New tunnel (`ovpn_file::inline` puts its files
   inline), sent with AddTunnel, kept by craked as a root-only file
   (`profiles/<name>.ovpn` next to the host store, as NetworkManager and
   openvpn3 keep theirs), never in `tunnels.json` or ListTunnels. craked
   runs openvpn as root, so a profile is read against an allowlist of
   client options (`openvpn/profile.rs`): nothing that runs a program,
   loads a library, writes or reads a file outside the profile (scripts,
   plugins, `engine`, `pkcs11-providers`, `management`, `log`, `cd`, file
   paths in place of inline blocks) gets in; what Crake owns (`dev`,
   `persist-tun`, `user`/`group`) is dropped; anything unknown is refused
   by name, so a real profile tells us what to add.
   Slices: (1) craked's engine on Linux, tested in a namespace against a
   real `openvpn` server (pending web sign-in included), (2) the app (New
   tunnel's OpenVPN card, the sign-in window's questions, the journey), (3)
   packages (`openvpn` as an engine alternative), (4) Windows (openvpn and
   its drivers from OpenVPN's installer: 2.7 dropped Wintun for
   ovpn-dco-win, with tap-windows6 when DCO is off; management over
   loopback TCP), (5) macOS (Homebrew's openvpn, utun),
   (6) OpenVPN 3 tunnels moved to it (their profile's text copied out of
   openvpn3, unless locked) and the openvpn3 code removed.
   **Decided with the user (2026-10-18, after the critique of
   design/proposals/openvpn-tunnel.html):** OpenVPN 3 profiles move to the
   new engine now, not after it leaves Beta (slice 6 comes with the app's
   slice); and importing a .ovpn picks OpenVPN by itself ("Import .ovpn…"
   in the Gateway label's row fills in the protocol, the gateway and the
   name), so someone holding the file needn't know what a protocol is.
   **Slice 1: built (2026-10-18).** craked: `openvpn/` (profile, env,
   mgmt), `manager/openvpn/` (`run.rs` the process, `sign_in.rs` the
   management dialogue as a state machine: hold release, the kept token
   first, the profile's own credentials once, then the user; static and
   CRV1 challenges, CR_TEXT, a key's password, WEB_AUTH), protocol
   `openvpn` (AddTunnel's `profile` field; the gateway is the one it
   names), D-Bus `Asking`/`Answer` (polkit-guarded: a sign-in page is the
   user's) and ListTunnels `asking` (the question's id), the token in the
   upgrade handover (`resume.json`'s `openvpn`), `no_dco` per tunnel in the
   host store. Proven end to end (`e2e_openvpn.rs`, scripts/e2e: openvpn
   as a server beside ocserv with ACME's network behind it, `sso.py` as
   its browser sign-in, as the user's real gateway; a public host that
   drops what comes through a tunnel): the page asked for and opened,
   connected with what was pushed, the internet direct, a name only its
   DNS knows routed before the first packet, a host learned by probing,
   off and on again with the gateway's token and nothing asked, an
   upgrade's handover with the token (a mutation that hands no token over
   fails it). Not yet (later slices): the app's side; AppArmor's rules
   for openvpn and `profiles/`; smart cards
   (`pkcs11-providers`, refused today) and Windows' certificate store
   (`cryptoapicert`). The DCO fallback is proven end to end
   (`e2e_openvpn::a_gateway_without_offload_runs_without_it`, 2026-10-19):
   no open-source server refuses DCO, so a stand-in for openvpn says
   "Use --disable-dco" unless started with it and runs the real openvpn
   either way; craked notices, starts again without, connects to the real
   gateway, logs it once and starts without from then on (a mutation that
   ignores the line fails it). Not against a gateway that really refuses
   it: the user's Azure profile does (CLAUDE.local.md).
   **Slice 2 (the app) and 6 (the move): built (2026-10-18;
   design/proposals/openvpn-tunnel.html, chosen by the user).** New
   tunnel: "Import .ovpn…" beside Gateway picks OpenVPN by itself (name
   from the file, the profile's gateway shown, not typed, the list folded
   behind it); one "OpenVPN" card replaces "OpenVPN 3 profile", its
   Profile list holding OpenVPN 3's profiles to take over (a locked one
   says to import its file); a profile is read against the allowlist the
   moment it's chosen (`crake_proto::openvpn_profile`, moved from craked,
   with `Refusal` sentences: forbidden, unknown, a file it points to);
   step 2 "As the profile says". Sign-in: switched on, an OpenVPN tunnel
   just connects; craked's questions (`Update::Asking`) open the window
   (`openvpn.rs`, its registry a global); a page opens in the browser
   ("Finish in the browser tab Crake opened", "Waiting for you in
   Firefox": the desktop entry's name, `sys::browser_name`; "your
   browser" on Windows and macOS), "Open the page again"; asked again
   after a refusal, "Initech didn't accept that username or password."
   The move (`openvpn::move_from_openvpn3`): each OpenVPN 3 tunnel added
   in the app, once off and where openvpn is installed, gets its
   profile's copy (UpdateTunnel; craked logs "X now runs on Crake's own
   OpenVPN." and refuses while its session runs); a locked or refused
   one stays, with the panel's amber notice (`panel/staying.rs`,
   "Import .ovpn…" opens its Edit). Tested: window tests (New tunnel's
   import and take-over, the sign-in window's questions, the notice),
   lifecycle (the move), e2e through the app's client, the journey's
   import step. Tunnels from craked.toml or IT that are OpenVPN 3 stay so.
   **OpenVPN tunnels IT sets up, or craked.toml names (2026-10-18):** a
   managed file says `protocol = "openvpn"` and `profile = "initech.ovpn"`
   (beside it, or a whole path) in place of a gateway, which is read from
   the profile; the profile is checked against the allowlist when the file
   loads (one Crake can't run skips that tunnel, the reason in craked's
   log, as a broken file is) and read and checked again at each start
   from IT's file (`TunnelConfig::profile`; an app-added tunnel's is
   craked's copy in `profiles/`). The learning scope and the rest of the
   managed rules hold as for openconnect's; sign-in is the profile's
   (questions through `Asking`). e2e: `an_openvpn_tunnel_it_set_up`
   (IT's file and profile in managed.d, a password sign-in, a host inside
   the scope through, one outside refused in IT's words).
   **The sign-in kept (2026-10-18):** a username and password typed for
   an OpenVPN tunnel are kept in the keyring once it's up
   (`openvpn/kept.rs`, `keyring::Kind::Username` beside `Password`), and
   when craked asks again the app answers from there without opening the
   window; a refusal (craked's `retry`) forgets both and the window asks.
   Removing the tunnel forgets them. Demo mode keeps nothing; tests keep
   them in memory.
   **Slice 3 (packages): built (2026-10-18).** openvpn is a
   Recommended engine (.deb and RPM Recommends; not one of the
   required alternatives, `openconnect | openvpn3`: dnf met
   "(openconnect or openvpn or openvpn3)" with openvpn, and AnyConnect
   users were left without openconnect, CI 2026-10-18; Arch an optdepend; the
   NixOS module puts it on craked's path and the system's). AppArmor:
   openvpn runs under craked's profile (`ix`, as openconnect), and
   resolved's PropertiesChanged may reach craked. The package test has an
   OpenVPN phase (ACME's OpenVPN gateway beside ocserv, a password
   sign-in answered over D-Bus Asking/Answer, openvpn confined, a host
   learned through it, removed with its profile); `scripts/vm-test.sh
   26.04` passes it with the profile enforced. A release still needs CI's
   package tests on every system (22.04's openvpn is 2.5).
   **Slice 4 (Windows): built (2026-10-18).** `scripts/fetch-openvpn-windows.sh`
   takes openvpn.exe, tapctl.exe, their DLLs and the ovpn-dco driver from
   OpenVPN's MSI and tap-windows6 from its release (pinned by checksum,
   unmodified, a README with their licences); the installer puts them in
   `Crake\openvpn` and installs both drivers (pnputil). craked makes each
   run's adapter `crake-<name>` with tapctl (ovpn-dco, or tap-windows6
   without DCO) and removes it after, runs the hook as a copy of
   crake-hook.exe in its private run folder (openvpn there takes quotes
   as part of a path, and Program Files has a space), and locks its
   private folders with icacls (SYSTEM and Administrators). Management:
   2.7.8 on Windows ignores the management password file (it never asked
   for it; Linux's 2.7 does), so craked listens on a loopback port of its
   own, openvpn connects to it (`--management-client`), and craked takes
   only the connection whose process (Windows' TCP table) is that
   openvpn. And it took the second of two commands that came in one read
   (Windows' openvpn also hands the hook none of the pushed DNS: it
   keeps `dhcp-option`s for its adapter; craked takes the DNS servers and
   domains from openvpn's own output instead, its "PUSH: Received control
   message: 'PUSH_REPLY,...'" lines, continuations joined
   (`openvpn::env::Pushed`), where the hook has none; found by the
   real-gateway test on CI's Windows, 2026-10-18.)
   for the first (the password went in as the username again), so craked
   sends each command once the one before is answered (`Paced`, every
   platform). The Windows VM test has an OpenVPN tunnel (a password
   sign-in through the app's client, routed, off, no adapter left).
   craked's own folder, ProgramData\Crake, is locked the same way when
   the service starts (`create_private_dir`, `/T` for what an older Crake
   left there): ProgramData lets every user read, and its log, host store
   and settings had been readable by any user of the PC (found
   2026-10-18); the VM test checks no Users, Everyone or Authenticated
   Users entry is left on the folder, the log or the host store.
   **Slice 5 (macOS): built (2026-10-18).** Homebrew's openvpn (the cask
   depends on it; craked looks in /opt/homebrew/sbin and /usr/local/sbin),
   `--dev utun --dev-type tun` (openvpn takes tun or tap from a device's
   name, and without the type every tunnel stopped at the push: "problem
   with tun vs. tap setting", found by CI's Mac). `tests/macos_openvpn.rs`
   on CI's Mac as root: an openvpn server on loopback (its own `dev tun`:
   macOS' openvpn refuses `--mode server` with `dev utun`), a certificate
   sign-in, the utun the hook names with the tunnel's address and its DNS
   route, gone with the tunnel. Not the data path (a server on the same
   Mac answers its own addresses directly): 7j's.
7j. **Real gateways in CI, on every system (requested 2026-10-18; after
   7i).** Each CI system (Linux, macOS, Windows) starts Crake, connects to
   a real gateway of each kind there is software for, and checks that,
   connected, both a host only the tunnel reaches and the internet
   directly answer: so engine changes on macOS and Windows no longer wait
   for a colleague's Mac or a PC (the colleague then only checks what CI
   can't: installing, the approval click in System Settings, Crake.app's
   service under launchd, daily use).
   Gateways: AnyConnect (ocserv) and OpenVPN (`openvpn` as a server, with
   a pending web sign-in like the user's real one) carry traffic; for
   GlobalProtect, Fortinet, F5 and Array there's no open-source server, only
   openconnect's own test servers (its `tests/fake-*-server.py`), which
   prove the sign-in and nothing after; Pulse/Juniper has nothing.
   Linux: each gateway in a container of its own network, as the e2e and
   package tests already do (UDP included: DTLS, OpenVPN over UDP).
   macOS and Windows: GitHub's runners there have no Linux containers, and
   a gateway on the same machine shares its network (its "internal" hosts
   answer without the tunnel, proving nothing), so the gateways run in a
   Linux job and the Mac and Windows jobs reach them through **Cloudflare
   Tunnel** (decided with the user: free on the Zero Trust plan, the
   user's existing Cloudflare account; both ends connect out, a service
   token in the repository's secrets lets only CI in). Tailscale was
   considered: its free plan has 1,000 ephemeral minutes a month (~20 runs
   of three machines) and is for non-commercial use. Cloudflare Tunnel
   carries TCP only: AnyConnect over TLS and OpenVPN over TCP there; UDP is
   proven on Linux. On those runners craked runs by hand as root (as
   tests/macos.rs does), not as Crake.app's registered service or the
   installed Windows service unless the runner allows it.
   Setup with the user, once: a tunnel, a hostname and a service token in
   the Cloudflare dashboard.
   **macOS and Windows: built, waiting on that setup (2026-10-18).** CI's `gateways`
   job runs the e2e gateway's image with a network on the Linux runner
   (`scripts/ci-gateways.sh`, `scripts/e2e/ci-start.sh`: ocserv on
   127.0.0.1:8443, openvpn over TCP on 127.0.0.1:1194, ACME's network
   answering only through a tunnel), `cloudflared tunnel run` publishes
   them, an artifact (`gateways-ready`, the OpenVPN CA) says they're up,
   and it stays until the `macos` and `windows-gateways` jobs have
   finished. The Mac job's last test step and the Windows job
   (`scripts/ci-through-cloudflare.sh macos|windows`) carry each to 127.0.0.1
   with `cloudflared access tcp` and the service token and runs the
   scenario suite (`crates/crake/src/scenarios/`, since 2026-10-20; see
   "Testing without installing anything") and `gateway_tests/`: on the Mac as root, craked in the
   test as the e2e tests run it; on Windows against the installed service
   (`windows-installer` builds the installer from the run's executables,
   openconnect and OpenVPN cached by their scripts' hash; the job installs
   it silently, `/S`, and runs the app's test binary from `windows-build`): AnyConnect signed in with the app's sign-in, OpenVPN
   with its password asked through craked; connected, 10.40.1.10 answers
   once probed (through the tunnel's device), wiki.acme.internal is asked
   of macOS and routed before the first packet, 1.1.1.1 answers off the
   tunnel's device (OpenVPN's gateway pushes redirect-gateway); off, ACME's
   network is gone. cloudflared is pinned by version and checksum
   (`scripts/cloudflared.sh`). Without the secrets every step skips.
   In Cloudflare (Zero Trust): a tunnel whose token is the secret
   CLOUDFLARE_TUNNEL_TOKEN, with two public hostnames,
   crake-ci-anyconnect.dorhawk.software → tcp://localhost:8443 and
   crake-ci-openvpn.dorhawk.software → tcp://localhost:1194; a
   self-hosted Access application over both that admits only the service
   token `crake-ci` (Service Auth), its ID and secret the secrets
   CF_ACCESS_CLIENT_ID and CF_ACCESS_CLIENT_SECRET. **Set up by the user
   on 2026-10-18**, in the user's Cloudflare account (the dorhawk.software
   zone), all on the free Zero Trust plan:
   - Networking → Tunnels: `crake-ci` (cloudflared), with two routes
     (Published application): crake-ci-anyconnect.dorhawk.software →
     tcp://localhost:8443 and crake-ci-openvpn.dorhawk.software →
     tcp://localhost:1194; Cloudflare made a CNAME for each in the zone's
     DNS. "Inactive" except while a CI run's gateways job is connected.
   - Zero Trust → Access → Service credentials: the service token
     `crake-ci` (non-expiring).
   - Zero Trust → Access → Applications: `Crake CI gateways`
     (self-hosted, both hostnames), with the policy `crake-ci` (Service
     Auth, include Service Token crake-ci): nobody else gets in.
   - GitHub, dorhawk-software/crake → Settings → Secrets → Actions:
     CLOUDFLARE_TUNNEL_TOKEN, CF_ACCESS_CLIENT_ID, CF_ACCESS_CLIENT_SECRET.
   - **Added 2026-10-19 (the user):** five more routes on `crake-ci` and
     the same hostnames in `Crake CI gateways`: crake-ci-sso →
     tcp://localhost:8080 (OpenVPN's browser sign-in page, sso.py), and
     openconnect's fake servers crake-ci-fortinet/-gp/-f5/-pulse →
     tcp://localhost:4431–4434. The gateways job publishes those ports
     (`ci-gateways.sh`, `ci-start.sh` starts the fake servers);
     ci-through-cloudflare.sh carries them to the same local ports and
     sets CRAKE_GATEWAY_SSO / CRAKE_FAKE_GATEWAY only when they answer, so
     `gateway_tests::openvpn_browser_sign_in_through_cloudflare` and
     `e2e_protocols::each_protocols_sign_in` (now portable) skip without
     them. Taking it down: the same as below, these five routes and
     hostnames with the rest.
   To take it down: delete the application, the policy and the service
   token (Access), the tunnel (its routes go with it; check the two
   CNAMEs are gone from the zone's DNS), and the three secrets; CI then
   skips every real-gateway step by itself (no code change needed). To
   change the token (it leaked, or a yearly habit): Access → Service
   credentials → refresh `crake-ci`, put the new ID and secret in the two
   secrets; the tunnel's token: the tunnel's "Rotate token", then
   CLOUDFLARE_TUNNEL_TOKEN. One tunnel for every
   run: a push cancels the branch's run still going, so two runs' gateways
   meet only for a moment. **Running since 2026-10-18; both systems green
   on 7ae0d82 (run 39149518264):** the Mac passed from its first run;
   Windows ran none at first (the module gated to macOS by mistake), then
   found three bugs in Crake, all fixed: a probe falling into the
   UDP-blocked restart right after connecting (and one a reconnect
   followed at once) left the host direct for PROBED_FOR; "not probed
   again for 10 minutes" held across tunnels coming up (probing.rs, both
   now: a probe across a tunnel's change doesn't count, and a tunnel
   coming up forgets the probed hosts); and Windows' openvpn hands its
   hook no pushed DNS (`openvpn::env::Pushed`, from openvpn's output). The gateway tests also check the internet by name
   (one.one.one.one resolved as an app resolves it, while up and after
   switching off: "my internet stopped" is almost always names, craked
   standing in for the system's DNS), and `the_internet_survives_crakeds_end`:
   craked killed mid-connection (macOS: a craked process of its own,
   SIGKILL, started again as launchd would; Windows: the service's
   process, restarted by Windows), then within 45 s names work and macOS'
   DNS no longer points at 127.53.0.1. And (2026-10-19, the testing-gaps
   review; scenarios since 2026-10-20) a name public DNS doesn't know
   answered by ACME's (portal.acme.example, the rescue) and reached at
   once, a name asked over TCP, and with ACME off its names refused at
   once (kept private). Not through Cloudflare, so only in the local
   Windows VM test: a full tunnel (it would send cloudflared's own
   connection to Cloudflare into the tunnel it carries) and IPv6 (no
   IPv6 path).
   **The other protocols' sign-in (Linux e2e, 2026-10-18):**
   `e2e_protocols.rs` against openconnect's own fake servers for Fortinet,
   GlobalProtect, F5 and Juniper/Pulse (openconnect 9.12's tests/, LGPL,
   fetched into the e2e image pinned by checksum, one port each,
   4431-4434; Fortinet's, F5's and Juniper's cookie helper patched at
   build: their bytes cookie makes today's Werkzeug answer 500, and
   openconnect then asks for the password forever). Crake's sign-in asks
   what each asks and gets a session: Fortinet's code (tokeninfo),
   GlobalProtect's portal then gateway, F5's domain as a choice,
   Juniper's realm and token page (set through each server's /CONFIGURE,
   not the address); a sign-in that asks more than 8 questions fails
   instead of looping. Switched on with the session, craked's tunnel ends
   in trouble with a reason when the gateway refuses it at the tunnel
   stage (the fake servers carry no traffic). A regression guard for
   sign-in, not proof against a company's real gateway (forms, ports,
   SAML through ADFS, host checks, EMS): those protocols stay Beta until
   one is used. Array has no fake server. Through Cloudflare on the Mac
   and Windows too since 2026-10-19 (`e2e_protocols::each_protocols_sign_in`,
   portable).
   **A vendor's own appliance, FortiGate-VM (tried 2026-10-19, with the
   user's FortiCloud account; `scripts/fortigate-vm.sh`, uncommitted
   until it works):** the free "permanent trial" licence (one per
   account, no expiry, 1 CPU, 2 GB, three interfaces/policies/routes,
   **low encryption only**) is fetched from FortiCare inside the VM
   (`execute vm-license-options account-id|account-password`, then
   `execute vm-license`). FortiOS 7.4.12 (the only 7.4 a free account
   may download: older builds need a support contract) can't fetch it
   under qemu: its FortiGuard client fails before any connection
   ("ssl_ctx_create_new: Falied in setting cipher list", "Failed creating
   SSL struct"), with -cpu host, working DNS (qemu's 10.0.2.3, not
   FortiGuard's 96.45.45.45) and TCP to Fortinet's servers proven;
   `strong-crypto` doesn't exist unlicensed. And even licensed, a
   low-encryption unit can't run SSL-VPN (a Fortinet article, 2020), and
   7.6.3+ dropped the tunnel mode openconnect speaks: Crake over SSL-VPN
   against a real FortiGate needs a full evaluation licence from
   Fortinet's sales (not asked yet: the user, 2026-10-19). 8.0.1, tried
   for the FortiClient side only (7f's "adopt FortiClient's interface":
   the gateway's protocol doesn't matter there; its limits no longer say
   low encryption), gets past the cipher error but still can't fetch the
   licence: "Requesting FortiCare Trial license" then "Failed to download
   VM license", nothing more with `diagnose debug application
   forticldd|update -1`, though DNS works and TCP 443 to
   forticare.fortinet.com connects (ping never does on qemu's user
   network). The web page's licence dialog said why: FortiCare error 61,
   "Invalid serial number", for FGVMEV0000000000: qemu gives a VM an
   all-zero SMBIOS UUID unless told, and FortiCare refuses a unit
   without one. With `-uuid` (made once, kept in target/vm/fortigate/
   uuid: the licence is bound to the unit) **8.0.1 is licensed**
   (2026-10-19, the user's account; serial FGVMEV_ plus letters, "License
   Status: Valid"). Its trial says low encryption only, yet IPsec IKEv2
   offers every strong proposal (AES-CBC and GCM 128-256, ChaCha20-
   Poly1305, SHA-256 to 512): FortiClient can connect to it, and so could
   strongSwan. 7.4.12 booted as the same unit (`fortigate-vm.sh console
   7.4`: its own disk, the same UUID) took the same licence, so its
   cipher-list error had been the zero UUID too; but SSL-VPN never runs
   on the trial: `config vpn ssl settings` says enabled and accepts a
   tunnel portal, user, policy and port 10443, yet `diagnose sys process
   pidof sslvpnd` is empty and nothing listens (low encryption, as the
   2020 article says). So openconnect's Fortinet SSL-VPN against a real
   FortiGate still needs a full evaluation; the trial serves IPsec (8.0).
   FortiClient for Linux does IPsec only under EMS (Fortinet's admin
   guide), so 7f's "ride on FortiClient" can't be tried against it on
   Linux; on Windows it could (the free client does IKEv2 there).
   **strongSwan against it, proven by hand (2026-10-19)**, the first step
   of Crake's own IPsec engine (decided with the user: before FortiClient):
   `fortigate-vm.sh console` gives the FortiGate a second card, port2
   (192.0.2.1), whose only cable is a socket file to `fortigate-vm.sh
   client`, a throwaway Ubuntu 26.04 VM that installs strongSwan from the
   archive inside itself (`scripts/fortigate-client/`: acme.conf, run.sh)
   and reports CLIENT-RESULT. The FortiGate's side, pasted into its
   console: port2 static; a phase1-interface `acme` (type dynamic, IKEv2,
   peertype any, mode-cfg with 10.40.9.10-20, a pre-shared key, aes256-
   sha256, DH 19: 8.0 warns that 14 is insecure); a phase2 with the same;
   10.40.0.1/32 on the tunnel's interface with ping (the host only the
   tunnel reaches); and one firewall policy from `acme` (FortiOS answers
   a dial-up gateway that no policy uses with NO_PROPOSAL_CHOSEN, its log
   saying "ignoring IKEv2 request, no policy configured"). The client got
   10.40.9.10, an ESP SA (AES-256, no PFS: strongSwan offered no group
   for ESP and the FortiGate took it) for 10.40.0.0/16, and 10.40.0.1
   answered through it only. Then sign-in as FortiClient does it: a user
   (alice, group vpn; phase1 `eap enable`, `eap-identity send-request`,
   `authusrgrp vpn`), the gateway still proving itself with the key, the
   user with EAP-MSCHAPv2 (Ubuntu: EAP-Identity is in
   libcharon-extra-plugins, universe; MSCHAPv2 in
   libcharon-extauth-plugins); and a DNS server (`system dns-server` on
   the tunnel's interface, non-recursive, its `dns-database` zone
   acme.internal in the **public** view: in the default shadow view it
   answered NXDOMAIN) handed over as `ipv4-dns-server1`: the client got
   10.40.0.1 as DNS and wiki.acme.internal from it. The engine: 7k.
7k. **IPsec (IKEv2) with strongSwan (decided with the user, 2026-10-19:
   before 7f's FortiClient road).** Why: FortiOS 7.6.3+ and 8.0 have no
   SSL-VPN tunnel (openconnect's Fortinet support covers older units
   only), so a new FortiGate's remote access is IPsec IKEv2, as are
   many others' (strongSwan, Windows' and Apple's built-in clients).
   Proven by hand against a real FortiGate (7j). Plan (Linux first):
   - **craked runs its own charon** (strongswan-charon's
     /usr/lib/ipsec/charon with Crake's strongswan.conf through
     STRONGSWAN_CONF: its own vici socket, no system config, the
     system's strongswan service and the user's connections left alone),
     one charon for every IPsec tunnel, started with the first, as it
     runs openconnect and openvpn.
   - **Driven over vici** (`rsvici`, tokio): load-conn, initiate,
     terminate, the ike-updown/child-updown events for up, down and
     reconnect; the CP payload (virtual IP, DNS, and FortiGate's split
     networks and domains when it sends them) handed to `tunnel_up` as the
     hook's are.
   - **An XFRM interface per tunnel** (`crake-<name>`, `if_id` in and
     out), traffic selectors wide (`remote_ts = 0.0.0.0/0`, `::/0`), so
     nothing goes in until Crake's own routing sends it: the nftables
     sets and fwmark routing apply as they are; no policies or routes of
     charon's (`install_routes = no`, no virtual IP on another link).
   - **Sign-in:** the gateway's key (a group key IT hands out, as in a
     FortiClient profile) or its certificate; the user's password over
     EAP (MSCHAPv2; GTC, MD5 if a gateway asks), asked through D-Bus
     `Asking` as OpenVPN's are and kept in the keyring once it worked;
     the user's certificate later. Never FortiClient's EMS or posture
     (7f: no spoofing).
   - **Tests:** e2e with strongSwan as the gateway (a responder in the
     e2e image, EAP-MSCHAPv2, a pool, DNS, ACME's network behind it), the
     journey, the package test; the FortiGate (`fortigate-vm.sh`) stays
     a hand check, as the user's real gateways are (its licence is the
     user's).
   - **Packages:** strongswan-charon, strongswan-swanctl (its vici
     plugin), libstrongswan-standard-plugins (openssl's crypto: without
     it Ubuntu 24.04's charon refused to start, "unmet dependency:
     HASHER:HASH_SHA1", CI 2026-10-19) and the two EAP plugin packages,
     suggested; never strongswan-starter's running service (built as
     slice 3, below).
   - **Other systems, to find out once Linux works:** macOS (Homebrew's
     strongSwan uses PF_KEY policies, no tunnel device, so a host would
     be an IPsec policy rather than a route; or Apple's IKEv2, which
     needs a Network Extension) and Windows (its own IKEv2 client makes
     an adapter Crake could route through). CI there reaches the test
     gateways through Cloudflare Tunnel, which carries TCP only, and IKE
     is UDP (500/4500): a UDP-capable link (Tailscale's ephemeral nodes,
     Cloudflare's WARP private networks), IKE over TCP (RFC 8229) if
     those clients speak it, or a gateway on the runner made to look
     remote; to choose then.
   **Slice 1, craked's engine on Linux: built (2026-10-19).** `ipsec/`
   (`Secret`, kept as `profiles/<name>.ipsec`, root's: the gateway's key
   and IKE identity, AddTunnel's `gateway_key`/`gateway_id`, never in
   tunnels.json; `charon.rs`, craked's charon with STRONGSWAN_CONF:
   random ports, no routes or virtual IP of its own, an explicit plugin
   list without resolve, updown's `dns_handler`, its log on stderr;
   `vici.rs`, rsvici: load-conn with `auth = eap` (whatever method the
   gateway asks), load-shared, initiate with charon's log for a refusal
   (`ERROR_AUTHENTICATION_FAILURE`, "method failed") or a failure's most
   telling line, list-sas every 2 s, terminate; `env.rs`, updown's
   PLUTO_* (the virtual IP, PLUTO_DNS4_n/6_n, the narrowed selector as a
   split-network hint) as a TunnelInfo), `manager/ipsec.rs` (the user's
   name and password asked through `Asking`, again with `retry` after a
   refusal; an XFRM interface `crake-<name>`, MTU 1400, `if_id` from the
   tunnel's index; the hook named in updown's command line, since
   charon gives its script only PLUTO_*; a second "connect" while
   connected is the same tunnel). Protocol `ipsec`; Config `charon`
   (Debian's, Fedora's or Arch's path). The hook reads PLUTO_VERB
   (`crake_proto::strongswan_reason`). OwnTunnel `domains` (an IPsec
   gateway's domain never reaches Crake: strongSwan 6 passes no
   INTERNAL_DNS_DOMAIN on, "handling INTERNAL_DNS_DOMAIN attribute
   failed", and DNS learning only routes names under a tunnel's
   domains). Proven by `e2e_ipsec.rs` against strongSwan as the gateway
   in the e2e image (scripts/e2e/ipsec.conf: the key, alice's password
   over EAP-MSCHAPv2, a pool, DNS, ACME's network on its own XFRM
   interface): a refused password asked again, connected, the internet
   direct, a name only its DNS knows routed before the first packet, a
   host learned by probing, nothing left after (interface, SAs, charon).
   Ubuntu's strongswan-charon recommends strongswan-starter, which starts
   the system's charon at once (seen on the user's machine, 2026-10-19:
   switched off with `systemctl disable --now strongswan-starter`); the
   vici plugin is strongswan-swanctl's. Not yet: the package's
   `crake-charon` unit (charon refuses to start beside another, its pid
   file's path built in: a unit with `TemporaryFileSystem=/run` and
   `/run/crake` bound back in; until then craked runs charon itself and
   says plainly when another strongSwan runs).
   **Slice 2, the app: built (2026-10-19;
   design/proposals/ipsec-tunnel.html, chosen by the user).** New tunnel
   (`new_tunnel/ipsec.rs`): "IPsec (IKEv2)" last in the protocol list,
   Beta ("FortiClient's “IPsec VPN”, and other IKEv2 gateways";
   Fortinet's card now says "FortiClient's “SSL-VPN”"); the list folds
   behind the chosen protocol (for every protocol) and "Import .ovpn…"
   shows only with Detect; Detect that finds no SSL VPN points at IPsec;
   step 1 has "Gateway's key" (Show/Hide; editing, "Kept by Crake's
   service" with Change… and Keep the old key: the key never comes back
   from craked) and "Company domains" (optional) under the gateway;
   step 2 a "Username and password" card, the Username field, and
   "Gateway's identity" under Advanced; without strongSwan the card says
   the install command (`engines::strongswan_install`, from
   /etc/os-release), and on Windows and macOS it's "Not available".
   Switched on, craked signs in and asks (`signin::craked_asks`: OpenVPN
   and IPsec; until then the app had started openconnect's sign-in for
   anything not OpenVPN, "Unknown VPN protocol 'ipsec'", found by the
   journey): only the password, with "Remember the password in your
   keyring" (now on every password the window asks, openconnect's and
   OpenVPN's too: kept only when ticked; never on a one-time code,
   `signin::asks_password`), and
   answered from the keyring the next time. A sign-in craked runs ends
   at Up, not when its window goes (switched off before the window had
   gone, craked's "off" was drawn as connecting). A refused key is its
   own trouble ("The key doesn't match ACME's gateway...", label "Key
   doesn't match", `vici::Signed::KeyRefused` from charon's "MAC
   mismatched"). Tested: window tests (`an_ipsec_gateway_with_its_key`,
   the remember toggle), e2e (`a_wrong_key_then_the_right_one`: the
   username from the tunnel, the key changed with UpdateTunnel), and the
   IPsec journey (`journey_ipsec.rs`, run by scripts/e2e-test.sh with
   CRAKE_E2E_IPSEC=1, not by the package tests, which have no IPsec
   gateway: New tunnel → the password asked → connected → off and on
   with the kept password and no window → off, nothing left). It also
   found the main journey's New tunnel step testing nothing: the app
   switches an added tunnel on after a timer, and GPUI's test platform
   runs timers only when its clock is moved, so Globex was never
   switched on and "its sign-in opens" found ACME's window, still open
   (its close is a timer too); the journeys now move the clock and wait
   for that tunnel's own window (`journey_kit::sign_in_of`, `decline`).
   **Slice 3, packages: built (2026-10-19).** charon runs as
   `crake-charon.service` (packaging/), which craked starts over systemd's
   D-Bus (RestartUnit, GetUnit, the unit's ActiveState, StopUnit;
   `ipsec/unit.rs`) when craked.service names it
   (`CRAKE_CHARON_UNIT`, Config `charon_unit`); run by hand or in the
   tests craked runs charon itself, as before. Why a unit: charon's pid
   file's path is built in (/var/run/charon.pid), so beside a system
   strongSwan it won't start, and craked's own /run is read-only
   (ProtectSystem=strict); the unit has a /run of its own
   (`TemporaryFileSystem=/run`, `/run/crake` bound back in), binds to
   craked (stops with it) and asks for Crake's own named AppArmor profile
   (`AppArmorProfile=-crake-charon`, packaging/apparmor/crake-charon):
   Ubuntu's profile for charon's path allows /etc/strongswan* and
   /run/charon.* only, and refused Crake's settings. craked's profile may
   make those calls (Properties `Get` only: zbus's caching proxy asked
   GetAll and followed PropertiesChanged, both refused, and craked took
   charon for stopped; and RestartUnit answers once its job is queued,
   so craked waits for the unit to leave "inactive" first: on CI's slower
   VM it had asked in between and stopped charon). strongSwan is
   **suggested**, not recommended as
   the plan said: Debian's strongswan-charon recommends
   strongswan-starter, which would start a system strongSwan on every
   install; New tunnel says the command, with --no-install-recommends.
   Fedora recommends `strongswan` (its service stays off), Arch an
   optdepend. The package test has an IPsec phase (strongSwan as ACME's
   gateway in its namespace, its charon a copy at another path holding
   the container's /run/charon.pid as a system strongSwan would, its
   plugins named: without a list it loaded kernel-libipsec, which took
   the gateway's traffic); `scripts/vm-test.sh 26.04` passes it with both
   profiles enforced. Locally the containers fail it: the kernel attaches
   this computer's Ubuntu charon (and swanctl) profiles by path in
   containers too. The IPsec journey (`journey_ipsec.rs`) runs in every
   distribution's package test too (check.sh `journey`; its craked runs
   charon itself, at the distribution's path, `Config::default`), said
   skipped where SELinux enforces or the distribution's AppArmor profile
   for charon is loaded (the VMs: it confines a charon craked runs
   itself, as with craked by hand; the IPsec phase runs the unit). The
   gateway's charon has a /run of its own (`unshare --mount`), and the
   IPsec phase stands in for a system strongSwan with a pid file and a
   live process in /run and /run/strongswan (Fedora's charon keeps its
   pid file there; the unit binds craked's /run/crake/ipsec/run there:
   an ExecStartPre's mkdir made it in a /run of its own, and Ubuntu
   26.04's mkdir is a path AppArmor's profile doesn't name).
   **SELinux (Fedora): built (2026-10-19).** SELinux runs charon as
   strongSwan's own `ipsec_t` whatever starts it (unconfined_service_t
   may not use its file as an entry point, and through /bin/sh it moves
   to ipsec_t at charon's exec), and its updown as `ipsec_mgmt_t`; the
   RPM's policy module (packaging/selinux/crake.te, compiled by
   build-rpm.sh with checkpolicy, installed with semodule in %post,
   removed in %postun) gives craked's `/run/crake/ipsec` a type of its
   own (`crake_ipsec_run_t`, by a named type transition when craked
   makes "ipsec" there), lets ipsec_t use it, and lets ipsec_mgmt_t (the
   hook) reach craked's socket. `vm-test.sh fedora:44` passes the IPsec
   phase enforced with no denials. The IPsec journey still skips under
   SELinux: its craked runs charon by hand from a temporary folder.
   NixOS (2026-10-20): `services.crake.ipsec = true` gives the module
   the charon unit and strongSwan in the system's profile, where the app
   looks too; its VM test connects an IPsec tunnel
   (docs/engineering/packaging.md).
   **IT's files and craked.toml (2026-10-20):** `protocol = "ipsec"`
   with `gateway`, `gateway_id` and `gateway_key_file` (one line, beside
   IT's file or a whole path; root's alone: a key others can read is
   refused, `ipsec::key_from_file`, and the tunnel skipped with the reason
   in craked's log), `dns_domains`, `user` and the usual `learn`; sign-in
   is the user's password (`sign_in` other than password, `group`,
   `certificate` and `extra_args` refused for IPsec). The key is read at
   each start (`TunnelConfig::gateway_key_file`; an app-added tunnel's is
   craked's copy in `profiles/`). Tested: policy and config units, the
   `it_ipsec` scenarios (scope held, can't be changed or removed).
   **Across an upgrade (2026-10-20):** charon's SA ends with craked and
   the password is the app's, so the old craked names the IPsec tunnels
   that were up in resume.json (`ipsec`), and the new one switches them on
   and asks; the app answers from the keyring when it keeps the password
   (the upgrade scenario for IPsec). Keeping charon's SA itself across the
   restart was weighed and left: its unit stops with craked.
   **"Change the key…" (2026-10-20):** a key the gateway turns down ends
   the sign-in window's failure with Change the key… in Try again's
   place, opening the tunnel's Edit (as ipsec-tunnel.html drew it; window
   test `a_refused_key_offers_to_change_it`).
   A gateway with several networks: charon runs updown once per pair of
   selectors, and each later call's network is added to the tunnel's
   hints (`add_hints`; until 2026-10-20 only the first call's showed;
   `nothing_is_routed_up_front` checks both of the e2e gateway's).
   Certificates (the gateway's and the user's; proposed
   2026-10-21 in design/proposals/ipsec-certificates.html, A and A
   chosen by the user; the spike, scripts/spikes/ipsec-certificate, proved
   the same day that an untrusted gateway's certificate can be read out of
   charon's "enc 3" dump byte for byte, fragments included, and that a
   pinned copy is trusted alone and strictly; a gateway sends it only when
   asked, so the system's authorities are always loaded; a wrong
   `remote.id` gets none, so the first attempt names no gateway; the dump
   holds the user's own EAP messages and `reload-settings` switches it
   off; vici's initiate stream at level 3 leaves the dumps out, so it's a
   root-only file log: details in the proposal's section 5). Built
   2026-10-21: a gateway with a certificate (craked's secret has no key;
   the system's authorities, loaded over vici when charon starts, or IT's
   alone; checked for the gateway's name), the look at an untrusted one
   (`ipsec::certificate`, `manager/ipsec_trust.rs`: name first, then its
   dates, then the trust question, `AskKind::Trust`; trusted, it's pinned
   in `profiles/<name>.ipsec-pin` and loaded before each connection), a
   refusal for a wrong name told apart from a wrong password by a look
   (the gateway refuses that name before it shows its certificate); e2e:
   `a_gateway_with_a_certificate_from_its_authority`,
   `a_certificate_no_one_vouches_for_is_asked_about` (the e2e gateway's
   acme-cert on 198.51.100.11; its strongswan.conf raises the half-open
   limits, since every look leaves one). Not yet: AnyConnect's question
   getting the same "For …, signed by …" line and the changed-certificate
   variant (the proposal's 3): openconnect gives only the fingerprint, so
   the app would read the certificate itself (gateway_cert.rs, with the
   certificate reader moved to crake-proto), and a pinned certificate that
   changed is a failure there (`--servercert`), not a question. Built the
   same day: New tunnel's step 1 (two cards, IT's authority in the key's
   slot; `new_tunnel/ipsec_proof.rs`) and the user's own certificate
   (step 2's three cards; the app keeps the file in the keyring and
   answers craked's `certificate` question from it without a window;
   craked opens it, `ipsec::credential`, a .p12 with p12-keystore,
   legacy RC2 and 3DES too, or PEM with an unencrypted key; its password
   asked as "Unlock your certificate", kept when ticked; its dates read
   first; charon gets it as `local` (pubkey), or `local-1` and `local-2`
   with a password, RFC 4739; a refusal before any EAP is the
   certificate's). e2e: `the_users_own_certificate` (the gateway's
   acme-certs and acme-certpw, alice's .p12 in /link); journeys for both.
   Not yet, written down here: a smart card for IPsec (strongSwan's
   PKCS#11 plugin, the PIN over vici's load-token; needs the SoftHSM test
   image the AnyConnect smart-card test waits for too: step 2 offers only
   a file, unlike the proposal's "Certificate from"), EAP-TLS for
   gateways that take the certificate over EAP (the proposal's "IKE first,
   then EAP-TLS"), a PEM file whose key has a password (said: give it as
   a .p12), IT's files naming a certificate or IT's authority
   (`gateway_ca_file`), the sign-in window's rows for the gateway and the
   certificate (its steps are still a fixed three), Edit's "You trusted
   its certificate on … Stop trusting it", the FortiGate by
   hand.
   **The Doctor (2026-10-18, asked for by the user: investigate a Mac or
   Windows problem without a colleague's machine):** `.github/workflows/
   doctor.yml`, started by hand (system, tunnel anyconnect/openvpn/none,
   minutes, shell). The CI gateways come up as in CI; on the chosen
   runner `gateway_tests::doctor` holds the tunnel up (phases written to
   CRAKE_DOCTOR_PHASE) while `scripts/doctor-snapshot.sh` looks before,
   connected, off and after craked: DNS (scutil / NRPT), routes,
   pf/firewall rules, a public name looked up and fetched, ACME's name,
   craked's logs; the artifact `doctor-<system>` holds it all, and the
   run's own log (run.log). With shell, tmate (mxschmitt/action-tmate,
   pinned by commit, `limit-access-to-actor`: the starter's GitHub SSH
   keys only; detached, so the snapshots go on). The Mac builds the
   branch; Windows takes main's latest green CI installer and tests. CI's
   and the Doctor's gateways jobs share one concurrency group (the one
   tunnel), so a run's gateways may queue behind another's: the Mac and
   Windows jobs follow their gateways job's state rather than a fixed
   wait (2026-10-19: main's queued 20 minutes behind a run started by
   hand while they gave up at 10); GitHub keeps one queued job per group,
   so a newer one cancels the one queued before it, which a branch skips
   with a warning and main fails (ci-through-cloudflare.sh). Its first
   run (macOS, AnyConnect) found pf left on with Crake's NAT rule after
   the tunnel was off and craked gone (macOS `down` gave up at the first
   route a gone utun refused; fixed, and the gateway tests check the
   anchor is empty since).
   Later: OpenVPN's browser sign-in through Cloudflare (its
   page, sso.py on 8080, needs a third hostname) and openconnect's fake
   servers for the other protocols' sign-in.
8. **Someday: other platforms** (see below). Not before Linux is solid.

