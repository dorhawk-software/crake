# Crake against the references

The approved screens (`design/screens/`, as of 29 Oct 2026: tray panel, log, tunnel detail, new tunnel, settings, sign-in) and the Mac menu bar (`design/proposals/menu-bar-mac.html`, built) set against three studies:
- [tray-and-network-apps.md](tray-and-network-apps.md): 9 tray and menu-bar network tools, 12 patterns.
- [linux-and-windows-conventions.md](linux-and-windows-conventions.md): GNOME Settings, Quick Settings, notifications, the AppIndicator extension, Dash to Panel, the HIG, Circle apps, Windows 11's tray, flyouts and toasts.
- [identity-and-delight.md](identity-and-delight.md): Teenage Engineering, Panic, Flighty, Linear, Things, Little Snitch, and three moments Crake doesn't draw yet.

Every sentence below that a screen would say was checked against craked's and the app's code (the strings are quoted from `manager/probing.rs`, `manager/naming.rs`, `manager/host_ops.rs`, `manager/hook.rs`, `manager/tunnels.rs`, `model/mod.rs`, `notify.rs`). Where a gap needs work the app doesn't do yet, it says **new work**.

## The headline

- **The bar is the Apple Design Award's, not Awwwards'.** No menu-bar app has been an ADA winner or finalist from 2023 to 2026; the closest winner in spirit is **Flighty** (Interaction, 2023): one line per flight, "the exact reason behind delays", and "we really have to shine when things go awry". Crake's whole idea is Flighty's: say what happened and why, in a line, with one number.
- **The reference closest to Crake's job is Little Snitch 6**: every connection meets a decision, every decision has a scope and a lifetime, and any connection leads to the rule that decided it ("Show Corresponding Rule"). It is also the clearest warning: it draws the stream (a map, a chart in the icon, a flash per block). Crake marks only its own decisions in a stream it leaves alone.
- **The product stance closest to Crake's is Tailscale's**: it routes almost nothing by default, and its tray mark changes only for trouble that cuts you off.

## Already right (keep)

- **The reason under every decision**, with one number ("It didn't answer directly. Through ACME it answered in 38 ms."): Flighty's identity, Little Snitch's corresponding rule, Tailscale's one-line title plus text.
- **One mark, one meaning.** The two-stroke tick is Things' checkbox: the mark of the act is the identity. Teal stays rare because it only marks decisions.
- **The switch is the state; the row's second line says it in Crake's words** ("Connected · 5 hosts", the trouble sentence). GNOME's VPN rows have only the switch; Crake's line is where it is better than the platform, not different from it.
- **The panel's shape** is a Windows flyout's (summary, tasks, one link) without tiles, and the GNOME HIG's banner is Crake's notice shape ("Not now", one action). "Copied" in place is the HIG's toast, kept in place.
- **The fallback menu is one level deep** (summary, tunnels with checks, Open Crake, New tunnel…, Settings…, Quit): GNOME cuts submenus (Tailscale's Linux tray), WireGuard's menu is the same shape.
- **The sign-in window is Warp-shaped**: one state per view, each with its next state, including "Open the page again".
- **New tunnel says the consequence where the choice is made**: step 3's "Everything" card already reads "Full tunnel. Crake won't learn anything." (the HIG's and Impression's rule; the study's suggestion is already there).
- **Sentence case and periods** against GNOME's header capitals: deliberate (DESIGN.md's copy voice); "Copy for Helpdesk" would read as another app.
- **Motion, then rest** (Things): a decision row slides in, the tick grows in two beats, the tray calls twice; no motion on anything that wasn't a decision.
- **One sound for one moment** (Playdate's crank): the optional "crex crex" belongs to a route and nothing else.

## Gaps, ranked (★ = biggest effect for least work)

Numbered across the whole pass. Some gaps are the screens drifting from DESIGN.md or from what the app writes; their reference is DESIGN.md or the code.

### Tray panel
1. ★ **The policy line names the tunnel's mode** (Proton VPN: a mode's name drifts across its platforms). Tunnels → Nordbank and Home lab: "Your profile · learning on" becomes **"Your profile · Learn as it goes"**, the name New tunnel's step 3 and tunnel detail give that mode; the other modes read "Your profile · Ask me first", "Your profile · Only what I list". App: one string in `panel/tunnels.rs` (today "Your profile · learning on").
2. ★ **The Listening dot stands still** (DESIGN.md's rejected list names this very screen's `breath` animation). Header → Listening: the dot loses its animation. Not visible in a still; the mockup's CSS changes.

