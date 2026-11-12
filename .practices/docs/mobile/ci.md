# Mobile: CI for iOS and Android

What a mobile app's CI builds, on which runner, with which pinned
toolchain, and what still runs locally. The general CI rules are in
[common/ci-caching.md](../common/ci-caching.md).

Links point at commits in [Obálka](https://github.com/dorhawk-software/obalka).

## Public: every platform on every push. Private: before every release

**What.** A public repository builds iOS and Android on every push, because
its minutes are free. A private one builds the platforms on tags, on
`wip/*` branches or on a self-hosted Mac, but iOS still compiles somewhere
before each release.

**Why.** While Obálka was private, macOS minutes cost money and nothing
built iOS. Types, lint and tests passed while the iOS side didn't compile;
the first iOS build, on 2026-09-16, failed for two reasons nobody had
seen. The private-repository budget rule
([common/ci-caching.md](../common/ci-caching.md#private-repos-ci-minutes-are-a-budget))
would have kept it there.

**Apply.** Public: a macOS job in the main CI. Private: a `wip/*` push
trigger to try a branch on the runner, and the local gate before a tag
(below).

**Check.** The last release's commit has a green iOS and Android build,
in CI or in the release log.

## A compile check: device SDK, Debug, unsigned

**What.** CI compiles the iOS app for the device SDK
(`-destination generic/platform=iOS`), in Debug, with
`CODE_SIGNING_ALLOWED=NO`. Android builds the release bundle's native
parts.

**Why.** The simulator SDK lacks some device-only constants. A camera
library used one, and only a device-SDK build failed on it. Unsigned keeps
signing secrets out of every push.

**Apply.** A job with no secrets, separate from the release workflow.
Add a Release-configuration build and the JavaScript bundle once a month,
or before a tag: a Debug build doesn't bundle JavaScript.

**Check.** The push CI has an iOS job with no secrets in its environment.

## Pin the runner image and Xcode, with the reason

**What.** The macOS runner is a named image (`macos-15`), and Xcode is
pinned with `DEVELOPER_DIR`. A comment says why that version: which
dependency needs it, and since when.

**Why.** The image's default Xcode was 16.4. A camera library named a
CoreVideo constant that exists only from the iOS 26 SDK, so the build
compiled for about twenty minutes and then failed on one file. Probed on
2026-09-16: every Xcode 16 SDK lacked it, every 26 had it.

**Apply.**
```yaml
runs-on: macos-15
env:
  # Xcode 26: the camera library needs the iOS 26 SDK (2026-09-16).
  DEVELOPER_DIR: /Applications/Xcode_26.3.app/Contents/Developer
```
Write the pin once (a composite action or a reusable workflow), not in
every workflow. When the local Mac moves to a newer Xcode, note the
difference.

**Check.** `grep -rn DEVELOPER_DIR .github` finds one definition.

## Toolchains come from the project's own files

**What.** CI reads each toolchain's version from the file the build already
uses: Go from `go.mod`, the Android NDK from `android/build.gradle`, Node
from `package.json`'s `engines` or `.nvmrc`, Java from one documented
setting.

**Why.** A version in two places drifts. Obálka's NDK install script reads
the version from Gradle; its Go setup uses `go-version-file`. A wrong JDK
failed a native module's build with a message that named nothing useful,
so the JDK is pinned too (17).

**Apply.** `actions/setup-go` with `go-version-file: native/go.mod`; a
script that greps the NDK version from Gradle. Never `@latest` for a tool
the build runs.

**Check.** No toolchain version is written in a workflow when a project
file already names it.

## The local gate builds every platform before a tag

**What.** The release script builds iOS and Android on the developer's
Mac before it tags, after the usual gate. The everyday local gate stays
fast (types, lint, tests, about 25 s); the platform builds run only before
a release.

**Why.** Obálka had no local gate at all: development had been on Linux,
where iOS can't be built, so CI was the only check. The owner chose, on
2026-11-12, to build every platform locally before a release, with CI
still building iOS on every push.

**Apply.** In the release script, after the checks and before the tag:
`xcodebuild` (device SDK, Debug, unsigned) and `./gradlew assembleRelease`
(it also bundles the JavaScript, which no push build checks), each followed
by the artefact check for every architecture the app ships. Stop on
failure; nothing is tagged. Keep the build folders outside the repository.

**Check.** A release run's output shows both builds before the tag line.

## A build made as a check publishes nothing

**What.** A build run to check something (a local gate, a rehearsal, a
compile check) uploads nothing: no source maps or symbols to the crash
reporter, no store upload, no tag.

**Why.** The first run of Obálka's local release check built Android in
Release, and the crash reporter's Gradle plugin uploaded that build's
source maps with the token in a local properties file
(Obálka, 2026-11-12). Nothing broke, but an unreleased build now sits in
the crash reporter as if it were a release.

**Apply.**
1. In every check script, switch the uploads off explicitly
   (`SENTRY_DISABLE_AUTO_UPLOAD=true` for Sentry's iOS and Android
   build steps).
2. Read each build plugin's upload switch in its own code, not its docs
   ([crash-reports.md](../common/crash-reports.md)).
3. Run the check once and search its log for "upload".

**Check.** The check script's log has no upload lines.
