# Changelog

What's new in each Crake release, for the people using it. The newest is at the top.

<!-- How this file works (scripts/release-notes.sh, .github/workflows/release.yml):
     changes worth telling users go under "Unreleased" as they're made. To
     release, that heading becomes "## <version> (<YYYY-MM-DD>)", the version
     goes into Cargo.toml, packaging/nix/package.nix and packaging/arch/PKGBUILD,
     and the tag v<version> is pushed: the release workflow refuses a tag whose
     section is missing, and that section is the GitHub release's text. -->

## Unreleased

- **Works with encrypted DNS (DNS over TLS).** A tunnel's own DNS is asked plainly, as it must be,
  even when the computer encrypts the rest; the log says once that it's on.

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
