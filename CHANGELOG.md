# Changelog

What's new in each Crake release, for the people using it. The newest is at the top.

<!-- How this file works (scripts/release-notes.sh, .github/workflows/release.yml):
     changes worth telling users go under "Unreleased" as they're made. To
     release, that heading becomes "## <version> (<YYYY-MM-DD>)", the version
     goes into Cargo.toml, packaging/nix/package.nix and packaging/arch/PKGBUILD,
     and the tag v<version> is pushed: the release workflow refuses a tag whose
     section is missing, and that section is the GitHub release's text. -->

## Unreleased

- **Traffic, live.** A new window, from the panel's Traffic button, shows what goes where right
  now: this computer, each tunnel that's up and direct, the busiest places each one reaches, and
  packets ticking along every path as they go. Under it, the last five minutes, the connections
  open right now (on Linux) and every rule Crake has. It's only asked for while the window is open,
  and stays on this computer like everything Crake sees.
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
