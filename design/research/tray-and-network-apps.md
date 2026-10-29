# Tray and menu-bar network tools: a reference study for Crake

Researched October 2026 for a design pass on Crake's surfaces: the tray icon and its states, the tray panel (header, "What Crake did", "Tunnels", the footer's Full log and Copy for helpdesk), the plain fallback menu (summary line, tunnels with check marks, Open Crake, Settings, Quit), the system notifications (routed, "Ask me first", "When two tunnels answer", a tunnel that stopped), the log, tunnel detail (facts, policy, Learned / Pinned / Set by the gateway), Settings → Learning and Names, New tunnel's step 3 ("What should go through it?"), the sign-in window's "Couldn't reach" state, and the Mac menu bar (design/proposals/menu-bar-mac.html).

**How to read this.** Each app section lists only what Crake can draw or do, grouped under the lenses that apply to that app: status at a glance, the one-sentence state, explaining a decision, undo and a rule's scope and lifetime, alerts and notifications, split tunnelling and per-app or per-host rules, the menu itself, and empty and error states. Points marked *(observed)* come from my own knowledge of the shipping app, not from a cited page; check them against the current build before quoting them as fact. Everything else links to a source. Each app ends with **For Crake:**, naming the pattern, the screen and the section, with copy in Crake's voice where it helps. Patterns that run into DESIGN.md's "Directions we tried and rejected" aren't recommended; they're listed at the end under "Not for Crake".

**Selection.** The brief's candidates, minus one: the Apple Design Award lists for 2023 to 2026 have no menu-bar app among their Mac winners and finalists. The Mac entries are games, Flighty, Tide Guide, Structured, Mela, iA Writer, Copilot Money, Crouton, Moonlitt and the like ([2026](https://developer.apple.com/design/awards/), [2025](https://developer.apple.com/design/awards/2025/), [2024](https://developer.apple.com/design/awards/2024/), [2023](https://developer.apple.com/design/awards/2023/)). Those are covered for a sibling app in Godwit's `design/research/mac-apps.md`; nothing on those lists lives in the menu bar or touches the network.

| App | Why it's here | Version looked at |
|---|---|---|
| Tailscale | A mesh VPN that routes almost nothing by default, like Crake; health warnings with a delay and dependencies; menu bar, tray and a window | 1.96 (macOS window default from 1.96.2; Linux systray beta from 1.88) |
| Little Snitch 6 | The reference for a decision with a scope and a lifetime, and for "why was this allowed?" | 6.x (6.2 network monitor, 6.3 Tahoe, 6.4.1) |
| LuLu | The same idea, open source, in fewer words: Rule Scope and Rule Duration | 4.5.3 |
| Cloudflare One client (WARP) | Include/exclude split tunnels by host and domain, and a switch that shows intent, not state | current docs, October 2026 |
| Mullvad VPN | One app state shown the same way in the window and the tray; honest limits | desktop app, help updated September and October 2026 |
| Proton VPN | Include and exclude modes named per platform; reconnect rules said out loud | Windows app after the April 2025 redesign; Linux GTK app 4.x |
| WireGuard | The plainest tunnel menu; on-demand rules that fight the off switch | macOS App Store app (version not checked) |
| iStat Menus 7 | Notifications from rules, temporary versus persistent | 7.5 |
| Time Machine and Backblaze | A status line that answers "am I fine?" with a time | macOS 27 Time Machine; current Backblaze Mac client |
| Apple Design Award winners in the menu bar | None found on the 2023 to 2026 lists (above) | n/a |

---

## 1. Tailscale (1.96)

