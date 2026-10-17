# Changelog

What's new in each Crake release, for the people using it. The newest is at the top.

<!-- How this file works (scripts/release-notes.sh, .github/workflows/release.yml):
     changes worth telling users go under "Unreleased" as they're made. To
     release, that heading becomes "## <version> (<YYYY-MM-DD>)", the version
     goes into Cargo.toml, packaging/nix/package.nix and packaging/arch/PKGBUILD,
     and the tag v<version> is pushed: the release workflow refuses a tag whose
     section is missing, and that section is the GitHub release's text. -->

## Unreleased

- **No more large pointer for a moment.** Coming onto a Crake window, the pointer could blink
  large for an instant: GNOME had given XWayland's background a pointer at an older scale's size,
  which shows before the window's own. Crake now gives it your theme's pointer at today's size,
  and keeps it so when the scale or the pointer size changes.

## 0.10.3 (2026-10-17)

- **The panel stays above the bar.** With long names in "What Crake did" (an Azure cluster's
  private-link name, say), the panel opened from a bottom bar could grow down under it, hiding the
  last tunnel and the buttons. It grows upward again, and long names break after a dot instead of
  in the middle of a word, in the panel and the log.

- **The pointer is its right size from the start.** Over Crake's windows the arrow could be too
  big until the pointer had been over a button once. Each window now has its own arrow from the
  moment it opens.

## 0.10.2 (2026-10-17)

- **"Keep internal names private" says when it can't.** If another DNS server on your computer
  holds the port Crake answers on, or Crake can't add its private DNS link, Settings → Names now
  says so under the setting, and Crake keeps trying. Before, it failed quietly, and with such a
  server running the names could reach it and go out to your provider's DNS.

## 0.10.1 (2026-10-17)

- **The pointer keeps its size.** After the display's scale changed while Crake ran (a laptop
  docked to a large monitor, or undocked), pointing at a button or a field in Crake's windows
  showed a pointer twice (or half) its size. It follows the scale now, and the cursor size set in
  Accessibility too.

- **Traffic's tick stays on a host you pinned.** A host Crake sent through a tunnel lost its
  two-stroke tick in Traffic once you pinned it; it keeps it now. Hosts you added yourself have none.

- **Traffic's ticks have room.** The line to a destination Crake learned stopped under its
  two-stroke tick, and packets ran beneath it; it now ends before the tick.

- **The bird in the bar is smooth again at its smallest.** 0.10.0 drew it pixel by pixel at 16 px,
  and next to the bar's other icons it looked pixelated; it's drawn with soft edges, as at every
  other size, with the cut of its wing back.

- **The app icon is the bird in one ink.** The corncrake's colours are gone from the icon: it's
  the same light bird as in the panel and the bar, calling in teal, on the dark tile.

## 0.10.0 (2026-10-17)

- **Sign-in asks in words.** "ACME asks for your group", "…for a code", "…for your username",
  with the gateway's own words quoted under it, instead of "ACME asks: GROUP". A field Crake
  doesn't know keeps the gateway's word.

- **Small things in the panel and New tunnel.** A notice's "Got it" is the same bordered button
  everywhere; the dot next to "Trouble" is red, as the bird's badge is; "Found a Cisco AnyConnect
  gateway" wears a plain check (the two-stroke tick is for hosts Crake routed); the note under
  "Stay signed in" about your browser is gone (the sign-in card says it); a number and its unit
  ("38 ms") no longer part at a line's end; the Traffic window's margins match the other windows'.
  The amber of "needs you" is a touch deeper in the light theme, so it reads against the panel.

