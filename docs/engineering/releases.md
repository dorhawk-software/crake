# Releases and newer versions

Moved here from CLAUDE.md on 2026-10-21 (the user's rule on keeping CLAUDE.md small: detail lives in the documentation it belongs to; CLAUDE.md keeps the rule and a pointer). Nothing was dropped in the move; add to this file from now on.

## Releases (decided with the user, 2026-10-13)

`CHANGELOG.md`, written by hand for the people using Crake, in its copy
voice (not generated from commits, which are written for developers):
**every user-facing change adds a line under "Unreleased" in the same
commit.** Dependency updates too (asked by the user, 2026-10-19): a
merged Dependabot update, or one fixed by hand, gets a line saying what
was updated and what it means for the people using Crake, since a
security fix in a dependency warrants at least a hotfix release, and
release.sh refuses an "Unreleased" with nothing in it, and a change outside
its kinds: `### New`, `Better`, `Changed`, `Fixed`, `Underneath` (what Crake
is built on; a dependency's security fix is Fixed), the headings the app's
What's new shows (`whats_new/`, the changelog built in; a test reads it
whole). To release, `scripts/release.sh <version>` (asked for by the
user, 2026-10-13; `--dry-run` first shows the changes and pushes
nothing): it refuses an empty "Unreleased", dates it
(`## <version> (<YYYY-MM-DD>)`), puts the version into Cargo.toml (and
Cargo.lock), packaging/nix/package.nix and packaging/arch/PKGBUILD,
commits "Crake <version>" and pushes, waits for CI on that commit and
pushes the tag `v<version>`. Interrupted, or after a fix CI asked for, run
it again with the same version: it waits on main's latest commit and tags
it. Don't push anything else to main while it waits (a push cancels the
CI run it's waiting for).
`.github/workflows/release.yml` refuses a tag without its section, with a
version that disagrees, or on a commit CI hasn't passed
(`scripts/release-notes.sh`); it builds the .deb and RPM, the Windows
installer (openconnect included, unsigned) and the macOS package (signed,
notarized, installed and removed on the runner) in release mode with the
same scripts CI uses, and publishes them with SHA256SUMS as the GitHub
release, its text the changelog's section. Then the Homebrew cask
(`packaging/macos/crake.rb.in`, the version and the package's checksum
filled in) is pushed to **dorhawk-software/homebrew-tap** (public, created
with the user's go-ahead 2026-10-13; `brew install --cask
dorhawk-software/tap/crake` installs Homebrew's openconnect with it) with
a write deploy key of that repository only (secret
HOMEBREW_TAP_DEPLOY_KEY; the organisation had deploy keys off, the user
switches them on). **The apt/dnf repository (built 2026-10-13; moved to its own
repository the same day, asked for by the user: no app should own what
the apps share):** `packages.dorhawk.software` is published by
**dorhawk-software/packages** (local: ~/Programming/Personal/dorhawk-packages):
the R2 bucket `dorhawk-packages` (the user's Cloudflare account; custom
domain, TLS 1.2 minimum, the r2.dev URL off; `/` serves index.html
through a URL rewrite rule on the dorhawk.software zone), its signing key
("dorhawk.software packages", B17A7118E97736C202AD5EAFF1D152DA1BBD1C97,
RSA 4096, signing only, no expiry; private half only in the user's
password manager and that repository's secret), the R2 token, the start
page and the scripts all live there, with `apps.json` listing the apps
(Crake for now). Its workflow fetches each app's latest release, tests
the repository in no-network containers with a throwaway key, signs the
RPMs, builds and signs the indexes, uploads them in order and installs
from the live site; it runs when an app's release asks
(`repository_dispatch` "release"), on a push, by hand, and hourly (stopping
at once when nothing is new). Crake's release only asks, in its
`packages` job, with the optional secret PACKAGES_DISPATCH_TOKEN (a
fine-grained token of the user's: Contents read/write on that repository
only; without it the hourly run finds the release). Crake's own release
RPM is no longer signed; the repository signs what it serves. The
README's "Installing" has the commands. **The AUR (2026-10-13):** the
release's `aur` job runs `scripts/aur-publish.sh <version>` (the PKGBUILD
with the version and the tag's source tarball's real checksum, its install
script and a .SRCINFO made in an Arch container, where makepkg checks the
checksum; pushed to ssh://aur@aur.archlinux.org/crake.git with the secret
AUR_SSH_KEY; `--dry-run` pushes nothing). Skipped while there's no secret:
AUR registration was closed that day (bot sign-ups); once the user has an
account with the key, publish the current release by hand with the
script, then releases keep it current. "Start at login" on macOS is
the app's own login agent, registered with SMAppService (from 0.7; a
LaunchAgent in ~/Library/LaunchAgents before).

## A newer Crake (built 2026-10-14; design/proposals/new-version.html)
`versions.rs`. (1) craked's D-Bus `Version` is newer than the app (a
package updated it; the app reads it when it connects, and on the system
bus it watches craked's name change owner (`client::owner_changes`) and
reads everything again: until 2026-10-15 a craked restarted by the
package went unnoticed, since the bus connection stays up, and the notice
never came; the tests' private socket drops, so they hadn't seen it):
the panel says "Crake was updated to X. Restart it to
finish."; Restart starts the new binary (Linux: the path without
" (deleted)") with `CRAKE_RESTARTING`, which waits up to 10 s for the old
one's single-instance socket or pipe to go (`single::taken`), and quits.
(2) Where nothing else updates Crake (`updated_elsewhere`: not the
dorhawk apt/dnf repository, pacman, the Nix store, Homebrew's cask),
once a day (looked at hourly, the time in app.json) the app asks
https://packages.dorhawk.software/versions.json with the system's curl and
says "Crake X is out. You have Y." with Download (the release page) and
Not now (`version:X`, so the next version is said again). On by default
(decided by the user: "automatic updates are amazing"); Settings → General
"Tell me when there's a new version" switches it off and is only shown
where it applies.
