# Releases

A release is one command, and it refuses until it is safe. It releases only
a commit that passed CI on main, takes its text from the changelog, sets
the version everywhere, and can be re-run after an interruption. The
release workflow checks all of it again. The shared release scripts arrive
in phase 2. Until then, the reference is Crake's `scripts/release.sh` and
`.github/workflows/release.yml`.

## Only a green main is released

**What.** The release script refuses to start until CI on main's latest
commit has finished successfully: every job, every OS. Then it tags
exactly the commit CI passed, not whatever HEAD is by then.

**Why.** Two incidents.
- Crake's script tagged HEAD after waiting for CI. A commit made while it
  waited would have shipped untested
  ([1b4096b](https://github.com/dorhawk-software/crake/commit/1b4096b)).
- For 0.7.0, the release started about 15 minutes after a push. The
  version commit cancelled the CI run still going below it, and a new
  test then failed on Windows and macOS on the release commit itself
  ([f5a4efc](https://github.com/dorhawk-software/crake/commit/f5a4efc),
  asked for 2026-10-14).

Locally only Linux tests run. Windows and macOS tests exist only in CI.

**Apply.** Before releasing: `gh run list --workflow ci.yml --branch main
--limit 1` must show success. Don't push anything else to main while the
script waits; CI's cancel-in-progress would cancel the run it waits for.

**Check.** The script refuses otherwise. The release workflow's first job
runs `gh run list --workflow ci.yml --commit $SHA --status success` and
fails on an empty result.

## One command from Unreleased to the tag

**What.** `scripts/release.sh [--dry-run] X.Y.Z`:
1. requires a clean tree with main equal to origin/main;
2. refuses an empty Unreleased section, or a change outside the
   changelog's kinds;
3. dates the section and sets the version in every file that carries it;
4. commits `<Product> X.Y.Z` with the notes as its body, and pushes;
5. waits for CI on that commit, then pushes an annotated tag on it.

Run again with the same version after an interruption, and it picks up
where it stopped.

**Why.** Crake's 0.2.0 was bumped and tagged by hand, and the release
asked for a script
([a7186e1](https://github.com/dorhawk-software/crake/commit/a7186e1),
2026-10-13). `release.sh 0.7.0 --dry-run` once released for real, so
`--dry-run` must come first and anything after the version is refused.
GNU `sed -i` broke the script on macOS; it edits with `perl -pi` now
([eb13f5b](https://github.com/dorhawk-software/crake/commit/eb13f5b)).

**Apply.** Run `--dry-run` first and read the diff. Run the real release
in the background; it waits for CI. Phase 2 makes the script take its
product name, repository, CI workflow, branch and version files (path and
pattern pairs) from configuration.

**Check.** `git show --stat vX.Y.Z` touches only version files and
CHANGELOG.md. The workflow refuses a tag whose version files disagree.

## The workflow re-checks, then publishes in safe order

**What.** On a `v*` tag:
1. The `notes` job checks that a dated changelog section exists, that
   every version file agrees with the tag, and that the tagged SHA passed
   CI. It writes the notes and the download guide.
2. The package builds run in parallel.
3. `publish` writes SHA256SUMS and runs `gh release create --verify-tag`.
4. Then come the package managers' jobs and `tidy`, which deletes the
   run's artifacts once everything is out.

Optional secrets mean "skip with a message", never "fail".

**Why.** The first release built every package and published nothing: the
checksum step got folders instead of files
([aed1687](https://github.com/dorhawk-software/crake/commit/aed1687)). For
0.15.0, a post-publish job ran a script without a checkout. The release
was up, but the cask, the apt repository and the AUR were then done by
hand ([9867560](https://github.com/dorhawk-software/crake/commit/9867560)).
Rule: everything needed after publishing is produced before it.

**Apply.** Produce every text file in the first job and pass it on as an
artifact. Name artifacts per run attempt. Use `concurrency: release` with
`cancel-in-progress: false`.

**Check.** The rehearsal below.

## A rehearsal that publishes nothing

**What.** Running the release workflow by hand (`workflow_dispatch`)
builds and tests everything and publishes nothing.

**Why.** Otherwise a change to the release workflow is first exercised at
a real release, with users waiting. Crake added the rehearsal along with
the per-architecture macOS build that cut 22 of 24 minutes
([7f45835](https://github.com/dorhawk-software/crake/commit/7f45835)).

**Apply.** Gate every publishing step on `github.event_name == 'push'`.

**Check.** After any edit to release.yml, run the rehearsal before the
next release.

## Slow external steps get honest timeouts

**What.** Waits on outside services have limits set from what was
measured. When a limit is hit, the step says which outcome it was: "not
finished yet" or "refused".

**Why.** Crake's first notarization ran out of its 30 minutes, because a
team's first submissions take hours
([1646ea7](https://github.com/dorhawk-software/crake/commit/1646ea7)). It
now waits up to two hours and prints Apple's log on a refusal.

**Apply.** Measure the step, then set the limit at a comfortable multiple.
Make the failure message distinguish slow from refused.

**Check.** The step's log on failure says which one it was.

## Dependencies: the platform's packages, or bundle and own the updates

**What.** Where a platform has a package manager, depend on its packages
(deb, rpm, Arch, Nix, Homebrew). Where it doesn't, bundle, and take
responsibility for following upstream security releases.

**Why.** Bundling system software into Linux packages means shipping old
copies with known holes. On Windows there is no package manager, so the
bundle is yours to keep current.

**Apply.** List the bundled components in the packaging docs, with where
each one's security announcements come from.

**Check.** In review of a packaging change: is anything bundled that the
platform's package manager provides?