- **Tunnel detail reads as its screen.** Pin and Forget are quiet bordered buttons again (they had
  become bare bold words), "last used" lines up on the right, a pinned host shows nothing there
  (its line says it's kept), and the title row says "Switch off", the words used everywhere else.

- **The log's empty day says it once.** "Nothing to tell today. No host needed a decision." under
  the day's chart, instead of a summary of zeros and "Nothing matches today"; a host left alone has
  no badge any more; the "Scroll to zoom · drag to move" hint shows only while you point at the
  chart.

- **Settings is quieter.** "Remove…" and "Clear log…" are plain buttons until you point at them
  (red is for something that stopped, or for the question "Remove ACME?" itself); the "?" marks
  stay only where their card says more than the line under the setting; the row that only means
  something when Crake starts at login is dimmed less, so it can still be read; and Privacy's
  statement sits on a plain surface.

- **Traffic shows its decisions, not its tunnels, in teal.** The packets moving through the graph
  are grey on every way now; the two-stroke tick marks each destination Crake learned, in the
  graph and in the table's "Way" column (it was "Goes"), the same tick as under the chart. Long
  names aren't cut short any more.

- **A new bird.** The one in the bar was a plain head; now it's a crake with its head raised,
  calling: a short bill parted at the tip, a square eye, the folded wing one cut across the
  breast. It opens its bill when it calls (a host sent through a tunnel), and drops its head to
  sleep. The same bird is in the panel, in Settings and on the app icon, where it wears the
  corncrake's own colours.

- **Crake says so when your desktop shows no tray icons.** Stock GNOME on Fedora and Debian
  shows none without its AppIndicator extension, and Crake used to start with nothing on screen.
  Now its panel opens with a note when you start it, and at login it sends one notification. Where
  it can, Crake switches the extension on in one click, or gives you the command to install it.
  Settings' bird row says the same.

- **Settings no longer says the bird is in the "top bar".** It's "the tray" on Linux and Windows
  and "the menu bar" on a Mac, with the bird itself next to the words, drawn the way that bar shows
  it. Its "?" now says what each look of the bird means: asleep, dashed, awake, two teal marks, an
  amber triangle, a red badge.

## 0.9.0 (2026-10-16)

- **A host that didn't answer once is no longer called "probably down".** The log now says what
  happened: "No answer just then, directly or through ACME. Nothing was changed."

- **Crake can be used from the keyboard.** Tab and Shift+Tab move through the panel, tunnel detail,
  Settings, the log, Traffic and the editor; Enter or Space presses what's in focus, and a ring
  shows where you are.

- **Public sites an older Crake sent through a company by mistake go direct again.** Before 0.8.0,
  one lost connection on a shaky network could send a public site through a tunnel for good (it's
  kept while it's used). Once after an update, Crake tries each such site directly and forgets the
  ones that answer, and says so in the log. Linux for now.

- **Switching an OpenVPN 3 tunnel off no longer says it "stopped unexpectedly".** Crake ended the
  session before telling its service, which sometimes noticed the tunnel's device gone first and
  called it ended by OpenVPN 3. The service is told first now, and a device missing for a moment
  isn't taken for a stop.

- **An edited OpenVPN 3 profile is never thrown away by one key.** With changes not saved, Escape,
  the × or Cancel ask first ("Leave without saving?"), and Keep editing is the default.
- **New tunnel gets out of the way once the gateway has answered.** The protocol list folds into
  "Choose the protocol yourself", so the next field is in view; opened, the protocols sit under
  "Used with real gateways" and "Beta" instead of a badge on each.
- **Traffic's numbers agree with their order.** The busiest places and connections show the rate
  they're ordered by (the last twenty seconds or so), so a row never shows more than the one above.
- **Traffic keeps teal for Crake's decisions.** The last five minutes are drawn in greys, with a
  teal tick where Crake sent a host through a tunnel; every way out has the same grey outline.
- **Quieter hosts in tunnel detail.** Pin and Forget no longer look like a wall of buttons; they fill
  in when pointed at, and a pinned host counts once, under Pinned.
- **The panel's bird says what the bird in the bar says**, with its amber triangle or red badge, and
  the leftover OpenVPN 3 notice has a Copy button inside the command, a Got it button, and says it
  goes away by itself.

- **A process OpenVPN 3 left running is shown in the panel, with the command that stops it.** The
  notice is tinted amber and has Copy and Got it beside the command; it goes by itself once the
  process has ended. The bird in the bar wears a small amber triangle meanwhile (a red badge, for a
  tunnel that stopped, takes its place). Crake doesn't end another program's process itself.

## 0.8.3 (2026-10-15)

- **Traffic names things better and stays still when nothing flows.** Many names can share one
  address (a company's DNS often gives every name under a domain the same one); a connection now
  keeps the name it was first seen with instead of the latest one looked up. A tunnel carrying only
  its keepalives no longer shows packets moving while it says 0 KB/s.

## 0.8.2 (2026-10-15)

- **"Crake was updated" now appears after a package update.** The app kept running against the new
  service without noticing it had changed, so the panel never said to restart. Now it notices within
  a couple of seconds and offers Restart.
- **Firefox no longer hangs when another OpenVPN 3 session reconnects.** OpenVPN 3 sends every name
  to a tunnel's company for a moment whenever any of its sessions reconnects; Crake used to put
  things back within two seconds, long enough for Firefox's proxy auto-detect to ask in that moment
  and hang. Now it's put back at once.
- **Crake tells you when OpenVPN 3 leaves a connection running.** OpenVPN 3 can lose track of a
  session's process: the session ends, the process doesn't, and it keeps reconnecting on its own
  for days, changing your other tunnels' DNS each time. When Crake sees that happen to one of your
  sessions, it says so in the log and a notification, with the command that stops it.
- **Crake no longer leaves an OpenVPN 3 session it can't follow.** A session Crake started is ended
  if anything goes wrong before it's connected, and Crake doesn't tell a session to connect when
  something else already has (two at once were how OpenVPN 3 lost those processes).
- **Traffic is calmer.** The rates follow the traffic over a few seconds instead of jumping every
  second, the busiest places and the connections keep their places unless the traffic really
  shifts, and packets are drawn above every path instead of disappearing under the one crossing it.

## 0.8.1 (2026-10-15)

- **OpenVPN 3 tunnels work again.** 0.8.0 lost an OpenVPN 3 tunnel's routing a second after it came
  up and left it "connecting": a sign-in through the browser looked stuck. Fixed: while the
  session's device is there, the tunnel stays up, and a session that reconnects is handed over
  again.
- **An update keeps an OpenVPN 3 tunnel's names.** After an update, an OpenVPN 3 tunnel could come
  back with none of its domains, and OpenVPN 3 could then send every name on the computer to the
  company's DNS (Firefox hung looking for a proxy). The tunnel's DNS and domains now go along with
  the handover.

## 0.8.0 (2026-10-15)

- **Traffic, live.** A new window, from the panel's Traffic button, shows what goes where right
  now: this computer, each tunnel that's up and direct, the busiest places each one reaches, and
  packets ticking along every path as they go. Under it, the last five minutes, the connections
  open right now and every rule Crake has. On Windows only TCP connections are listed (Windows
  doesn't say where UDP goes); a connection's bytes count from when the window first looked. It's
  only asked for while the window is open, and stays on this computer like everything Crake sees.
- **Edit an OpenVPN 3 profile in Crake.** An OpenVPN 3 tunnel's details have "Edit…" next to its
  profile: its text opens in a window, certificates and keys folded away and kept as they are.
  "Save and reconnect" gives OpenVPN 3 the new text and reconnects the tunnel; if OpenVPN 3
  refuses it, it says why and nothing changes. For people who tune their own profiles.
- **"Is it safe?"** Settings → About answers it in four lines: nothing leaves your computer, what
  happens if Crake stops, what your company sees, and who can change what.
- **Hover.** A window's × turns red while you point at it, and Settings' sections light up softly
  under the pointer.
- **Whole names.** A long host name cut short with "…" in a tunnel's details or the Traffic window
  shows whole when you point at it, and a click copies it.
- **Dismiss a stopped tunnel.** A tunnel that stopped on its own has a Dismiss button next to why in
  the panel: it goes back to plain "Off" and the red badge leaves the bird. Why it stopped is still
  in its details and the log.
- **The bird says what's up.** With a tunnel stopped and nothing else connected, the bird sleeps,
  with the red badge; it no longer looks awake as if something were connected. While another tunnel
  connects, it's drawn dashed with the badge.
- **No company detours for public sites.** One connection to a public site that got no answer (a
  lost packet on Wi-Fi), or a minute when the network itself had trouble, could send that site
  through a company's tunnel for a week, and break it when that tunnel's session ended. Crake now
  tries a public site directly once more before sending it through a tunnel, and leaves public
  sites alone while several places stop answering at once.
- **OpenVPN 3 profiles found the first time.** Switching on an OpenVPN 3 tunnel sometimes said its
  profile wasn't there, and "Try again" worked: openvpn3 had only just started. Crake now gives it
  a moment.
- **OpenVPN 3 tunnels survive a reconnect.** When openvpn3 sets a session up again (the network
  dropped for a moment, the computer woke up), Crake says the tunnel is reconnecting and routes
  through it again once it's back. It used to call the tunnel stopped while the session carried on.

## 0.7.0 (2026-10-14)

- **Crake says when it was updated and needs a restart:** after a package update the panel says
  "Crake was updated to X. Restart it to finish.", and Restart does it with your tunnels kept up.
- **Crake tells you when a new version is out,** where nothing else updates it (a Mac without
  Homebrew, Windows, a .deb or .rpm installed from a file): once a day it asks dorhawk.software for
  the newest version's number and the panel links its download page. Settings → General switches
  it off.
- **The panel no longer blinks and jumps when it changes size,** e.g. after Undo or Redo with a
  bar at the bottom of the screen: it grows and shrinks in place instead of opening again.

- **The sign-in window goes behind other windows** when you click elsewhere, like any window. It
  stayed above everything, even over the browser page of a web sign-in.

- **Forget all in a tunnel's detail.** One button empties what Crake learned for a tunnel, and
  what you sent direct with Undo, so it learns again from scratch as you use things. It asks first;
  pinned hosts stay.

- **On a Mac, System Settings lists Crake by its name.** Its service and "Start when you log in"
  are now registered by Crake itself, so Login Items & Extensions shows them as Crake, not under
  the name on its developer certificate. macOS asks you to switch the service on once: Crake opens
  after installing, and its panel says so, with a button to the right page. Nothing is routed
  until then.

## 0.6.1 (2026-10-14)

- **Crake no longer gets in the way of another VPN client signed in to the same company.** With
  "Keep internal names private" on, Crake refused a company's names while its own tunnel was off,
  even when AnyConnect or openconnect was connected there: every internal name failed (seen on a
  Mac). Names another VPN client serves are now left to it, and kept private again once it
  disconnects. On a Mac, Crake's service also no longer stops when macOS briefly refuses it its
  network settings.

- **Crake keeps a tunnel's DNS to its own domains, even when another program changes it.**
  openvpn3, when one of its sessions reconnects, set another OpenVPN 3 tunnel's DNS back to its
  own: every lookup, public ones included, went to that company's DNS, and short names like
  "wpad" turned into the company's (a browser auto-detecting its proxy then hung). Crake now
  notices within seconds and puts its own settings back.
- **OpenVPN 3 profiles whose sessions don't name their device can be used.** For some profiles
  (seen with an Azure VPN gateway's) openvpn3 never says which device a session uses; Crake now
  finds it from the process running the session, where it said "didn't say which device the
  session uses". If Crake still can't take a session over, it ends it rather than leave it up
  unrouted.

## 0.6.0 (2026-10-14)

- **Crake keeps its size when a monitor with another scale is plugged in.** On Linux, a laptop
  at 100% that got a HiDPI monitor showed Crake's windows at half size until Crake was started
  again, and the panel opened away from the tray icon. Crake now follows the desktop's scale as
  it changes, and the panel opens at the icon at any scale.
- **An OpenVPN 3 tunnel whose session ended behind Crake's back shows as down.** When openvpn3
  ended a session while Crake's window wasn't running (it does when the app that started it
  quits), the tunnel stayed "Connected · its DNS doesn't answer" for good. Crake now notices
  within seconds that the session's device is gone and says the tunnel stopped.
- **Signing in to OpenVPN 3 again no longer ends in "isn't a VPN device".** openvpn3 can say a
  session is connected a moment before it names its device; Crake now waits for the name instead
  of giving up and ending the session. If Crake can't use a session, the log says why.
- **A tunnel that stopped says why**, when Crake knows: "Session expired", "Ended by the gateway",
  "Sign-in refused" or "Ended by OpenVPN 3" instead of "Couldn't stay connected" in the panel,
  and the whole sentence in the tunnel's detail.
- **The tray shows that Crake is connecting while you sign in again** to a tunnel that stopped,
  instead of keeping its red "!" until the sign-in is done.

## 0.5.0 (2026-10-13)

- **The panel says when a browser looks names up itself.** Firefox with DNS over HTTPS on, or
  Chrome, Edge and Brave with Secure DNS set to a provider of their own, ask their own DNS, so
  Crake can't send a company's names through the tunnel before they connect: the first visit
  there waits a few seconds. A notice under the tunnel says so, and what fixes it, until "Got it".
- **"Copy for helpdesk" says what Crake sees on this computer:** each tunnel's device, address and
  DNS, who answers the computer's DNS, and which browsers look names up themselves (DNS over
  HTTPS, where Crake can't route a company's names before the first packet). A helpdesk no
  longer has to ask you to look it up (with host names hidden, the addresses are left out too).
- **"Start when you log in" set up by an older Crake is brought up to date** when Crake starts, so
  on macOS it too shows as Crake, with its icon, in System Settings' background items.
- **macOS and Windows: when the network's DNS stops answering, the internet keeps working.** While
  a tunnel is up, Crake answers the computer's DNS and asks the network's servers in its stead. If
  they stop answering (a hotel's sign-in page, a network that changed), Crake now gives the
  computer its own DNS back, says so, and tries again two minutes later.
- **Undo and Redo change the chart's tick, and only that.** Undo takes one off its count, or turns
  it grey when nothing's left; Redo puts it back. Undoing and redoing used to add a decision each
  time, as if Crake had routed the host again.

## 0.4.0 (2026-10-13)

- **macOS: a tunnel can no longer take the whole internet down.** While a tunnel is up, Crake
  answers the Mac's DNS so it can find names only a company knows. On a Mac whose DNS servers it
  couldn't read, every name failed; it now leaves such a Mac's DNS alone.
- **macOS: Crake's windows open in front.** Settings, New tunnel, the log and a tunnel's details
  opened behind the app you were in.
- **macOS: the Mac's own window buttons.** Close, minimise and zoom sit top left as in every Mac
  app, instead of Crake's header being drawn under them; Crake's own close button is gone there.
- **macOS: Crake shows as Crake in System Settings' background items,** with its icon, not under
  the developer's name.
- **macOS on Intel Macs too.** The installer and the Homebrew cask now carry Crake for both Apple
  Silicon and Intel.
- **Works with encrypted DNS (DNS over TLS).** A tunnel's own DNS is asked plainly, as it must be,
  even when the computer encrypts the rest; the log says once that it's on.
- **Settings → About has a "Buy me a coffee".** Crake is free and stays free; a coffee is a
  thank-you and unlocks nothing.

## 0.3.0 (2026-10-13)

- **OpenVPN 3: a second session for the same tunnel no longer stops both.** When a new session turns
  up (openvpn3's indicator reconnecting, say) it takes over from the old one, and the old one
  ending can't take the new one down.
- **"Start when you log in" works on macOS and Windows** (it did nothing there): a LaunchAgent on
  macOS, the user's Run key on Windows, as each system expects.
- **A tunnel that stops by itself says so, and why.** A notification tells you, with **Switch on**
  to sign in again. When an OpenVPN 3 server ends the session (many do after an hour or so), the
  log and the notification say the session expired and after how long, instead of only "the
  session is gone".

## 0.2.0 (2026-10-13)

- **Your first tunnel is easy to find.** With no tunnel yet, the panel says so and offers
  **Add a tunnel…**, while the bird peeks out of the grass now and then, looking for one. The
  tray's menu has **New tunnel…** too, on every system.
- **New tunnel shows it's still asking the gateway:** the dashed head turns, and after a moment
  the line says for how long. A slow gateway no longer looks like nothing is happening.
- **Tab** goes from New tunnel's name to its gateway, and Shift+Tab back.
- **packages.dorhawk.software** has a start page with the commands to add it. On a system with
  32-bit packages switched on, apt noted that the repository has none; fetch the sources file
  again to quiet it:
  `curl -fsSL https://packages.dorhawk.software/apt/dorhawk.sources | sudo tee /etc/apt/sources.list.d/dorhawk.sources >/dev/null`

## 0.1.0 (2026-10-13)

The first release. Crake connects to the VPNs you already have, and sends nothing through them
until a host needs it.

### What it does

- **Learns from DNS.** A name only the company's DNS knows is routed before the first packet.
  Names public DNS has never heard of are asked of your tunnels too.
- **Learns from silence.** A host that doesn't answer directly is tried through each tunnel. The
  one that answers gets it, and the log says so in a sentence. On Linux, apps that talk UDP are
  covered too.
- **Undo, always.** Every decision can be undone where it's shown. Sent direct, a host stays direct
  until you say otherwise.
- **Several tunnels at once,** in the order you set. When two answer for the same host, Crake asks
  you, takes the faster, or keeps to the order. Tunnels that can't be up together are refused
  with the reason, before they half-work.
- **Rules from IT.** A managed profile says what a tunnel may learn, or asks for a full tunnel.
  Crake shows it in plain words, and nobody can widen it.
- **Public name, private address.** When a company serves its own staff a different address for a
  public name, Crake asks the company's DNS and routes the private one.
- **Internal names stay internal.** With its tunnel off, a company's names fail at once instead of
  going to public DNS.
- **Networks that block UDP:** noticed, the tunnel runs over TCP on the same session, and the
  network is remembered for next time.
- **A tunnel's DNS that stops answering:** Crake says so, and names it answered before keep
  working.
- **Gateways that only let Cisco's client in:** Crake introduces itself as AnyConnect, and tells
  you it did.
- **IPv6** alongside IPv4: learned, routed, and refused rather than sent around a full tunnel
  that has none.
- **OpenVPN 3 profiles,** including sessions started elsewhere, and importing a `.ovpn` file.
- **New tunnel** finds out what the gateway speaks and tells you when its certificate runs out.
- **Passwords and certificates** live in your system's keyring, never in Crake's files.
- **Copy for helpdesk:** the day in plain text, with host names left out if you like.

### Protocols

Cisco AnyConnect (and ocserv) and OpenVPN 3 have been used against real gateways. GlobalProtect,
Ivanti Connect Secure (Pulse), Juniper, Fortinet, F5 and Array are spoken through openconnect and
marked Beta in the app until each has been.

### Where it runs

- **Ubuntu** 26.04, 24.04 and 22.04 and **Debian** 13 and 12: one `.deb`, from Crake's own apt
  repository at packages.dorhawk.software (see the README). On Ubuntu 22.04, sign in with a
  password or a certificate: its openconnect is too old for browser sign-in.
- **Fedora** 45, 44 and 43: an RPM, from the same repository with dnf.
- **Arch Linux:** a PKGBUILD for the AUR.
- **NixOS:** a flake with the package and a module (`services.crake.enable`).
- **Windows 11, preview:** an installer with openconnect included. It isn't code-signed yet, so
  Windows warns before installing it. The first connection to a host Crake has just learned fails
  once; the app's retry goes through.
- **macOS on Apple Silicon, preview:** a signed and notarized installer, also as a Homebrew cask
  (`brew install --cask dorhawk-software/tap/crake`, which brings openconnect along). It hasn't
  been tried on a real Mac yet.
