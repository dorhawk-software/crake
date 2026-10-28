# Not now (written down to come back to)

Moved here from CLAUDE.md on 2026-10-21 (the user's rule on keeping CLAUDE.md small: detail lives in the documentation it belongs to; CLAUDE.md keeps the rule and a pointer). Nothing was dropped in the move; add to this file from now on.

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
- **macOS: what CI can't check of craked registered by the app.** The
  user's approval (no runner can click it), so craked under launchd from
  Crake.app, and an upgrade's restart of a registered craked
  (`launchctl kickstart -k`, the tunnels handed over) are tried on a real
  Mac only: the colleague's, first with 0.7. Homebrew's cask upgrade
  (found by the user, 2026-10-26) never gets that far: the cask's
  `uninstall launchctl:` runs on every upgrade (Homebrew skips only
  `signal`; flight blocks aren't told it's an upgrade), so craked stops
  without the handover (tunnels logged off) and isn't loaded again;
  macOS still says "enabled", and `launchctl bootstrap` of the app's
  launchd file fails (5: Input/output error; `BundleProgram` is
  SMAppService's to load). The app now registers it again once it stays
  out of reach (`service::revive`, then unregister and register); it did
  on the user's 0.15.4 → 0.16.0 upgrade (2026-10-27: up again in 23 s). The
  tunnels kept across a Homebrew upgrade (decided by the user, 2026-10-27):
  the cask's `uninstall_preflight_steps` leaves the handover flag before
  `launchctl` stops craked, on every uninstall, since Homebrew doesn't tell
  a cask it's upgrading; a real `brew uninstall` then leaves the sessions
  to time out on the gateways instead of logging them off (routes, DNS
  and pf rules still go). Telling the two apart (the brew process's own
  command line) was offered and declined as too fragile. A cask change
  reaches only upgrades *from* the version that ships it: Homebrew runs
  the installed copy's uninstall, so 0.16.0 → the next is the first kept
  upgrade (0.15.5 was released as 0.16.0); still to be seen on a real Mac
  then.
- **New tunnel's protocol list, folded (asked for by the user,
  2026-10-26, on a Mac).** All nine protocols show open below "Detect
  automatically", as design/screens/new-tunnel.html has it, which the
  user found unfriendly. A design change: it goes through a proposal
  (two assessments and a confirmation pass, before/after), not into a
  bug-fix release.
- **All unsafe code behind one safe layer (asked for by the user,
  2026-10-14). Done 2026-10-19.** `crates/crake-os` holds every unsafe
  call Crake makes, each behind a safe function whose contract the crate
  keeps itself, with its `// SAFETY:` reasoning: Unix and Linux
  (getifaddrs, interface names and indexes, signals, socket options,
  netlink sockets, tun devices for tests, tokio's `AsyncFd::register` only
  for `OwnsFd` types, WAYLAND_DISPLAY unset only while /proc says the app
  has one thread), macOS (sysctl, link counters, CoreFoundation's arrays,
  dictionaries and run loop, utun devices for tests; with `app`,
  SMAppService and the displays), Windows (IP Helper, adapters, interface
  counters, the TCP tables and their statistics, the registry, Credential
  Manager, the service manager, console events and sessions; with `app`,
  monitors, window placement and frames by the handle's value). crake,
  craked, crake-hook, crake-proto and lint-literals `#![forbid(unsafe_code)]`;
  `scripts/check-unsafe.sh` (lint gate) fails on `unsafe` anywhere else,
  tests included (they no longer set the environment: fake scripts get
  their paths written in). Not yet: unit tests of crake-os's Windows and
  macOS parts on their CI runners beyond what the journeys exercise.
- **Small screens (audit 2026-10-19, the user's question; fixed then:
  Settings capped to its screen, the panel's middle scrolls with its
  header and footer fixed, the sign-in card stops at its screen's bottom
  and scrolls, New tunnel's dropdowns scroll; `panel::tests::
  many_tunnels_keep_the_panel_on_its_screen`).** Not yet: tunnel detail
  puts "Add a host" after its rows, far down with hundreds of hosts
  ("Add a host" above the list: a design question, through a proposal;
  the rows themselves are built only in view since 2026-10-27,
  `tunnel_detail::hosts::rows_in_view`: 500 rows took 20 ms a frame, 2000
  took 89, now under 2 ms either way); no window but the
  profile editor can be resized (`is_resizable: false`); the panel's
  scrolling middle showed
  no sign it scrolls but its cut-off last row (done 2026-10-21: every
  scrolling area has a scrollbar while the pointer is over it). Done
  2026-10-27: windows open in the primary monitor's work area
  (`place::window_area`: Windows' rcWork, the Mac's visible frame, X11's
  `_NET_WORKAREA` cut to the monitor and believed only when it leaves room
  for some bar, else the 120 px kept before), and Traffic and the profile
  editor at their own height (`place::window_height`).
- **Ubuntu 22.04's resolved dropping the hush's domains (CI's VM,
  2026-10-26, once).** Right after ACME was switched off, craked made
  crake-hush and gave it acme.internal; resolved then listed the link with
  no domains for 20 s and more, so ACME's names went public (the package
  test's "ACME's names are refused at once"). Not seen on 24.04 or newer,
  nor since. craked now checks the link every 2 s and sets the domains
  again (`refresh_hush`, a warning in its journal: "resolved dropped
  crake-hush's domains"); the package test notes each time it happened and,
  on a failure, prints resolved's journal. Cause unknown: likely resolved
  249 re-reading a fresh link's settings (networkd's state, or its own
  saved file) after craked's D-Bus calls. Next time the note shows, the
  two journals' timings say which.
- **A kept sign-in that ran out, as a scenario (2026-10-28).** Built and
  tested with craked's fake openconnect (`lifecycle.rs`,
  `a_kept_sign_in_that_ran_out_asks_for_a_new_one`) and in the app
  (`windows.rs`, `a_sign_in_that_ran_out_opens_again_when_just_switched_on`),
  not against a real gateway: none of the test gateways forgets a session
  soon enough (ocserv keeps a cookie 300 s, `cookie-timeout`, and the other
  scenarios switch on again within that). An ocserv of its own with a
  `cookie-timeout` of a few seconds (extras.sh, like the groups' one, and
  on CI's gateway VM) would let a scenario wait it out.
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
- **A shared practices repository for dorhawk.software (the user's idea,
  2026-10-16; written down so it isn't lost, to do once Crake is stable).**
  What the user has in mind, in their words' spirit: Crake grew a lot of
  hard-won engineering practice (lints as hard errors, a magic-number lint,
  file and function size limits, coverage floors that only rise, a journey
  regression suite on every supported system, tests that never touch the
  developer's machine, split and cached CI, pruned caches, two-layer Nix
  dependency builds, release scripts that refuse an empty changelog, a
  hand-written changelog in the product's voice, design proposals before UI
  changes, "write down what we don't do now", nice'd builds, never
  `nix path:` on a repo with target/...). That knowledge should outlive
  Crake and help the sibling apps: tern (a Rust/GPUI mail client, on hold
  until Crake is stable) and Obálka (TypeScript, React Native, which has
  tried good patterns of its own). The plan:
  1. A new **public** repository under dorhawk-software describing the
     practices: each one with what it is, why it's worth it (the incident
     or cost that taught it, e.g. the 328 GB disk fill, the cache
     evictions), how to apply it, and how to check it's applied.
     Actionable, not essays.
  2. **Structure: common plus per language.** Practices that hold anywhere
     (changelog discipline, deferrals written down, design proposals,
     release gates, CI caching ideas, privacy rules for public repos) in a
     common part; Rust specifics (clippy deny, lint-literals, cargo deny,
     machete, llvm-cov floors, GPUI patterns) and TypeScript/React Native
     specifics (from Obálka) in their own parts, with room for more.
  3. **Central management:** project repositories register with it (or
     hook into it), and when the practices change, the change reaches every
     registered project (e.g. a workflow opening a pull request there), so
     the practices are available to future agents in each repository as
     skills, CLAUDE.md sections and checks, not just as a document.
  4. **Usable beyond dorhawk.software:** any repository could adopt it as
     its baseline, like a runbook of what was learned. How the hook works
     (a template, a GitHub Action, a CLI, a skills package) is to be
     designed when we start; this note is the reminder of the intent.
- **IPv6 hosts on a network without IPv6 (Linux; found 2026-10-20 by
  the scenario suite).** Crake routes by mark, after the system's own
  route lookup; on a network with no IPv6 route at all (much office
  Wi-Fi), an app's connection to an IPv6 address fails at once
  ("Network is unreachable"), so an IPv6-only internal host (a name with
  only an AAAA record) can't be reached through a tunnel there, and
  there's no packet to probe from. macOS' scoped default route has the
  same effect, and macOS' resolver asks no AAAA at all there (CI's Mac:
  an IPv6-only name came back empty); Windows' tunnel default route
  (metric 9000) is taken by all IPv6 there instead. Fixing it on Linux means a main-table route of
  Crake's (e.g. IPv6 default via the tunnel, unmarked packets refused),
  which goes against "no routes but the DNS's". **Decided with the user
  (2026-10-20): left as it is** (IPv6-only internal hosts are rare;
  dual-stack names work). If a real gateway shows one: a lowest-priority
  IPv6 default route into the tunnel only while the network has no IPv6
  of its own, with anything unmarked refused at once. The e2e network has
  IPv6 (as most do); the probe scenario says `not_here` where the
  runner's network has none.
- **Windows on ARM64 (planned with the user, 2026-10-20).** The user's
  coming Apple Silicon Mac runs Windows 11 for ARM in a VM, which runs
  x64 programs emulated but never x64 drivers, and Crake for Windows is
  x64 throughout (its programs, openconnect, Wintun, ovpn-dco,
  tap-windows6). An ARM64 build: `aarch64-pc-windows-*` for crake,
  craked and the hook, openconnect built for ARM64
  (scripts/build-openconnect-windows.sh), Wintun's and OpenVPN's ARM64
  drivers (both ship them), an installer per architecture (or one
  choosing at install), and CI building and testing it (GitHub has
  Windows ARM64 runners). Detail in docs/engineering/platforms.md when
  started.
  **Built 2026-10-20** (docs/engineering/platforms.md, "Windows on
  ARM64"): the programs, openconnect (MSYS2 CLANGARM64), OpenVPN's and
  Wintun's ARM64 files, `crake-setup-<version>-arm64.exe`, installed and
  removed on CI's `windows-11-arm` and in each release; the gateway
  tests on ARM64 by hand through the Doctor (system `windows-arm64`,
  first passed 2026-10-21). Not yet: a real Windows on ARM (the user's
  Mac).
- **Browser sign-in on Windows.** The openconnect Crake builds for
  Windows has no `--external-browser`; New tunnel and sign-in say so
  (2026-10-20). Building it in (and an opener there: `BROWSER` is
  xdg-open on every system but macOS) is for when someone needs it.
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
- **The sign-in window drawn taller than it was (2026-10-21, the user's
  notebook, right after a restart and login).** An OpenVPN tunnel's browser sign-in
  window showed its card cut at the bottom: no bottom border, Cancel and
  "Open the page again" cut in half. Not a window the window manager
  didn't grow: then GPUI knows the window is short, and the card shrinks
  and scrolls inside it
  (`signin_view::tests::a_window_left_short_keeps_its_card_inside`). So
  GPUI drew for a taller window than the one on screen, an X11/XWayland
  mismatch, likely while the session's scale was still settling (Crake
  started at login). Not reproduced; on the next connect the window was
  fine. If it comes back: `xwininfo -id <the window>` (its real size)
  beside CRAKE_DEBUG=1's output, and whether moving the window redraws it
  whole.
- **No-tray's command line has no scrollbar (2026-10-21).** Every other
  scrolling area shows one while the pointer is over it
  (`controls::over`); the one-line install command in the no-tray
  notice scrolls sideways under 6 px of padding, where a bar would sit
  over the text. Its Copy is the way to the whole line.
- **A bigger Nix cache, if Cachix's free 5 GB runs out (2026-10-28).**
  Since 2026-10-27 the nixos job pushes only the dependency builds
  (testing.md), which may fit; watch the cache's size on Cachix first. If
  it doesn't, the user's choice is **Cloudflare R2** as a plain Nix binary
  cache: a public bucket, `nix copy --to 's3://…?endpoint=…&region=auto'`
  signed with Crake's own key (a repo secret), CI substituting over
  `https://` with the public key, so forks still pull. $0.015/GB-month
  past 10 GB free (100 GB ≈ $1.35/month), downloads free, which matters
  since every nixos run pulls a few GB. Nothing evicts on its own: a
  lifecycle rule (e.g. 30 days) does, and the dependency layers are
  pushed again after it. Also weighed: Hetzner Object Storage (the same
  setup, ~€6.49/month for 1 TB), a Hetzner VPS with Attic (least recently
  used eviction like Cachix's, but a server to keep), Cachix Starter
  (50 GiB, the dearest). Moving means the nixos job's substituter, key
  and push step, testing.md and CLAUDE.md's "Nix goes to Cachix".