### Log
3. ★ **The reasons are craked's own sentences** (Flighty: "the exact reason"; the screen had drifted). The list → every second line as craked writes it: "It didn't answer directly. Through ACME it answered in 38 ms." (not "No answer directly. ACME answered in 38 ms."); "Only ACME's DNS knows it (10.40.1.20). Routed before the first packet."; "Public name, private address: ACME's DNS knows it as 10.40.7.12. Routed before the first packet."; the undone row "cdn.acme.example goes direct again." / "You undid the route. Crake won't learn this host again on its own."
4. **The row pointed at leads to what decided it** (Little Snitch "Show Corresponding Rule"). The list → a routed row, pointed at, shows **Show in ACME** in the small-action look (ink, underlined) where its tunnel's badge is; it opens tunnel detail → Learned with that host's row selected. Only on the row pointed at, so the log stays a column of sentences. App: new work (a link from the log into tunnel detail with a row selected).

### Tunnel detail
5. ★ **The lifetime is said with the list** (Little Snitch: a rule's lifetime is part of it; LuLu's "Rule Duration"). Learned → one muted line between the tabs and the rows: **"Crake forgets a learned host after 7 days unused."** (7 days is Settings → Learning's "Forget hosts unused for", read live). Pinned: "Pinned hosts are kept until you forget them."; From the gateway: "ACME's gateway sent these when it connected." A line of the same height on every tab, so the list doesn't move between tabs. One sentence for the list, not a third clause on every row.
6. ★ **Copy for helpdesk says how each tunnel is connected** (identity study, moment 3; Flighty's "one line"; WireGuard's state in words). The copy opens with "Crake sends only the hosts that need a tunnel through one. Everything else goes direct." and each tunnel's line adds since when, to which gateway and with which engine: **"ACME: Connected · 5 hosts, since 08:31 to vpn.acme.example with openconnect, address 10.40.12.7"**. One count, from the same place as the panel. With "Hide names and addresses when sharing" on, the gateway and the address are left out (built in `model::helpdesk`, not by `scrub`). First drawn as a sentence under tunnel detail's facts; the finish review dropped that: it repeated the four facts right above it.
7. **The empty Learned tab names the tunnel** (GNOME HIG placeholders). "Nothing here." becomes **"Nothing learned through ACME yet."** Copy only; the sample data has no empty tab, so it isn't drawn.

### New tunnel
8. ★ **"Found a gateway" wears a plain check, not the routed tick** (DESIGN.md, Colour: "a confirmation ('Found a gateway' wears a plain check)"; the screen had drifted). Step 1 → the line under Gateway: an ink check, same words.

### Settings
9. ★ **Tunnels comes first in the sidebar** (DESIGN.md's Screens table and `proposals/settings-tunnels.html`, chosen; the screen had drifted). Sidebar → "Tunnels" above General. Learning → "Try tunnels in this order" leaves, as it did in the app (the order is Tunnels' list now). Only the sidebar and Learning are redrawn here; the Tunnels section itself is the chosen proposal.
10. **Names says that Crake routes addresses** (Cloudflare One's split-tunnel docs: a domain rule is an address rule, and other names on that address follow). Names → "Ask a tunnel's DNS for these names", its "?" card gains one sentence: **"Crake routes addresses. Another name on the same address goes through too."** Copy only; the Names section isn't the screen's default view, so it isn't drawn.

### Sign-in
11. ★ **The checks wear plain checks** (DESIGN.md, Colour: teal is never a confirmation). Single sign-on → "Reached gw.nordbank.example"; When it can't connect → "Internet is up", "Name found: 203.0.113.40": ink checks, not the routed tick. The red failure mark stays. The screen was drawn light only; the copy gets DESIGN.md's dark tokens so it can be compared in both (the app already follows the system).

### The tray's menu and the Mac menu bar
12. ★ **The menu's first line says the last decision and when** (Time Machine's "Latest backup", Backblaze's "backed up as of"). The fallback menu (GNOME, Windows) and the Mac's menu: **"Listening · ACME, Nordbank · last route 13:21"**, or "· nothing routed today"; past three tunnels, "Listening · 4 tunnels". On GNOME this line is the only place the bird's state is in words: the AppIndicator extension never shows tooltips. App: small (`model::summary`).

## Moments (new surfaces, drawn in `proposals/reference-pass/screens/moments.html`; gap 6 in `helpdesk.html`)

- **A. The first route.** Once per install, under the new row in "What Crake did", a neutral notice (surface tint, no teal, no amber) until Got it: **"Crake's first route."** / "Only git.acme.internal goes through ACME. Everything else still goes direct. Each time Crake sends a host through a tunnel, it shows here, and Undo puts it back." The notice spans the panel, in DESIGN.md's notice shape (title at 600, body, a bordered Got it). The bird, the sound, the notification and the log stay as for any route: the first call isn't louder. App: new work (a "seen" flag in app.json, as `seen_version`).
- **B. A tunnel coming back** (Flighty: shine on the bad day; Cloudflare One: the switch is what you asked for, the words are what is). While openconnect tries again, the panel row keeps its switch on and says **"Lost the gateway · trying again…"** (muted: nothing has stopped, nothing needs you); today it says "Connecting…", like a first connection. The log's lines get reasons: "ACME lost the gateway. Trying again." / "Its 5 hosts can't be reached until it's back. Everything else goes direct as usual."; "ACME is back." / "Gone for 42 s. Its 5 hosts go through it again." App: the row's words are small; "Gone for 42 s" is new work (craked keeps the time of the drop). The row carries the dashed head (Crake's sign for "under way", ink). The bird doesn't call: a reconnect isn't Crake's decision (openconnect also reconnects after a suspend or a network change). Drawn on ACME, beside its Managed badge.
- **C. A question that stays** (Little Snitch's Silent mode keeps a pending decision on the connection). With "When two tunnels answer: Ask me", the question today lives only in a notification (with "Keep it"). Under the routed row's reason, a line in craked's words, **"Nordbank answers for it too."**, with **Use Nordbank** (small action) and **Keep ACME** (muted dismiss); once either is chosen, the line goes. App: new work (the entry exists as a System entry; the panel leaves those out).

## Dropped

- **A map of connections** (Little Snitch, Mullvad, Proton). DESIGN.md: "The internet is never drawn."
- **State told by colour** (Mullvad's green and red padlock, Windows' accent shield, per-device colours in Resources and Mission Center). The bird changes shape, never colour; tunnels are told apart by name.
- **Marks that need a legend** (Mullvad's yellow and red marks, Little Snitch's blue and orange buttons, Tailscale's dot-grid icon, Fluent's icon buttons in toasts). They fail "reads on sight"; buttons name the action.
- **Blinking, flashing or a shimmer as status** (Little Snitch's flashing rows and icon, Mission Center's loading shimmer, a pulsing "trying again"). The rejected breathing dot in another form; status is said in words, and the dashed bird already says "under way".
- **Counters and charts as reassurance** (Proton's NetShield counts, a traffic chart in the menu bar, Flighty Passport's lifetime stats). Numbers that answer no question: the generic dashboard.
- **A modal alert that holds the connection** (Little Snitch, LuLu). Crake holds a SYN for one round trip at most; an alert you must answer makes it an app you live in.
- **A three-pane monitor** (Little Snitch's Network Monitor). The rejected tern shape.
- **Per-app routing** (Mullvad, Proton). Crake routes per host; a different question and new work in every OS layer.
- **"Expires in" on a hand-added host** (LuLu). A second decision in a row that has one; written down for later, not drawn.
- **"Today / Earlier" headings in Learned** (LuLu's Recent filter). The rows already carry their time; on a normal day the heading splits nothing.
- **Header capitals, a bottom toast in the panel, an illustrated empty state, a summary notification ("3 hosts routed")** (the GNOME HIG). Sentence case is Crake's voice; the panel is too small for a toast; the first run already has the bird rising from the grass; GNOME 48 stacks one notification per decision anyway.
- **Teenage Engineering's hardware look and lowercase, Flighty's split-flap board, Panic's jokes in the product, Things' "pop" on every act, Linear's themes, confetti for the first route.** Skeuomorphic hardware, imperfection and motion for their own sake, a second accent, marketing: all on DESIGN.md's rejected list.
- **Proof of life ("Last heard from the gateway 6 s ago", WireGuard).** Reads well, but openconnect's keepalive timing doesn't reach craked today; a claim the app can't make. Written down for later.
- **A name sharing a routed address, shown per row** ("learned with git.acme.internal, same address", Cloudflare). craked doesn't keep the names per address; gap 10 says it in words instead.

## Platform notes for docs/engineering/

1. **GNOME shows no banner for a Low-urgency notification and ignores an app's timeout** (gnome-shell `messageTray.js`, `notificationDaemon.js`): Low goes only to the list under the clock, Normal and High get a 4 s banner, Critical stays until dismissed. Crake sends a routed host at Low with 8 s (`notify::routed`), so on GNOME "Quiet notification" shows nothing on screen. **Needs your call:** send Normal (a 4 s banner), or keep Low and say so under the option ("It waits under the clock; nothing pops up"). A tunnel that stopped by itself is the one case for Critical.
2. **The AppIndicator extension (v53–v66), by its code, opens the native menu on a single click (after the double-click delay) and calls `Activate(x, y)` on a double-click.** CLAUDE.md says the panel opens at "the click". To confirm on the user's 26.04 notebook and write down which gesture opens the panel; the extension never shows tooltips, which is why gap 12 matters.
3. **Dash to Panel can put a bar on the left or right edge, and one on every monitor.** `place.rs` opens the panel upward from a bottom bar and downward from a top bar; a side bar is a case to test.
4. **Windows: toasts are credited to "Windows PowerShell" when no AppUserModelID is set** (notify-rust's fallback; `notify.rs` sets none), a `default` action becomes a visible "Open Crake" button beside Undo, and "until dismissed" is only Short. Needs the AUMID, Undo as the only button, and the Reminder scenario (with "Switch on") for a stopped tunnel. To check in the Windows VM.
5. **Windows: a new tray icon starts under the ^ overflow** and only the user can bring it out; Windows' VPN quick setting and shield cover only Windows VPN profiles, so Crake's Wintun tunnels likely show in neither (to verify in the VM). A first-run note for Windows ("The bird is under the ^ by the clock. Drag it next to the clock to keep it in sight.") is new work, not drawn here.
6. **Windows 11 rounds flyouts at 8 px;** if DWM draws the panel's corner there, it's 8, not DESIGN.md's 10.
7. **Quick Settings can't host a third-party toggle without a Shell extension;** GNOME's VPN toggle lists only NetworkManager VPN and WireGuard connections. A GNOME Shell extension stays "not now" (CLAUDE.md).
8. **The Mac: a status item can sit hidden behind the notch** (Tailscale reads its `occlusionState` and says so in a window). Crake can't open its panel from a hidden bird; how it would say so is a Mac decision, written down, not drawn.
