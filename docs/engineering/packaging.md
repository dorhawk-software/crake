# Crake's packaging and dependencies

<!-- Moved out of CLAUDE.md on 2026-10-20 (CLAUDE.md had grown past the
     150k characters an agent's context takes in). CLAUDE.md keeps the
     rules and a summary with a pointer here; this keeps the detail. Edit
     it as CLAUDE.md was edited: what was decided, built and found, dated. -->

## Packaging and dependencies (decided with the user)

Crake drives VPN engines it doesn't contain: openconnect (AnyConnect,
GlobalProtect, Pulse, Juniper, Fortinet, F5, Array) and OpenVPN 3. Rule:
**where the platform has a package manager, depend on its packages; where
it doesn't, bundle, and own the updates.**

- **Debian/Ubuntu (.deb), first:** never bundle openconnect or other system
  software (a bundled copy misses the distro's security fixes; Debian policy
  discourages it). Rust crates (GPUI etc.) are linked in as usual; system C
  libraries come from `${shlibs:Depends}`.
  ```
  Depends:    ${shlibs:Depends}, nftables, iproute2, systemd-resolved,
              polkitd, dbus, openconnect | openvpn3
  Recommends: gnome-shell-extension-appindicator, gnome-keyring
  Suggests:   openvpn3
  ```
  At least one engine is required (`openconnect | openvpn3`): an
  OpenVPN-3-only user pulls nothing extra, a user with neither gets
  openconnect. Not plain Recommends: `--no-install-recommends` would leave
  Crake with no engine. openvpn3 isn't in Ubuntu's archive (OpenVPN's own
  repository), so it's never pulled in, only suggested. The tray extension
  and keyring are Recommends (KDE needs neither).
- **First idea: a PPA on Launchpad** (superseded by our own apt repository,
  see the toolchain finding below), so Ubuntu users
  `sudo add-apt-repository ppa:…/crake && sudo apt install crake`.
  Launchpad builds from a *source* package, on its own builders, without
  network access: the Rust crates must be vendored into the source package
  (`cargo vendor`, `.cargo/config.toml` pointing at the vendor dir), and
  the build must use a Rust toolchain from Ubuntu's archive (versioned
  `rustc-1.xx`/`cargo-1.xx` packages; check the series we target has one
  new enough for edition 2024 and GPUI). debian/ with rules, control (the
  Depends above), changelog, copyright; signed with the user's GPG key,
  uploaded with `dput`. Test the built .deb in a throwaway container
  first, never on the user's machine.
- **Toolchain finding (built in containers):** GPUI needs Rust 1.95 in
  practice (`std::hint::cold_path`, `slice::as_array`), whatever the
  crates declare. Ubuntu's archive has rustc-1.93 at most in 26.04 and 1.91
  in 24.04/22.04; only 26.10 has 1.95+. So a Launchpad PPA could build
  Crake for 26.10 and newer only. **Decided with the user** (who runs
  26.04 LTS on purpose): build the .deb ourselves (GitHub Actions, current Rust via
  rustup) on the oldest Ubuntu supported (22.04), so one .deb runs on
  22.04, 24.04, 26.04, 26.10 and recent Debian, and publish it in our own
  signed apt repository (e.g. apt.dorhawk.software on Cloudflare) so
  `apt install crake` and updates work everywhere. Depends need
  alternatives for older releases: `systemd-resolved | systemd (<< 252)`,
  `polkitd | policykit-1`.