A mesh VPN on macOS, Windows and Linux. Without an exit node it routes only traffic between your own devices; "public internet traffic, such as visiting a website, goes directly from your device" ([Tailscale: Exit nodes](https://tailscale.com/kb/1103/exit-nodes?tab=macos)). That's the closest product stance to Crake's in this list.

**Status at a glance**
- **The menu leads with the state.** The macOS menu's first item is the connection switch with its word ("Connected"), then "This Device" with its name and address, then Network Devices and Exit Nodes *(observed)*.
- **The tray mark changes only for trouble that cuts you off.** A health warning's `ImpactsConnectivity` field decides it: "the client will display an exclamation mark on the tray icon when ImpactsConnectivity is set to true and the Warnable is unhealthy" ([health.go](https://raw.githubusercontent.com/tailscale/tailscale/main/health/health.go)). Every other warning stays inside the menu or window.
- **A red dot on the Dock icon only for critical errors.** "Critical errors will be indicated by a very visible red dot"; opening the app shows the error at the bottom of the sidebar ([Tailscale: windowed macOS UI beta](https://tailscale.com/blog/windowed-macos-ui-beta), 26 September 2025).

**Alerts and notifications**
- **Notifications for trouble, nothing else.** Tailscale says it uses notifications "only to alert you of connectivity trouble" ([same post](https://tailscale.com/blog/windowed-macos-ui-beta)).
- **A warning must last before it's shown.** `TimeToVisible` is "the Duration that the Warnable has to be in an unhealthy state before it should be surfaced as unhealthy to the user. This is used to prevent transient errors from being displayed to the user" ([health.go](https://raw.githubusercontent.com/tailscale/tailscale/main/health/health.go)).
- **A warning that follows from another is held back.** `DependsOn` lists warnings that must be healthy "before this Warnable is relevant… if any of these Warnables are unhealthy, then this Warnable is not relevant" (same file). One cause, one message.
- **Each warning has a one-line title and a longer text.** "The title should be short and fit in a single line"; the text is generated with the details, and severity (high, medium, low) decides how intrusive it is: high "could trigger a modal view", low is "displayed in a less intrusive way" (same file).
- **Messages name the cause and the fix together**, for example "Linux DNS configuration issue" (enable systemd-resolved, run as root, or set DNS by hand) and "Tailscale blocked by Screen Time" ([Tailscale: client messages](https://tailscale.com/docs/reference/messages/client.md)).

**Explaining a decision; scope of a rule**
- **The exception sits next to the rule.** Under Exit Nodes, "Allow Local Network Access" keeps your own network reachable while everything else goes through the exit node ([Exit nodes](https://tailscale.com/kb/1103/exit-nodes?tab=macos)). The scope of the big rule and its exception are one menu apart.
- **A recommended choice with its reason.** The window suggests an exit node "based on latency, performance, and location" and remembers the last one ([windowed UI beta](https://tailscale.com/blog/windowed-macos-ui-beta)).

**The menu itself; when the menu can't be seen**
- **Notch detection.** The Mac app reads its status item's `occlusionState` and, when the icon is hidden behind the notch, says so in a window: "The Tailscale icon is hidden due to limited space in the menu bar", with advice to rearrange other icons. The long-term answer was a windowed app, on by default from 1.96.2 ([Tailscale: macOS notch escape](https://tailscale.com/blog/macos-notch-escape), 27 March 2026).
- **GNOME limits submenu depth.** On Linux the systray "provides some common actions like fast user switching and exit node selection", and because "the GNOME desktop restricts the depth of sub-menus", exit-node choice stops at the country ([Tailscale: Linux systray](https://tailscale.com/docs/features/client/linux-systray.md)).
- **Quitting asks what you mean.** From 1.90, quitting the window asks whether to quit fully or keep Tailscale in the menu bar ([windowed UI beta](https://tailscale.com/blog/windowed-macos-ui-beta)).

**For Crake:** take `TimeToVisible`, `DependsOn` and `ImpactsConnectivity` as three rules for the tray's **trouble** badge and the panel's notices. (1) A tunnel that drops and is back within a few seconds doesn't earn the red badge; the log still records it. (2) When ACME's gateway can't be reached, don't also raise "ACME's DNS isn't answering" (crake-keep, 7h): the gateway is the cause, the DNS follows from it. (3) Only something that cuts off a tunnel touches the bird; an AnyConnect user-agent fallback or a UDP-blocking network (7b) belongs in the log and tunnel detail, not on the tray. It's worth checking how the trouble state is raised today (`model::trouble_words` and `shape_untroubled`) before claiming this is new. Second, take notch detection for the Mac menu bar: if the bird's status item is occluded, Crake says so once in the panel's notice shape ("The bird is hidden behind the notch. Make room in the menu bar, or open Crake from Applications.", Got it / Not now). That is new work on macOS; Linux already has the no-tray handling (no-tray-after.html). Third, keep the fallback menu flat: GNOME cuts submenus, and Crake's fallback is already one level (summary line, tunnels, Open Crake, New tunnel…, Settings…, Quit).

---

## 2. Little Snitch 6

An outbound firewall for macOS. Every connection meets a rule or an alert; every rule has a scope and a lifetime. It's the most direct reference for Crake's "decision plus undo".

**Explaining a decision**
- **From any connection to the rule that decided it.** Right-click a connection in Network Monitor and choose "Show Corresponding Rule": the rules window opens "with the deciding rule selected" ([Objective Development: support](https://obdev.at/support/littlesnitch/md3i7)).
- **A denied connection shows itself as it happens**: "a red flashing of that connection" in Network Monitor ([same page](https://obdev.at/support/littlesnitch/md3i7)).
- **Rules say when they're useless.** Configuration marks a rule "redundant, it never matches a connection", and another that "covers other rules, making them redundant" ([Little Snitch 6 Help: icons](https://help.obdev.at/littlesnitch6/lsc-icons)).

**Undo, scope and lifetime**
- **Answering an alert is creating a rule.** "Your answer creates a rule covering that connection and similar future ones. In the factory configuration, the rule covers the entire destination domain" ([Help: operation modes](https://help.obdev.at/littlesnitch6/concepts-opmodes)).
- **Scope is chosen on the name itself.** "Clicking the domain name opens a menu to choose between domain and host rules" ([Help: connection alert](https://help.obdev.at/littlesnitch6/alert-overview)). The scope control is the thing it scopes, not a separate field.
- **Lifetime is part of the rule.** "Choose 'forever' or the event for which the rule should be deleted" (same page). Little Snitch 5 lists those events: "until logout or restart, until the process terminates or for a period of time" ([Help 5: connection alert](https://help.obdev.at/littlesnitch5/alert-overview)). "Once" "affects only one data packet and no rule is created" (same page).
- **Where the rule applies.** The alert's Profile control sets whether a rule holds in the current profile or in all of them ([Help: connection alert](https://help.obdev.at/littlesnitch6/alert-overview)).

**Alerts and notifications**
- **An alert can wait, but the connection waits with it.** Minimized, the alert "shows the number of stalled connection attempts"; "The connection attempt is stalled until you answer the alert!" ([Help: minimized alert](https://developer.obdev.at/littlesnitch6/alert-minimized)).
- **Quiet mode that still keeps a to-do list.** In Silent mode, unmatched connections are allowed or denied without a word, but Network Monitor marks each with rule-management buttons "which mean your decision is still pending" ([Help: operation modes](https://help.obdev.at/littlesnitch6/concepts-opmodes)).
- **Notification Center alerts and sounds arrived with 6** ([release notes 5→6](https://obdev.at/products/littlesnitch/releasenotes-swu-5to6.html)).

**Status at a glance; the menu**
- **The menu-bar icon can carry a five-minute traffic chart or a live rate**, flash when a connection is blocked, and show "a warning triangle in the menu bar if Little Snitch is currently inactive because the network filter is turned off" ([Help: status menu](https://help.obdev.at/littlesnitch6/pref-statusmenu)). The menu shows whether Silent Mode is on (same page).

**For Crake:** take **"Show Corresponding Rule"** for the log and the panel. Every decision line already carries its reason; give the reason a way to the thing that decided it. A host left alone because of the user's list: "Left 192.0.2.40 alone." / "It's on your Never send through a tunnel list." with **Show the list**, opening Settings → Learning at "Never send through a tunnel" with that entry selected. A host outside IT's scope: "ACME IT allows learning only for *.acme.internal." with **Show the policy**, to Settings → Managed by IT. A routed host: **Show in ACME**, to tunnel detail → Learned with the row selected. One link per line, in the small-action look (ink, underlined), never teal. Second, take the **lifetime said with the rule**. Crake already has three lifetimes; it just doesn't name them in one way: learned ("Forget hosts unused for 7 days"), pinned (until you unpin it), undone ("Crake won't learn this host again on its own"). In tunnel detail's Learned rows, say it after the reason: "Learned at 13:21 · no answer directly · goes after 7 days unused"; Pinned rows: "Pinned · stays until you unpin it". Third, take Silent mode's pending buttons as confirmation of what Crake already does: "Ask me first" puts a line in "What Crake did" with **Send through ACME** (`panel/decisions.rs`), so the question outlives the notification. As far as I can see the "When two tunnels answer: Ask me" question doesn't: it's a System entry, which the panel's decisions leave out, so "Use Nordbank" exists only in the notification. Give it the same line in "What Crake did": "Nordbank answers for git.acme.internal too." / "It's going through ACME (38 ms). Nordbank answered in 61 ms." with **Use Nordbank**.

---

## 3. LuLu (4.5.3)

Objective-See's free, open-source outbound firewall for macOS, maintained by Patrick Wardle.

**Undo, scope and lifetime**
- **Two named choices under the decision.** "Rule Scope": by default the rule covers the whole program; "Remote Endpoint" limits it to the destination being contacted. "Rule Duration": always, the process's lifetime, or "Expires in" hours and minutes, after which "the rule is deleted" ([Objective-See: LuLu](https://objective-see.org/products/lulu.html)).
- **A "temporarily" answer creates no rule at all** (same page).
- **Rules survive moves and updates** because signed programs are matched by name and bundle identifier, not by path (same page).

**Explaining a decision**
- **The alert shows who and where, and offers more on request**: process name and destination, with the full path or URL on hover; buttons for code signing, the process's "Origins" (its parents) and VirusTotal; "Details and Options" opens the rule controls (same page).
- **The rules window has a "Recent" filter**: rules from the last 24 hours (same page).

**Quiet mode**
- **Passive mode** applies existing rules and shows no alerts; a setting decides whether new connections are allowed or denied and whether rules are created for them (same page).
- **A block list overrides rules**; a remote list reloads once a day (same page).

**For Crake:** LuLu's labels show how few words a scope and a lifetime need, and Crake's New tunnel → step 3 ("What should go through it?") already has both: the mode (scope) and "Forget hosts unused for" (lifetime). Take LuLu's **"Recent"** for tunnel detail: the Learned tab's rows are already by time, so a muted heading over today's ("Today", "Earlier") lets someone answer "what changed since this morning?" without the log. And take **"Expires in"** only as a question for later, written down: a host added by hand in tunnel detail ("Add a host", "Send through ACME") is pinned forever; "for today" is a real need (a one-off build server) but it is new work in craked's sets and a second decision in a row that today has one. Don't add it to the Undo action: one action, one meaning.

---

## 4. Cloudflare One client (formerly WARP)

Cloudflare's device client for its Zero Trust network; the consumer WARP app shares the shell.

**Status at a glance; the one-sentence state**
- **The switch shows what you asked for; the sentence under it shows what is.** "The toggle button in the WARP GUI shows the intended state, not the actual state… if the toggle is on, it means that the client intends to connect… the actual status may show disconnected if there's an issue. So always check the message below the toggle for the current connection state" ([Cloudflare: WARP basics video](https://developers.cloudflare.com/videos/warp-1-basics/)).
- **Errors have a code and a human cause.** CF_CAPTIVE_PORTAL_TIMED_OUT, CF_DNS_LOOKUP_FAILURE, CF_HAPPY_EYEBALLS_MITM_FAILURE (usually "a router, firewall, antivirus, or other security product blocking UDP"), each with a fix ([Cloudflare: client errors](https://developers.cloudflare.com/cloudflare-one/team-and-resources/devices/cloudflare-one-client/troubleshooting/client-errors/)).

**Split tunnelling by host and domain**
- **Two modes, named by what they list.** "Exclude IPs and domains" (the default: everything goes through except the list) and "Include IPs and Domains" (only the list goes through) ([Cloudflare: Split Tunnels](https://developers.cloudflare.com/cloudflare-one/team-and-resources/devices/warp/configure-warp/route-traffic/split-tunnels)). Each entry can carry a description (same page).
- **A domain rule is really an address rule, and the docs say what follows.** The client adds each domain's resolved address to the routing table; so a domain on a shared provider "can unintentionally include or exclude other hostnames on the same IP", and cached or secure-DNS lookups can slip past (same page). `example.com` matches only itself; `*.example.com` matches subdomains but not the apex (same page).
- **Routing and naming are separate rules.** Split Tunnels "only affects IP traffic, so DNS is still resolved by Gateway unless the domain is added to Local Domain Fallback", which is also how private names get resolved (same page).

**Undo, lifetime**
- **Temporary exceptions have a clock.** On a captive portal the client disconnects "and it will automatically reconnect after the Timeout duration"; an admin override code lets a user switch the client off for a set time, counted in fixed hour blocks ("A code generated at 9:30 AM with a 1-hour timeout will expire at 10:00 AM") ([Cloudflare: captive portal detection](https://developers.cloudflare.com/cloudflare-one/team-and-resources/devices/cloudflare-one-client/configure/settings/captive-portals/); [Cloudflare: device client settings](https://developers.cloudflare.com/cloudflare-one/team-and-resources/devices/cloudflare-one-client/configure/settings)).

**For Crake:** take **intent versus state** for the tray panel's **Tunnels** rows and tunnel detail's header. The switch is what you asked for; the word under the name is what is. A switched-on tunnel that isn't up must never read as up: "Nordbank" / "Not connected: the gateway didn't answer." (the switch on), not a bare "Off". Crake's trouble words (`trouble_words`, trouble-reason.html) already say why; the rule to keep is that the switch and the words may disagree, and the words win. Second, take Cloudflare's **shared-address honesty** for tunnel detail → Learned. Crake learns names but routes addresses (craked's nftables sets hold addresses), so every other name on that address goes through too. Say so where it happens: "assets.acme.example · learned with git.acme.internal (same address)", or in the "?" beside "Ask a tunnel's DNS for these names" in Settings → Names: "Crake routes addresses. A name that shares an address with a routed one goes through too." Showing which names share an address is new work (craked would have to keep the names it saw per address); the sentence in the "?" isn't. Third, Cloudflare's "routing and naming are separate" is exactly Crake's split between Settings → Learning and Settings → Names; keep the two sections apart and keep saying which one a setting belongs to.

---

## 5. Mullvad VPN

A privacy VPN for every desktop; the reference for one state told the same way everywhere.

**Status at a glance; the one-sentence state**
- **Window and tray say the same state.** Connected: the window's top bar is green and says "Connected", and the padlock in the tray or menu bar "will turn green and lock". Disconnected: top bar red, "Disconnected" in red, padlock red. Blocking: the connection screen says "BLOCKING INTERNET" and the padlock is green with a red dot ([Mullvad: using the app](https://mullvad.net/en/help/using-mullvad-vpn-app), updated 18 September 2026).
- **The consequence is the label.** While connecting, "the app will block the internet until a connection to a VPN server is established" (same page): the state is named by what it does to you, not by the mechanism.

**Empty and error states**
- **An error locks, says why, and waits for you.** The error state "locks the firewall so no traffic can flow"; the user is told what the problem is and must click disconnect to unlock ([Mullvad: security.md](https://github.com/mullvad/mullvadvpn-app/blob/main/docs/security.md)).
- **A name people get wrong, written down.** Lockdown mode "is regularly misunderstood as the kill switch"; it "only changes whether or not the disconnected state should allow traffic to flow freely" (same document).
- **Small, in-place notices for what needs you later**: an update, or an account three days from running out, shown on the connection screen with a yellow dot on the padlock ([using the app](https://mullvad.net/en/help/using-mullvad-vpn-app)).

**Split tunnelling (per app)**
- **Per-app exclusion only, with its limits listed**: Safari and other WebKit apps can't be excluded on macOS; only outgoing connections; Windows Store apps can't be excluded; excluded apps "still use the DNS server on the connected Mullvad server"; reverse split tunnelling isn't planned because "a VPN should be the default" ([Mullvad: split tunneling](https://mullvad.net/en/help/split-tunneling-with-the-mullvad-app), updated 20 October 2026).
- **A way to check the rule works**: exclude a browser and open the Connection check, which should say "Not using Mullvad VPN" (same page).

**For Crake:** take **"the consequence is the label"** for policy lines in the panel's **Tunnels** and in tunnel detail, which Crake already does for Globex ("Full tunnel required by Globex IT. Crake won't learn here."); keep every new state to that shape. Take Mullvad's **written-down limits** for Settings' "?" cards: each says what the setting doesn't do, as "Clear the log" already does ("Learned hosts stay. Only the history goes."). The first to check is "Never send through a tunnel": does a name on that list still get asked of a tunnel's DNS by "Ask every tunnel when a name is unknown"? Whatever craked does, the "?" says it in one sentence. And take Mullvad's **check that a rule works** for tunnel detail's "Add a host": after Send through ACME, the row could say what happened next ("Answered through ACME in 41 ms" once traffic flows), which is the probe Crake already runs when it learns; running it for a hand-added host is new work.

---

## 6. Proton VPN

A privacy VPN with apps on every platform; its split tunnelling is the clearest example of a scope that has to be named per platform.

**Split tunnelling (per app and per address)**
- **Two modes, but their names drift.** Windows says "Exclude mode" and "Include mode"; macOS has only exclusion; Android's mode button offers "Exclude" and "Inverse"; Android TV says "Exclude" or "Include" ([Proton: split tunneling](https://protonvpn.com/support/protonvpn-split-tunneling)).
- **When a change takes effect is said where you make it.** Windows: "Apply" if disconnected, "Reconnect" if connected; Linux: reconnect, then restart each listed app, each time you connect (same page).
- **Platform limits are in the open.** On macOS split tunnelling is "currently an experimental feature" that doesn't work with Safari or other WebKit apps; on most platforms it isn't compatible with the kill switch (same page).

**Status at a glance**
- **The redesigned Windows app leads with a map**: "Know at-a-glance that you're protected by Proton VPN with a map showing your connection location", plus NetShield counts of ads and trackers blocked ([Proton: new Windows and iOS apps](https://protonvpn.com/blog/new-windows-ios-apps), 3 April 2025). The header reads "Protected" or "Unprotected" with your address *(observed)*.
- **On Linux the tray is a remote control**: it "stays active when you close the main Proton VPN window" and offers Quick Connect or Disconnect, plus pinned servers ([Proton: tray quick connect](https://protonvpn.com/support/quickly-connect-via-systems-tray/)).

**For Crake:** take the **lesson of the drifting names**: a mode's name is the same everywhere it appears. New tunnel's step 3 names Crake's four modes "Learn as it goes", "Ask me first", "Only what I list" and "Everything", but the tray panel's **Tunnels** policy line says "Your profile · learning on". Use the mode's own name there: "Your profile · Learn as it goes", "Your profile · Only what I list". Second, take **"when it takes effect"** for every Settings → Learning row whose effect isn't obvious, and say it once in the section's lede if it's true of all of them: "Changes take effect at once. Nothing reconnects." (check that it is true for "Wait for a direct answer" and "Try tunnels in this order" before writing it).

---

## 7. WireGuard (macOS app)

The official WireGuard client for macOS and iOS. Deliberately plain; included for the menu and for one trap.

**The menu itself**
- **Tunnels as a list you tick, then one way into the window.** The macOS app adds "Bringing up / down a Wireguard tunnel from the status bar"; the Dock icon appears only while a window is open ([Roopesh Chander: WireGuard for iOS and macOS](https://roopc.net/work/wireguard/)). The menu lists each tunnel with a check mark and ends with "Manage Tunnels" *(observed)*.

**Status at a glance**
- **Proof of life, in words and a time.** A tunnel's detail shows its status, "Latest handshake" as a relative time, and data received and sent *(observed)*; the developer lists "Showing bytes transferred for active tunnels" as a feature ([roopc.net](https://roopc.net/work/wireguard/)).

**Undo, scope and lifetime: the trap**
- **On-demand rules by interface type and by Wi-Fi name** ([roopc.net](https://roopc.net/work/wireguard/); [WireGuard list, 2022](https://lists.zx2c4.com/pipermail/wireguard/2022-April/007586.html)) **fight the off switch.** A user "not able to quit/deactivate the wireguard session" was told the cause was on-demand being on; turn it off, then the tunnel can be deactivated ([WireGuard list, January 2020](https://lists.zx2c4.com/pipermail/wireguard/2020-January/004876.html)).

**For Crake:** Crake's fallback menu already has WireGuard's shape (tunnels with check marks, Open Crake), and should keep it. Take **proof of life** for tunnel detail's facts: beside "Connected for 4 h 53 min", a line that says the tunnel is answering, not only that it was set up ("Last heard from the gateway 6 s ago"). That is new work: craked would need openconnect's keepalive or dead-peer timing, which the hook doesn't forward today. Take the **trap** as a rule for "Switch on the tunnels you left on" and any reconnect Crake does by itself: a tunnel the user switched off stays off until the user switches it on, and the panel never shows a switch that won't stay where it was put.

---

## 8. iStat Menus 7 (7.5)

Bjango's system monitor in the menu bar. Here for its notifications, not its meters.

**Alerts and notifications**
- **Notifications come from rules the user sets**: "The rules tab can be used to set up notifications [that] let you know when an event relating to iStat Menus' monitoring has happened" ([Bjango help: rules](https://bjango.com/help/istatmenus7/rules/)). 7.0 extended them to CPU, GPU, memory, disks, network, sensors, battery, power and weather ([MacRumors: iStat Menus 7.0](https://macrumors.com/2024/07/31/istat-menus-7-0-brings-new-features/)), including "alerts when your public IP changes" ([Bjango: iStat Menus](https://bjango.com/mac/istatmenus/)).
- **Two weights, chosen per kind.** "Temporary notifications appear for a while, then disappear. Persistent notifications remain on your screen until closed"; the app splits them between two helpers so macOS can show each kind its own way ([Bjango help: rules](https://bjango.com/help/istatmenus7/rules/)).
- **A condition has a duration.** A user's example rule: notify "if my MacBook Pro's CPU usage hits 80% for 30 seconds" ([MacSales: iStat Menus 7](https://eshop.macsales.com/blog/95542-istat-menus-7-is-the-latest-in-a-long-line-of-delightful-mac-monitoring-apps-from-bjango/)).

**The menu itself**
- **Combined mode** merges items into one menu-bar item for laptops short of room; you choose what shows in the bar and what in the menu ([Bjango](https://bjango.com/mac/istatmenus/)). The network menu adds link speed and a network locations item in 7.0.2 ([TidBITS](https://tidbits.com/watchlist/istat-menus-7-0-2)).

**For Crake:** take **temporary versus persistent** for Crake's notifications, and decide it per kind, not per setting: a routed host is temporary (Settings → General's "Quiet notification", with Undo); a question ("Ask me first", "When two tunnels answer") stays until answered or until the panel's line answers it; a tunnel that stopped on its own stays until it's back or dismissed. On Linux that's the notification's urgency and timeout; on macOS, the alert style the system offers per app. Today `notify.rs` gives a routed host `Urgency::Low` and every other kind `Urgency::Normal`; the questions and the stopped tunnel are the ones to make persistent. Take **a condition with a duration** as the same idea as Tailscale's `TimeToVisible`, in the user's words where it surfaces: "Nordbank has been down for 2 minutes."

---

## 9. Time Machine and Backblaze

Two backup menu extras. Neither touches the network the way Crake does; both answer "am I fine?" in one line.

**The one-sentence state**
- **Time Machine** shows progress while a backup runs and, when none is running, the date and time of the latest backup ([Apple Community, quoting Apple's guidance](https://origin-discussions2-us-dr-prz.apple.com/thread/254133254)); its actions are "Back Up Now" and "Stop This Backup" ([Apple: start, pause or resume a backup](https://support.apple.com/guide/mac-help/mh19531/mac)). The menu's first line reads like "Latest Backup to Studio: Today, 13:02" *(observed)*.
- **Backblaze** says "You are backed up as of" a time, which is when the remaining files last reached "0 files" ([Backblaze: Mac settings](https://www.backblaze.com/computer-backup/docs/mac-settings-and-system-preferences)); "Backup Now" starts one (same page).

**Empty and error states**
- **A mark on the icon for a problem, explained in the menu.** Time Machine's menu-bar icon takes an exclamation mark when it can't reach the backup disk ([Apple Community](https://discussions.apple.com/thread/255314146) and [MacRumors forum](https://forums.macrumors.com/threads/618196), both anecdotal) *(observed)*.
- **The icon can vanish without the app knowing**, when the menu bar is full ([Macworld via search](https://www.macworld.com/article/2588350/time-machine-incorrect-battery.html); [Apple Community](https://discussions.apple.com/thread/255314146)).

**For Crake:** Crake's fallback menu and tray tooltip already open with a summary line ("Listening · ACME, Nordbank", `model::summary`). Take the **time** from Time Machine and Backblaze: the line says the last thing Crake did and when, because "Listening" alone doesn't tell you whether Crake has done anything today. "Listening · ACME, Nordbank · last routed a host at 13:21", or with nothing done, "Listening · ACME, Nordbank · nothing routed today". The panel's header stays as it is (DESIGN.md: the header carries only status); "What Crake did" already gives the time per line.

---

## What to take, in order

1. **Trouble that earns the badge** (Tailscale `TimeToVisible`, `DependsOn`, `ImpactsConnectivity`). A short drop doesn't raise the red badge; one cause, one notice; only what cuts off a tunnel touches the bird. **Where:** tray icon states, the panel's notices, notifications for a tunnel that stopped.
2. **From a reason to the rule that made it** (Little Snitch "Show Corresponding Rule"). Show the list, Show the policy, Show in ACME, one per line. **Where:** the panel's "What Crake did", the log's rows.
3. **The lifetime said with the rule** (Little Snitch, LuLu). Learned: "goes after 7 days unused"; Pinned: "stays until you unpin it"; Undone: "Crake won't learn this host again on its own". **Where:** tunnel detail → Learned and Pinned rows; the panel's undone line (already said).
4. **Intent and state shown apart** (Cloudflare One client). The switch is what you asked; the words are what is, and win. **Where:** the panel's Tunnels rows; tunnel detail's header.
5. **A question that outlives its notification** (Little Snitch Silent mode). "Ask me first" already does this; "When two tunnels answer: Ask me" doesn't yet. **Where:** the panel's "What Crake did", with Use Nordbank.
6. **One name per mode, everywhere** (Proton's drifting labels). **Where:** the panel's Tunnels policy line ("Your profile · Learn as it goes"), tunnel detail's policy, New tunnel step 3.
7. **Addresses are what's routed; say so** (Cloudflare's shared-IP warning). **Where:** Settings → Names "?" now; tunnel detail → Learned's shared names later (new work).
8. **Temporary or persistent, by kind** (iStat Menus). **Where:** notifications (`notify.rs`), Settings → General's "When Crake routes a host" keeps governing only the routed kind.
9. **The status line with a time** (Time Machine, Backblaze). **Where:** the fallback menu's first line and the tray tooltip.
10. **The bird hidden behind the notch** (Tailscale). **Where:** the Mac menu bar; a notice in the panel. New work.
11. **Proof of life** (WireGuard's latest handshake). **Where:** tunnel detail facts. New work in craked.
12. **The off switch is the last word** (WireGuard's on-demand trap). **Where:** Settings → General "Switch on the tunnels you left on", reconnect behaviour.

---

## Not for Crake

- **A map of where you are** (Mullvad's dot on a map, Proton's Home screen map, Little Snitch's Network Monitor map). DESIGN.md: "The internet is never drawn. There's no map; the log and charts carry it."
- **State told by the icon's colour** (Mullvad's green, red and red-dotted padlock; Cloudflare's red WARP banner versus blue Zero Trust banner). The bird changes shape, never colour; only the state marks are coloured, each with one meaning, and tunnels are told apart by name.
- **Marks that need a legend** (Mullvad's yellow versus red exclamation marks beside apps on Linux, meaning "close it first" and "can't be used"; Little Snitch's blue and orange rule-management buttons; Tailscale's dot-grid tray icon whose pattern changes with an exit node *(observed)*). The user drops any symbol that needs a legend or a tooltip; say it in words.
- **Blinking and flashing as status** (Little Snitch's blinking orange button on a stalled connection, its red flashing rows, its icon flashing on every block). Too close to the rejected breathing and pulsing dots: status is said in words, and motion is kept for the few things Crake does (the routed call, going to sleep).
- **Shouted states** (Mullvad's "BLOCKING INTERNET"). Uppercase is the rejected website look, and the copy voice is short declaratives: "Nothing gets through until ACME connects." if that's ever true of Crake.
- **Playful copy in a warning** ("Sneaky!" in Tailscale's notch window). No exclamation marks, no "Oops", in Crake.
- **Counters as reassurance** (Proton's NetShield counts of ads and trackers blocked; iStat Menus' live rate or chart in the menu bar; Little Snitch's traffic chart in the status item). Numbers that don't answer a real question, and the generic dashboard (cards with numbers) DESIGN.md rejects. Crake's one chart per surface already says what it counts.
- **A modal question that holds the connection** (Little Snitch's alert: "The connection attempt is stalled until you answer the alert!"; LuLu's alert). Crake holds a first SYN for at most one direct round trip (`DIRECT_WAIT`, 400 ms) and otherwise asks without holding anything ("Ask me first" routes nothing and asks in a notification and a panel line); an alert that freezes an app until answered makes Crake an app you live in.
- **A three-pane monitor** (Little Snitch's Network Monitor: list, inspector, map). The tern-like three-pane shape DESIGN.md rejects; Crake's log is one column of sentences.
- **Per-app split tunnelling** (Mullvad, Proton, Tailscale on Android). Crake routes per host and has no per-app routing; it would be new work in every OS layer, and it answers a different question ("which app?") than Crake's ("which host needs a tunnel?").

---

## Sources

Tailscale
- [Exit nodes (macOS)](https://tailscale.com/kb/1103/exit-nodes?tab=macos) · [Windowed macOS UI beta](https://tailscale.com/blog/windowed-macos-ui-beta) · [macOS notch escape](https://tailscale.com/blog/macos-notch-escape) · [Linux systray](https://tailscale.com/docs/features/client/linux-systray.md) · [Client messages](https://tailscale.com/docs/reference/messages/client.md) · [health.go (Warnable)](https://raw.githubusercontent.com/tailscale/tailscale/main/health/health.go)

Objective Development (Little Snitch 6)
- [Connection alert](https://help.obdev.at/littlesnitch6/alert-overview) · [Minimized alert](https://developer.obdev.at/littlesnitch6/alert-minimized) · [Operation modes](https://help.obdev.at/littlesnitch6/concepts-opmodes) · [Icons](https://help.obdev.at/littlesnitch6/lsc-icons) · [Status menu](https://help.obdev.at/littlesnitch6/pref-statusmenu) · [Show Corresponding Rule](https://obdev.at/support/littlesnitch/md3i7) · [Release notes 5→6](https://obdev.at/products/littlesnitch/releasenotes-swu-5to6.html) · [Little Snitch 5 connection alert (lifetimes)](https://help.obdev.at/littlesnitch5/alert-overview) · [TidBITS: 6.2](https://tidbits.com/watchlist/little-snitch-6-2/)

Objective-See
- [LuLu](https://objective-see.org/products/lulu.html)

Cloudflare
- [Split Tunnels](https://developers.cloudflare.com/cloudflare-one/team-and-resources/devices/warp/configure-warp/route-traffic/split-tunnels) · [WARP basics video](https://developers.cloudflare.com/videos/warp-1-basics/) · [Client errors](https://developers.cloudflare.com/cloudflare-one/team-and-resources/devices/cloudflare-one-client/troubleshooting/client-errors/) · [Captive portal detection](https://developers.cloudflare.com/cloudflare-one/team-and-resources/devices/cloudflare-one-client/configure/settings/captive-portals/) · [Device client settings](https://developers.cloudflare.com/cloudflare-one/team-and-resources/devices/cloudflare-one-client/configure/settings)

Mullvad
- [Using the Mullvad VPN app](https://mullvad.net/en/help/using-mullvad-vpn-app) · [Split tunneling with the Mullvad app](https://mullvad.net/en/help/split-tunneling-with-the-mullvad-app) · [security.md](https://github.com/mullvad/mullvadvpn-app/blob/main/docs/security.md)

Proton
- [Split tunneling](https://protonvpn.com/support/protonvpn-split-tunneling) · [New Windows and iOS apps](https://protonvpn.com/blog/new-windows-ios-apps) · [Quick connect via the system tray](https://protonvpn.com/support/quickly-connect-via-systems-tray/)

WireGuard
- [Roopesh Chander: WireGuard for iOS and macOS](https://roopc.net/work/wireguard/) · [WireGuard list: on-demand and deactivating (2020)](https://lists.zx2c4.com/pipermail/wireguard/2020-January/004876.html) · [WireGuard list: on-demand per SSID (2022)](https://lists.zx2c4.com/pipermail/wireguard/2022-April/007586.html)

Bjango (iStat Menus 7)
- [iStat Menus](https://bjango.com/mac/istatmenus/) · [Help: rules](https://bjango.com/help/istatmenus7/rules/) · [MacRumors: 7.0](https://macrumors.com/2024/07/31/istat-menus-7-0-brings-new-features/) · [MacSales: iStat Menus 7](https://eshop.macsales.com/blog/95542-istat-menus-7-is-the-latest-in-a-long-line-of-delightful-mac-monitoring-apps-from-bjango/) · [TidBITS: 7.0.2](https://tidbits.com/watchlist/istat-menus-7-0-2)

Apple and Backblaze
- [Start, pause or resume a Time Machine backup](https://support.apple.com/guide/mac-help/mh19531/mac) · [Apple Community: where is the Time Machine menu](https://origin-discussions2-us-dr-prz.apple.com/thread/254133254) · [Apple Community: Time Machine icon missing](https://discussions.apple.com/thread/255314146) · [MacRumors forum: exclamation mark](https://forums.macrumors.com/threads/618196) · [Macworld: Time Machine battery alert](https://www.macworld.com/article/2588350/time-machine-incorrect-battery.html) · [Backblaze: Mac settings](https://www.backblaze.com/computer-backup/docs/mac-settings-and-system-preferences)
- Apple Design Awards: [2026](https://developer.apple.com/design/awards/) · [2025](https://developer.apple.com/design/awards/2025/) · [2024](https://developer.apple.com/design/awards/2024/) · [2023](https://developer.apple.com/design/awards/2023/)

Crake's own files read for the "For Crake" lines: `design/DESIGN.md`, `design/screens/` (tray-panel, log, tunnel-detail, new-tunnel, settings, sign-in), `design/proposals/menu-bar-mac.html`, `crates/crake/src/sys/linux/tray.rs` (fallback menu and tooltip), `crates/crake/src/model/mod.rs` (`summary`, `trouble_words`), `crates/crake/src/panel/decisions.rs` (Undo, Redo, Send through), `crates/crake/src/notify.rs` (routed, asked, also answered, stopped), `crates/craked/src/manager/probing.rs` ("When two tunnels answer").
