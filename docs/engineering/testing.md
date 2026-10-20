# Testing Crake without installing anything

<!-- Moved out of CLAUDE.md on 2026-10-20 (CLAUDE.md had grown past the
     150k characters an agent's context takes in). CLAUDE.md keeps the
     rules and a summary with a pointer here; this keeps the detail. Edit
     it as CLAUDE.md was edited: what was decided, built and found, dated. -->

## Testing without installing anything

The user doesn't want development builds installed on their machine.

- `cargo test`: unit tests, plus lifecycle tests with a fake openconnect.
- `crates/crake/src/wire_tests.rs`: the app and craked together over real
  D-Bus (craked's manager on a private socket, as `--dev`; the app's own
  `client.rs` drives it and its updates feed the app's `Model`): add an
  OpenVPN 3 tunnel, attach, add a host, list hosts, change a setting, a
  refusal, detach, remove. A new D-Bus call gets a step there.
- Window tests (headless, GPUI's test platform: nothing on screen):
  `#[gpui_kit::test]` with `gpui_kit::test::TestWindowExt` (click, input,
  press, scroll), in `tests.rs` of every window: `new_tunnel/`, `panel/`,
  `settings_view/`, `tunnel_detail/`, `log_view/` and `signin_view/`
  (against a fake openconnect, given by `Spec::openconnect`; real I/O
  needs `cx.executor().allow_parking()`). An element is clickable from a
  test once it has `.test_support()` as its last call (a no-op in normal
  builds; the dev-dependency turns on gpui-kit's `test-support`). Nothing
  of this computer's in a test: `look_around` returns fixed profiles and
  engines, `Prefs { scratch: true }` writes nothing.
- **The journey suite: the regression suite. It stays green; every change
  (refactors above all) runs it, and every new entry point of the app gets
  a step in it.** `crates/crake/src/journey_tests.rs`, run by
  `scripts/e2e-test.sh` (and CI): the real app shell (`start(cx, Setup)`,
  what `main()` runs: model, request router, windows, D-Bus client) on
  GPUI's headless test platform, a real craked behind a real D-Bus socket,
  real openconnect against ocserv, real nftables. One day in the app: the
  panel's switch → sign-in (trust, password) → connected, tray included; a
  host probed → "What Crake did" → Undo/Redo down to nftables; Settings;
  tunnel detail's Add a host; the log's Copy for helpdesk; New tunnel →
  added → its sign-in cancelled; switching off; craked stopping and
  coming back under the app. Mutations it catches: Undo sending no hosts,
  logged decisions dropped. Two test-platform stand-ins, each said where
  it's done: frames are drawn by hand and a window's resize delivered as a
  window system would (`settle`), and New tunnel gets a full-HD height
  (its own fit-to-step runs on a next frame the test platform never
  sends). `CRAKE_JOURNEY_DEBUG=1` prints every click's target,
  `CRAKE_DEBUG=1` the app's own debug lines.
  **Everywhere Crake ships (requested, 2026-10-11): the journey, at least
  its basics, runs on every supported system after every change**, so an
  implementation change can't quietly break one of them: each Linux
  distribution (Ubuntu 22.04/24.04/26.04, Debian 12/13, Fedora 43–45,
  Arch, NixOS) with its own openconnect, resolved and libraries, Windows,
  and macOS when it comes. **Linux distributions: done.**
  `scripts/build-deb.sh` builds the journey's test binary on 22.04 with
  the packages (`target/deb/journey`); `scripts/deb-test.sh` runs it in
  every package test's container after the tunnel phase (`check.sh
  journey`: the service steps aside, the journey runs its own craked with
  the packaged hook and the system's openconnect, nft and ip). Passes on
  Ubuntu 22.04/24.04/26.04, Debian 12/13, Fedora 43/44/45 and Arch; NixOS in its VM test.
  Windows: done, `scripts/windows-vm-test.sh` runs `windows_journey.rs`
  (the same day: the panel's switch and sign-in, Undo/Redo, Settings,
  tunnel detail, the log, New tunnel, switching off, the service
  restarting) against the installed service, with `windows_tests.rs`
  (helpers shared through `journey_kit.rs` and `test_app.rs`). It found
  two bugs: a new Wintun adapter's address sat in duplicate-address
  detection, so the first probe through a fresh tunnel failed (addresses
  are now created Preferred, as WireGuard does), and a domain kept private
  for a tunnel that's off beat a tunnel that's up with the same domain
  (`dns_links::owner` now prefers the tunnel). In CI too:
  `ci-through-cloudflare.sh` runs `windows_journey` against the installed
  service and the CI gateways through Cloudflare (the `windows-gateways`
  job), and `macos_journey` (craked as a process of its own, as root)
  in the `macos` job; both pass in every run (checked 2026-10-19, after
  this line had said macOS was still to do).
- `scripts/netns-test.sh`: real nftables and policy routing inside a
  disposable container with no network (host filesystem read-only, with a
  `/run` of its own: until 2026-10-15 the container saw this computer's
  /run, so a test running as root reached its system bus, the user's real
  openvpn3 and systemd-resolved; the e2e containers too).
- `scripts/e2e-test.sh` (`crates/crake/src/e2e_tests.rs`, skipped unless
  `CRAKE_E2E=1`): a real gateway. ocserv in one container (`scripts/e2e/`:
  user alice, password hunter2, a self-signed certificate; ACME's network
  10.40.0.0/16 behind it: DNS 10.40.0.2, git.acme.internal 10.40.1.10,
  wiki.acme.internal 10.40.1.11), the test in another, run from the host's
  read-only filesystem with the host's openconnect, nft and ip. Neither has
  a network: their only link is a socket pair in a temporary folder
  (`relay.py`, which drops UDP 443 while the test's `block-udp` file
  exists). The app's sign-in (trust the certificate, the password), craked
  over D-Bus, probing, a name only the tunnel's DNS knows, switching off
  and stopping leaving nothing; and a network that blocks UDP (7b: noticed,
  over TCP on the same session, remembered). Keep both containers without a
  network: a gateway on a Docker network would be a service on this machine.
  `CRAKE_E2E_FILTER` picks tests (e.g. `scenarios::ipsec`).
- **The scenario suite (`crates/crake/src/scenarios/`; asked for by the
  user, 2026-10-19: one scenario per test, the same tests on every
  system).** Each scenario is one `#[test]`, written once in `checks.rs`
  and run against every kind of gateway (`every_scenario!`: AnyConnect,
  OpenVPN, and IPsec on Linux) on Linux (the e2e containers), macOS and
  Windows (the CI gateways through Cloudflare). Each starts from nothing,
  like a Spring test's context: craked fresh (Linux, macOS: in the test;
  Windows: the installed service, its test tunnels removed), the tunnel
  added, signed in, connected; then one scenario; then all removed. What
  differs per system is `platform/` (where an address goes: Linux by
  Crake's nft sets, macOS `route get`, Windows `Find-NetRoute`; the
  tunnel's device; a name over TCP; what's left behind). A scenario a
  system can't run says so in the output (`not_here`), never passes
  quietly. The Linux client container runs the system's own
  systemd-resolved on a D-Bus of its own (scripts/e2e/client.sh), with the
  gateway's public DNS stand-in (203.0.113.53, dnsmasq, which logs every
  question) as the network's DNS: names are asked as an app asks them,
  so the public-name rescue and "keep names private" run on Linux too
  (proven by the stand-in's log: the name never left). The testcontainers
  crate was weighed and not used: the test runs as root inside a
  no-network container, and starting containers from there would need
  Docker's socket in it (root on this machine); the gateway's container
  stays one per run, started by the script. Its first run found craked
  leaving an IPsec tunnel's XFRM interface behind when it ended without
  stopping it (unlike a tun device, it outlives its process); craked now
  removes Crake's leftover ones when it starts (`Net::remove_left_xfrm`).
  Since 2026-10-20 it also covers split-horizon (Linux only: it needs the
  gateway reached by name, vpn.acme.example from the public DNS stand-in,
  and a public DNS the test controls; through Cloudflare the gateway is
  127.0.0.1, so macOS and Windows say `not_here`), a tunnel's DNS that
  stops answering (`platform::silence`: an nft rule, a pf anchor of the
  test's own, a firewall rule), a second tunnel that clashes, two
  companies at once (Globex: ocserv's second user, bob, with its own
  network 10.41.0.0/16, DNS and domain from `ocserv-users/bob`, so it
  rides the same Cloudflare route; `scripts/e2e/companies.sh` sets both
  companies' networks up for start.sh and ci-start.sh, and a host both
  reach, 10.42.0.10), "Use Globex" moving a host, one address for two
  companies, and IT's tunnels (`it_scenarios!`, AnyConnect and OpenVPN:
  managed.d in craked's test folder, or the installed service's with a
  restart on Windows). In the e2e image ACME's domain comes from
  `ocserv-users/default`'s `split-dns` (`ocserv-companies.conf`, added to
  ocserv.conf in the image, takes the global default-domain out, which
  would reach bob too; the package and Windows VM tests use ocserv.conf
  alone: CI's package tests failed the one run it was changed). carol
  signs in with a certificate (a PKCS#12 file from ACME's users' CA, in
  the image; the gateways-ready artifact carries it to the Mac and
  Windows jobs): the `anyconnect_certificate` scenarios. Found then:
  IT's files were read only with a craked.toml, which Windows' and macOS'
  installers don't ship (`Config::without_file`); the memory answerer
  (7h) took over a tunnel's names in the wrong order and was left behind
  by a craked that ended during an outage; and IT's tunnel edited with
  partial fields was refused for its fields, not for being IT's.
  The routing modes "Ask me first" and "Only what I list" (set as Edit
  sets them, UpdateTunnel with the tunnel's own fields; a host added by
  hand to one's own list tunnel goes through: only IT's scope refuses
  that) and another program rewriting a tunnel's DNS link (Linux; a
  mutation that stops `keep_dns_links` fails it) are scenarios too.
  A gateway whose sign-in asks the group is a second ocserv on 8445
  (start.sh; erin, of Engineering and Sales; Linux only, as picky.py).
  OpenVPN's dave signs in with his password and then a code (sso.py's
  CR_TEXT challenge; `openvpn_code`, on every system).
  A gateway that admits only AnyConnect (7d) is `scripts/e2e/picky.py`, a
  front on 8444 that answers 404 to any other user agent and passes TLS
  records through one for one (re-chunked, ocserv ends the session:
  "unexpected CSTP length"); Linux only (a Cloudflare hostname of its own
  would be needed). `CRAKE_E2E_KEEP=<folder>` keeps the gateway's logs
  (ocserv logs to /link/ocserv.log).
  AnyConnect's browser sign-in (2026-10-20): ocserv speaks no SAML, so
  `scripts/e2e/cisco_sso.py` (port 8446) plays Cisco's sso-v2 "external
  browser" in front of it: the sign-in page, the token encrypted to the
  key openconnect sent (its hpke.c: ECDH P-256, HKDF "AC_ECIES",
  AES-256-GCM) handed to openconnect's port 29786, then it signs in to
  ocserv itself and passes the tunnel through. The client's browser is a
  `xdg-open` of client.sh's that follows the page with curl (on PATH,
  in the client's /run, which e2e-test.sh mounts exec). The scenarios
  `anyconnect_browser` (Linux only: elsewhere the browser would be a
  real one). Its first run found every browser sign-in on Linux and
  macOS failing: openconnect starts the browser with posix_spawn, which
  doesn't search PATH, and Crake named it bare (now found on PATH,
  `signin::browser_on_path`).
  The engine's other paths, Linux (2026-10-20, each on every gateway
  kind): a router's ICMP "host unreachable" starting the probe at once
  (the e2e gateway rejects 10.40.1.17 from the wire), a UDP host learned
  (an echo on 10.40.1.18:7007), a full tunnel and back (the public web
  server drops what comes through a tunnel), another VPN client's domain
  left to it (a resolved link `othervpn0` with the public DNS stand-in;
  a mutant craked that sees no other client's domains fails it), and the
  first-SYN race (`racing!`: craked started with `race_first_syn`, which
  the other scenarios leave off; one connection with 2 s to connect,
  which times out with the race off). Each says `not_here` on macOS and
  Windows, with why.
  Not yet in the suite (2026-10-20): a certificate kept in the keyring
  or on a smart card (the tests keep no keyring; SoftHSM could stand in
  for a card, but the e2e client runs from this computer's read-only
  filesystem, so its softhsm2 module and tools would have to be
  installed here: never. It needs a client image of its own, as the
  gateway has, or the package tests' containers, which install what
  they like), OpenVPN's and IPsec's certificates, and a full tunnel and
  IPv6 off Linux (Cloudflare carries neither).
  Windows' "stopping craked leaves nothing" stops the installed service
  (its adapters, NRPT rules, routes and firewall rules must be gone, names
  must work) and the next test starts it again; IPv6 inside the tunnel
  (ocserv hands it out, ACME's fd40:1::10 and v6.acme.internal) runs on
  every system.
  Split-horizon runs everywhere since 2026-10-20: through Cloudflare the
  gateway is named crake-ci-acme.dorhawk.software (the runner's hosts
  file points it at cloudflared's local end, ci-through-cloudflare.sh),
  and ACME's DNS gives packages.dorhawk.software a private address. The
  DNS-rewrite scenario does each system's version: openvpn3's rewrite of
  the link (Linux), Crake's /etc/resolver file removed (macOS), its NRPT
  rule removed (Windows); the last two found craked never checked them
  (`link_as_set` was always true there).
  Seen once (CI's Windows, 2026-10-20): Globex's name asked right after
  it came up beside Initech had no answer; asked again it had. The
  scenario asks for up to 5 s and prints each miss. If it shows up again,
  craked's DNS server needs a debug line per question to say why (the
  new adapter not taking traffic yet is the guess).
  CI time (2026-10-20): Windows' gateway tests run on two runners at once
  (`windows-gateways (anyconnect)` and `(openvpn)`, split by
  CRAKE_GATEWAY_FILTER), and the scenarios' craked asks a tunnel's DNS
  every 5 s (`dns_check_secs`; Windows' service from the craked.toml
  ci-through-cloudflare.sh writes): the outage scenario took 3 minutes
  at the default 30.
  The older chained tests (`e2e_tests.rs` and the rest) still run; their
  checks move into scenarios as the testing-gaps catalogue is worked down.
- CI (`.github/workflows/ci.yml`, on every push; a new push cancels the
  branch's run still going): the lint gate, `cargo test`, the coverage
  floors, Crake for Windows (checked, then built and tested on a Windows
  runner), the netns and e2e scripts, the NixOS VM test, and the packages:
  built once (`packages`), then one `package-test` job per system, all at
  once, the journey included. Caches (asked for: quick feedback): Rust's
  (rust-cache) for check and Windows; `~/crake-cache` for the packages
  (`CRAKE_CACHE`: build-deb.sh and build-arch.sh use a folder instead of
  their Docker volumes; a new key each week, stable Rust's version and
  Cargo.lock: on 2026-10-16 Rust 1.99.1 came mid-week, the cache held
  1.99.0's build, and since a key that exists is never saved again every
  .deb built from scratch all week; Arch's own rust package too, which
  pacman installs apart from rustup's: on 2026-10-16 Arch rebuilt 1.99.0
  as pkgrel 3 and every Arch build took 18 min instead of 4), about 35 s
  instead of 14 min for the .deb, 37 s instead of 10 for Arch; the Nix store
  (cache-nix-action), where crane keeps the crates.io dependencies a
  derivation of their own (packaging/nix/package.nix; the vendored GPUI
  backend stays whole in it).
  **Caches (asked by the user, 2026-10-12: runs took 35 min with
  "No cache found"):** GitHub keeps 10 GB of caches per repository and
  evicts the least used; Nix (3.3 GB), the packages (2.6 GB, a new one
  each week), check (2.5 GB), the Windows build (1.5 GB) and the rest,
  with copies per branch, didn't fit, so each run evicted what the next
  needed. Now caches are saved by `main`'s runs only (rust-cache
  `save-if`, actions/cache's restore/save split, cache-nix-action's
  `save`) and restored on every branch; CI builds carry line tables only
  (`CARGO_PROFILE_DEV_DEBUG`), not full debug info; and `check` runs the
  tests once, under coverage (the separate `cargo test` was the same
  suite again). A branch run is fast once `main` has run with the same
  Cargo.lock. Measured on main: packages 2.6 GB, the Windows build 1.4,
  check 1.3, macOS 1.1, network 1.1 and Nix 3.3 still came to ~10.8 GB
  and Nix's was evicted. **The namespace and e2e tests run in `check`
  (2026-10-12, asked for: faster CI):** they had a `network` job of their
  own, the slowest (~15 min, ~11 of them compiling: `check`'s coverage
  build is instrumented, in another folder, and a `-p crake` build unifies
  dependencies' features differently, so nothing was reused).
  `scripts/check-coverage.sh` now measures the cargo-llvm-cov "show-env"
  way (plain `cargo test --workspace`, instrumented, in
  target/llvm-cov-target, opt-level 0) and then runs the scripts it's given
  in that very build (`CRAKE_CARGO_SCOPE=--workspace`): they build nothing.
  Their binaries in containers write no coverage (`LLVM_PROFILE_FILE=
  /dev/null`). Locally the scripts work as before. **GitHub's 10 GB can't be raised** (asked 2026-10-12: same for
  public/OSS and private, no paid tier), so the Nix store (the 3.3 GB
  whale it kept evicting) moved to **Cachix** (free for open source,
  cache `crake`): the nixos job's `cachix/cachix-action` pushes every path
  it builds and substitutes from it next time; Cachix has no 10 GB cap, so
  GitHub's budget now holds only the five Rust caches (~9 GB). **Replaced
  caches are deleted (asked by the user, 2026-10-14):** after a Cargo.lock
  change the Rust caches were saved again under the new key while the old
  ones stayed, and together they pushed out the packages cache, so 0.7.0's
  .deb was built from scratch (17 min instead of 7). CI's `caches` job, on
  main after the jobs that save, runs `scripts/prune-caches.sh`: per kind
  (the key without its trailing hashes and dates) only the newest stays.
  **The package cache is swept before it's saved (2026-10-17):** it had
  grown to 6 GB (from 2.6) and pushed out the macOS cache (macOS then
  built from scratch every run): its restore keys carry it on from week
  to week, and nothing was ever removed from the two target folders in
  it, so each compiler (rustup's 1.99.0 and 1.99.1 side by side, Arch's
  rebuilds), each version of Crake's own crates and each replaced crate
  stayed. `scripts/sweep-target.sh`, run by both package builds in their
  containers, keeps only the units Cargo's JSON says the build used
  (fresh ones included; the Arch build's PKGBUILD stays the AUR's, a
  `cargo` wrapper first on PATH asks for the JSON) and drops the unpacked
  registry sources. Setup (done
  with the user): a Cachix account, a public cache named `crake`, an auth
  token as the repo secret `CACHIX_AUTH_TOKEN`. Cachix's free plan holds
  5 GB and then evicts the least recently used paths (a warning email at
  85%); it had reached ~3.5 GB by 2026-10-12 because every path a run built
  was pushed, Crake's own per-commit outputs included. Now `pushFilter`
  keeps those out: only the dependency derivations (crane's `crake-deps`,
  `crake-journey-deps`) go up, which is what the next run reuses. The NixOS VM-boot (~6-8
  min) is uncacheable, so nixos stays the slowest job even cached.
  When Cargo.lock changes, nixos takes ~50 min (measured on 0.2.0's
  run: crake-deps 17, crake 2½, crake-journey-deps 25, the journey 3,
  the VM 1). Not for a release's version bump (2026-10-14): the
  dependency builds never see Crake's version (packaging/nix/package.nix:
  the cleaned Cargo.tomls drop it, the dummy source gets a Cargo.lock at
  0.0.0, made at evaluation, since a derivation is hashed by its inputs),
  and `scripts/nix-deps-check.sh` (first step of the nixos job) fails if
  they would change with it. **Two layers (asked by the user,
  2026-10-14):** a dependency change no longer rebuilds them all. Each
  dependency build has a base, the dependencies of an earlier commit
  (flake input `crake-base`, pinned in flake.lock; `crake-base-deps`,
  `crake-journey-base-deps`), and the current ones built on top of it
  (crane's mkCargoDerivation with the base as `cargoArtifacts`: its
  buildDepsOnly always starts empty), so cargo recompiles only the crates
  that changed and what depends on them. Measured locally: the rustls
  update on a base from before it recompiled rustls and Crake's dummy
  crate, 12 s instead of ~6 min (both cargo passes), where the first
  attempt recompiled all 687 crates: cargo keeps a build only while its
  sources' path is the same ("PathToSourceChanged"), and crane's vendored
  sources are a store path named after the whole Cargo.lock, so every
  Crake build (`postConfigure` in package.nix) reads them through a link
  at `/build/vendor-cargo-deps`, only when the build folder is /build:
  Nix's sandbox. Without one, Nix 2.30 and later give every build a new
  folder (`/nix/var/nix/builds/nix-<pid>-<n>`), the link moved each time
  and CI recompiled everything, Crake's own build included (found on
  26bcc8f's run: 90 min); there the store path is kept, which at least
  reuses everything while Cargo.lock is unchanged. So CI's nixos job turns
  the sandbox on (`sandbox = true`; Ubuntu 24.04 needs
  `kernel.apparmor_restrict_unprivileged_userns=0` for the runner's
  user's namespaces). Moving the base forward (`scripts/nix-base.sh [COMMIT]`,
  default main's latest; commit flake.lock) costs one full build (~45 min)
  and is worth it when the layer on top has grown. This covers what a
  Cargo.lock trimmed to Linux's crates was meant to (not possible: Cargo
  resolves the lock for every platform at once, so a lock without
  Windows' crates fails `--locked`). Merging the two deps derivations was considered and left
  (2026-10-13): the journey's build turns on GPUI's test-support, so GPUI
  compiles twice either way; only small shared crates would be saved.
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
- Filming the app (the brag video, the website's images; 2026-10-15): demo
  mode answers Undo and Dismiss itself (no craked), `CRAKE_DEMO=openvpn3`
  makes Nordbank an OpenVPN 3 tunnel (tunnel detail's "Edit…", a long
  profile name), `CRAKE_SHOW=profile:<tunnel>` opens its profile editor
  (a sample profile in demo mode), `CRAKE_APPEARANCE=dark|light` pins the
  theme, `CRAKE_DEMO=long` adds two names too long for one line in the
  panel (Azure private-link names: the panel grows after opening). Rendered headless on Xvfb (`GPUI_X11_SCALE_FACTOR=2`), each window
  cut into sections with its rounded corners made transparent.
- Motion: `CRAKE_SHOW=panel` opens the panel at start; `CRAKE_DEMO=arrive`
  adds a decision and flips the last switch every 8 s; `CRAKE_MOTION=slow`
  runs every animation ten times slower, `=off` turns them off (as GNOME's
  "Animations" switch does). Screenshot-portal captures flash the user's
  screen: take one or two, never a burst; use headless Chrome for proposals.
- Real gateway from a terminal: `sudo ./target/debug/craked --config dev/<yours>.toml --connect <tunnel>`
  in a separate terminal. Signs in on the terminal, then takes commands
  (`add`, `pin`, `unpin`, `forget`, `list`). Ctrl+C removes everything; state
  lives in `/tmp/crake-dev`. `dev/` is git-ignored.

