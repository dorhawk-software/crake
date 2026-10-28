# Tried by real people (a checklist, ticked one by one)

Moved here from CLAUDE.md on 2026-10-21 (the user's rule on keeping CLAUDE.md small: detail lives in the documentation it belongs to; CLAUDE.md keeps the rule and a pointer). Nothing was dropped in the move; add to this file from now on.

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
  Found 2026-10-26 (a colleague, Windows 11 with Norton): Norton kept the
  unsigned craked.exe from starting (the System log: "The Crake service
  failed to start: Access is denied", a second after the install), so
  there was no craked.log; New tunnel's detection worked (the app runs
  openconnect itself) and the tunnel then never came. With Crake's install
  folder and ProgramData\Crake excepted in Norton, it connected. Open:
  signing (SignPath). Built after it: the panel says why the service isn't
  running (its newest event in the System log, by number and error code:
  `service::stopped`), and New tunnel stays open and says so when craked
  isn't there (it had closed on a request nobody answered). Not now:
  craked writing to the Event Log when it can't open craked.log.
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
- [ ] Windows 11 for ARM (the ARM64 installer; the user's coming Mac,
  in a VM): installing, the drivers, a tunnel up.
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
- [x] The service approved by the user, craked running under launchd from
  Crake.app, and running again after an upgrade (2026-10-27, the user,
  0.15.4 → 0.16.0 by `brew upgrade`: craked's log says the tunnel switched off at
  11:32:09 UTC as Homebrew stopped it, the new craked was up 23 s later
  with no "isn't running", and it was on again at 11:33:06).
- [ ] An upgrade keeping the tunnels up: 0.16.0's change, so first the
  upgrade from 0.16.0 (Homebrew runs the installed version's steps; the
  one above, from 0.15.4, still switched them off, as expected).
- History of the service item (0.7; also in Not now). 2026-10-26, the
  user, 0.15.1 → 0.15.2 by `brew upgrade`:
  approved and running before; after it craked wasn't running at all
  ("Crake's service isn't running.") and the tunnels were off. Homebrew's
  cask takes the daemon out of launchd (`launchctl remove`) on upgrades
  too, macOS still said "enabled", and nothing registered it again; the
  user switched it off and on in System Settings. Since then the app
  registers it again (`service::revive`); open until an upgrade shows
  that works.
- [ ] The panel under the menu bar, placed at the icon on each display.
- [ ] The Traffic window: connections listed with their ways and rates.
- [x] Out of the Dock but while a window of Crake's own is open
  (2026-10-27, the user, 0.16.0 from Homebrew): in the Dock with Settings
  or the log open, gone once they closed (`windows/dock.rs`; until then
  it sat in the Dock all the time).
- [ ] The bird dimmed with its neighbours on a display not in use: a
  template image since 2026-10-28 (`tray::template`), the plain bird
  only; with a badge or the call it stays coloured (the user saw it
  bright there, 2026-10-28).
- [x] The bird crisp on a display that isn't Retina (2026-10-27, the
  user, 0.16.0): the 1x drawing macOS picks there (`tray::Icon::mac`;
  until then it halved the 2x one into a blur).

**Everywhere:**
- [ ] Importing a real .ovpn file (New tunnel's Import .ovpn…).
- [x] OpenVPN run by Crake itself against a real gateway (2026-10-19,
  the user, on 0.11.0: browser sign-in, the company's internal network
  reached).
- [x] Split-horizon with a public address only the tunnel reaches
  (2026-10-26, the user, macOS, AnyConnect, a development build): a
  company site asked for a password through Crake (its public address,
  for outsiders) and opened through the company's own client; a colleague
  found it. Crake took only the tunnel DNS's private addresses then. Now
  any address the tunnel's DNS gives that public DNS didn't counts when
  only the tunnel reaches it (`naming::route_own`); the site opened.
- [ ] Each Beta protocol against a real gateway (only AnyConnect and
  OpenVPN 3 have met one; the Beta tag comes off as each is ticked).
