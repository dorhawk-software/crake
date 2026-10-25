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

- Append-only JSON lines in craked's state directory (`crake_proto::LOG_PATH`:
  `/var/lib/crake/log` on Linux), which craked closes to itself at start
  (`config::own_state_dir`; until 2026-10-21 every user could read it). Reading
  craked's API is guarded like changing it, and its signals go only to
  callers let in (`dbus/guard.rs`). Retention
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

## A newer Crake

`versions.rs`: "Crake was updated to X. Restart it to finish." when a
package updated craked (Restart starts the new binary), "Crake X is out."
where nothing else updates Crake (once a day, versions.json; Settings →
General switches it off), then What's new once updated (`whats_new/`).
Detail: **docs/engineering/releases.md**.

## Not now (written down to come back to)

**The rule (the user, 2026-10-14): whatever we decide not to do right now
is written down, in docs/engineering/not-now.md or next to the code it
concerns, in the same commit.** A doc comment on a stub isn't enough;
prefer code that fails loudly over a silent stub. Done items leave the
list. Each item's history and detail: **docs/engineering/not-now.md**; in
one line each:

- Zed upstream, GPUI change 3 (Xft.dpi at runtime): the branch is ready;
  the user files the issue and PR in their own words (Zed's AI policy).
- Two sign-in tabs, and OpenVPN 3 sessions stopped before sign-in: likely
  openvpn3-indicator; to watch one sign-in live with the user.
- openvpn3 leaves a session's process running: detected and shown (built);
  not a process left from before the app started.
- macOS: craked registered by the app (approval, launchd, an upgrade's
  restart): tried on a real Mac only.
- Unsafe code behind one safe layer (`crake-os`): done; not yet unit tests
  of its Windows and macOS parts on their runners.
- Small screens: partly fixed; open: hundreds of hosts in tunnel detail,
  resizable windows, the work area, tall windows (scrollbars: done).
- The sign-in window once drawn taller than it was, after a restart: not
  reproduced; what to capture next time.
- Display scale changes on Windows and macOS: not tried.
- winget for Windows; Undo in macOS notifications.
- macOS colouring the menu bar's bird itself (a template image): waits for
  the menu bar's own appearance and a real Mac to try it.
- No-tray's one-line command: no scrollbar (it would cover the text).
- OpenVPN 3 sessions whose device openvpn3 doesn't name, under the package.
- A shared practices repository for dorhawk.software (the user's idea, once
  Crake is stable).
- IPv6-only hosts on a network without IPv6: decided, left as it is.
- Windows on ARM64: built; not yet a real Windows on ARM.
- Browser sign-in on Windows (openconnect without `--external-browser`).
- Performance: never measured; measure before claiming numbers.
- A colleague's requests (Traffic, the split as a whole, "Is it safe?",
  the profile editor): built; the Traffic window's open ends are listed.
- The explainers (docs/explainers.md); the website's version number.
- Untried on real systems: the checklist below.

## Tried by real people (a checklist, ticked one by one)

Asked for by the user, 2026-10-14: what someone has used Crake for on a
real machine (not a VM made for tests), each item ticked with the date and
who (a role, not a name), and what was found and fixed:
**docs/engineering/tried-by-people.md** (tick items there). Windows, the
other Linux desktops and most of macOS haven't met a real person yet;
Ubuntu 26.04 with GNOME and a bottom bar is the user's own notebook.

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
voice (not generated from commits): **every user-facing change adds a line
under "Unreleased" in the same commit**, dependency updates too (what was
updated and what it means for the people using Crake; a security fix in a
dependency warrants at least a hotfix release). Each change goes under its
kind: `### New`, `Better`, `Changed`, `Fixed`, `Underneath` (what Crake is
built on; a dependency's security fix is Fixed), the headings the app's
What's new shows (`whats_new/`, the changelog built in; a test reads it
whole). To release: `scripts/release.sh <version>` (`--dry-run` first): it
refuses an empty "Unreleased" or a change outside its kinds, dates it,
sets the version everywhere, commits and pushes, waits for CI and pushes
the tag; run it again with the same version after an interruption. Don't
push anything else to main while it waits. The release workflow, the
Homebrew tap, the apt/dnf repository (dorhawk-software/packages) and the
AUR: **docs/engineering/releases.md**.

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

Keep the split that makes them possible: decisions (host store, expiry,
policy, log, probing logic) stay portable; everything that touches the OS
stays behind small modules (`net`, `routing`, DNS, tray). The notes per
system: **docs/engineering/platforms.md**, "Someday: other platforms".