- **The .deb: built.** `scripts/build-deb.sh` builds it in an Ubuntu 22.04
  container with rustup's stable Rust (`packaging/deb/`: control with the
  Depends above plus `dpkg-shlibdeps`' libraries, maintainer scripts that
  enable and start `craked.service`, a desktop entry, a comment-only
  `/etc/crake/craked.toml` conffile; purge removes `/var/lib/crake`).
  Layout: `/usr/bin/crake`, `/usr/sbin/craked`, `/usr/libexec/crake/hook`,
  the unit, D-Bus policy and polkit action under `/usr/lib` and
  `/usr/share`. `scripts/deb-test.sh` installs it in throwaway containers
  that boot systemd (22.04, 24.04, 26.04; the container's own cgroups made
  writable, real `ip_unprivileged_port_start`): apt resolves the Depends,
  craked runs as the service, polkit refuses users without an active
  session, then, cut off from every network, a real tunnel through the
  installed service (ocserv in a namespace of its own): probing, DNS via
  systemd-resolved, names kept private by craked's DNS on port 53; remove
  and purge leave nothing. It found the unit lacking
  `CAP_NET_BIND_SERVICE` (craked's own DNS couldn't bind port 53), and that
  a systemd-resolved that doesn't answer kept tunnels from coming up (the
  hook timed out); resolved now gets 3 s per call (`resolved.rs`).
  **AppArmor:** `packaging/apparmor/usr.sbin.craked` confines craked (its
  files, netlink, its D-Bus name and calls to resolved and polkit, ip, nft,
  openconnect and the hook); the package loads it before craked starts and
  unloads it on removal. openconnect runs under craked's profile, not one
  of its own: the unit's NoNewPrivileges forbids the switch ("no new
  privs"). Containers can't test it (loading a profile there would load it
  into this machine's kernel), so `scripts/vm-test.sh [--complain]
  [release]` runs the same checks in a throwaway Ubuntu VM (qemu, KVM, no
  display, no ports; cloud image cached in `target/vm/`) with the profile
  enforced: craked and openconnect confined, nothing refused. Passes on
  22.04, 24.04 and 26.04. On a computer with Crake installed (the user's), the
  package test's containers run craked under the host's loaded profile
  (the kernel attaches it by path, /usr/sbin/craked; found 2026-10-18 when
  the containers' openvpn was refused by 0.10.4's profile): a new rule is
  proven by vm-test.sh only, and a container's AppArmor denial may be the
  host's older profile. **In CI since 2026-10-19** (the testing-gaps
  review: CI's Linux runners have KVM, as the nixos job's VMs use it):
  the `vm-test` job, after `packages`, runs it on 22.04, 24.04, 26.04
  (AppArmor enforced) and fedora:44 (SELinux enforcing), the journey
  phase included (vm-test.sh copies target/deb/journey into the VM).
  Still to do, with the user: the signed apt repository (their key) and its
  publishing.
- **Upgrades keep tunnels up (requested).** postinst, upgrading a running
  craked, leaves `/run/crake/handover` and restarts it; craked, stopping,
  then hangs up each openconnect tunnel without logging off (SIGHUP) and
  writes their sessions, and the OpenVPN 3 devices the app attached, to
  `/run/crake/resume.json` (0600, tmpfs); the new craked reads it once,
  deletes it and switches them on again ("Crake was updated. Your tunnels
  are reconnecting with your sign-ins."; no "Switched off/on" lines). A
  flag older than 2 min or a file older than 10 min is ignored. The unit
  needs `RuntimeDirectoryPreserve=restart` and `KillMode=mixed` (without
  it systemd SIGTERMs openconnect itself, which logs off). Tested: lifecycle
  (`an_upgrade_hands_the_tunnels_to_the_new_craked`) and the package test
  (`dpkg -i` over a connected ACME: back, signed in, learned hosts still
  routed), containers and AppArmor VMs.
- **Ubuntu 22.04 (decided with the user): supported, with password and
  certificate sign-in only.** Its openconnect is 8.20, without
  `--external-browser` (openconnect 9+). `engines.rs` asks openconnect's
  `--help` for it (and `--version` for the number); New tunnel tags the
  browser card "Not available", says why ("…needs openconnect 9 or newer;
  this computer has 8.20. Use a username and password or a certificate."),
  won't continue with it, and starts new tunnels on the password there.
  Sign-in turns openconnect 8's "unrecognized option '--external-browser'"
  into the same sentence. Checked against 22.04's real openconnect.
- **At runtime (built, `engines.rs`):** the app checks which engines exist
  (`engines::openconnect()`: `CRAKE_OPENCONNECT`, openconnect on PATH,
  or /usr/sbin and /sbin: Debian installs it in /usr/sbin, which its
  users' PATH leaves out, so the app called it missing there; found in a
  Debian 13 VM, the package test runs as root and missed it; openvpn3's D-Bus
  configuration service file). New tunnel shows every protocol; one whose
  engine is missing is tagged "Not installed", says how to get it ("Needs
  openconnect, which isn't installed: sudo apt install openconnect") and
  can't be continued with. Sign-in that can't start openconnect says
  "openconnect isn't installed" with the same command.
- **Fedora/RPM: built.** `scripts/build-rpm.sh` packs the files
  `build-deb.sh` staged (`target/deb/root`: the same binaries, built on
  22.04, run on Fedora's newer glibc) with `packaging/rpm/crake.spec`:
  `Requires: (openconnect or openvpn3)`, nftables, iproute,
  systemd-resolved, polkit, dbus; `Recommends:` as above; no AppArmor
  profile (Fedora confines services with SELinux; craked runs as an
  unconfined service there: `scripts/vm-test.sh fedora:44` runs the whole
  package test in a Fedora VM with SELinux enforcing, a real tunnel
  included, and SELinux refuses craked nothing). The test's gateway enters
  its namespace with `nsenter`: `ip netns exec` / `ip -n` remount /sys,
  which SELinux refuses to a test script there. %post enables craked, or on an upgrade leaves the handover flag and
  restarts it; removing takes /var/lib/crake (rpm has no purge).
  `scripts/deb-test.sh fedora:43 fedora:44 fedora:45` runs the same
  package test with dnf and rpm.
- **Arch: built.** `packaging/arch/PKGBUILD` (for the AUR; builds a
  release's source with Arch's Rust; `options=('!lto')`: makepkg's -flto
  makes C parts of crates GCC bitcode lld can't link) and `crake.install`.
  pacman has no alternatives: both engines are `optdepends`, and the app's
  engine check says which a tunnel needs. Arch layout: everything in
  /usr/bin, the hook in /usr/lib/crake (the shipped craked.toml sets
  `hook_path`). Arch enables no services on install: the install message
  says to point /etc/resolv.conf at resolved and `systemctl enable --now
  systemd-resolved craked`; post_upgrade hands tunnels over as elsewhere.
  `scripts/build-arch.sh` builds it from the current commit;
  `scripts/deb-test.sh archlinux:latest` tests it, its gateway in a second
  container wired by `relay.py` (ocserv is AUR-only). Publishing to the
  AUR needs the user's AUR account.
- **Not Flatpak or Snap:** craked needs root-level network control on the
  host (nftables, policy routing, resolved), which sandboxes don't allow.
- **NixOS: built.** `flake.nix`: `packages.x86_64-linux.default`
  (`packaging/nix/package.nix`, buildRustPackage from Cargo.lock; GPUI's
  run-time libraries added to the app's rpath), `nixosModules.default`
  (`services.crake.enable`: craked's service as packaging/craked.service
  with nftables, iproute2 and openconnect on its path, D-Bus, polkit,
  resolved, /etc/crake/craked.toml with the hook's store path; dhcpcd and
  NetworkManager told to leave `crake-*` alone: dhcpcd took crake-acme and
  the DNS craked had handed resolved for it was gone). `checks.x86_64-linux.vm`
  (`packaging/nix/test.nix`): two VMs on the test's own network, ocserv
  behind which ACME's network sits, and a client with the module: sign-in,
  connect over D-Bus, probing, DNS, names kept private.
  `scripts/nix-test.sh` runs it in the nixos/nix container (no Nix on this
  machine), CI with nix-quick-install-action. Upgrades keep tunnels signed
  in: the module reloads craked on `nixos-rebuild switch`
  (`reloadIfChanged`), and its reload leaves the handover flag and
  restarts it, as the packages' upgrade scripts do. The VM test also
  checks polkit's refusal, the upgrade (a specialisation with craked's
  unit changed, switched to with ACME up), and runs the journey (Nix
  builds its test binary: the flake's `packages.journey`). IPsec
  (2026-10-20): `services.crake.ipsec = true` puts strongSwan in the
  system's profile (where the app finds it,
  /run/current-system/sw/libexec/ipsec/charon), names its charon in
  craked.toml and runs it as `crake-charon.service` (the package's unit,
  without AppArmor); New tunnel says that line on NixOS. The VM test's
  gateway also runs openvpn (sso.py signing alice in) and strongSwan
  (scripts/e2e's settings), and the client connects an OpenVPN and an
  IPsec tunnel through craked over D-Bus, each asked its password with
  Asking/Answer, a host learned through each. Locally the
  container makes its own /dev/kvm open to all (Nix's build users lose
  their groups; without it the VMs ran emulated).
- **macOS and Windows (someday):** no package manager to pull engines in,
  so the installer bundles them: openconnect (Windows: its DLL plus Wintun;
  macOS: inside the signed, notarized .app). Bundling makes Crake
  responsible for their security updates: a Crake release follows each
  openconnect security release. openconnect is LGPL-2.1: ship it as a
  separate library with its licence and a pointer to its source. OpenVPN 3
  there: its core library, or OpenVPN Connect when installed; to decide.
  A Homebrew formula could depend on Homebrew's openconnect instead.

