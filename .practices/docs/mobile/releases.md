# Mobile: releases and the stores

How an iOS and Android app goes from a tag to TestFlight and Google Play:
versions from the tag, build numbers from the CI run, signing that leaves
nothing behind, the artefact checked before a store sees it, and store
promotion left to a person. The language-free release rules are in
[common/releases.md](../common/releases.md).

Links point at commits in [Obálka](https://github.com/dorhawk-software/obalka),
a React Native app on TestFlight and Google Play.

## The version comes from the tag; native files are never edited

**What.** The marketing version is the tag without its `+N`. It is passed
to the build (`MARKETING_VERSION` for Xcode, a Gradle property for
Android). The native project files keep a placeholder (1.0, 1). The
release workflow refuses a tag that disagrees with `package.json`.

**Why.** Xcode and React Native upgrades rewrite native project files
freely, and a version edited in three places drifts. Obálka checks tag,
`package.json` and the app's own version constant against each other.

**Apply.**
```sh
xcodebuild ... MARKETING_VERSION="$VERSION" CURRENT_PROJECT_VERSION="$BUILD"
./gradlew bundleRelease -PappVersionName="$VERSION" -PappVersionCode="$CODE"
```
In `build.gradle`, read those properties with a fallback for local builds.

**Check.** `git diff vX..vY -- ios/*.xcodeproj android/app/build.gradle`
shows no version change between releases.

## Build numbers come from the CI run

**What.** iOS build = the workflow's run number. Android versionCode = run
number × 100 + attempt. Both only go up.

**Why.** Both stores refuse a build number that doesn't rise, and a
half-finished run (one store uploaded, the other failed) must not reuse a
number already spent.

**Apply.** Take them in the first job and pass them to both builds.
Caveat: the run number counts per workflow file. Renaming or recreating
`release.yml` restarts it, and the stores then refuse every build. Write
that next to the workflow's name.

**Check.** The stores' build list rises by one per release.

## Another build of the same version: `+N`

**What.** Releasing the current version again tags `vX.Y.Z+N`
(SemVer build metadata). The store shows the same version with a new
build number, and the changelog section takes the new lines.

**Why.** The owner wanted testers to see 1.0.0 (1), (2), (3) while a
release took shape
([800a65b](https://github.com/dorhawk-software/obalka/commit/800a65b)).

**Apply.** The release script compares the requested version with the
last tag and adds `+N`. It checks that the last tag was pushed: in Obálka,
a failed push left a local tag, and the rerun would have made `+2` and
never published the first.

**Check.** `git ls-remote --tags origin` has every tag the script made.

## Release uploads are never cancelled

**What.** The release workflow's concurrency group has
`cancel-in-progress: false`.

**Why.** A cancelled upload can spend a build number on one store and not
the other. CI runs can cancel each other; a release can't.

**Apply.**
```yaml
concurrency:
  group: release-${{ github.ref }}
  cancel-in-progress: false
```

**Check.** The release workflow has no `cancel-in-progress: true`.

## A store whose secrets are missing is skipped, by name

**What.** Each store's job starts with a step that lists the secrets it
needs, prints the missing ones as a warning, and sets `ready=false`.
Every later step checks `ready`.

**Why.** One store is often set up before the other (Apple before Google,
in Obálka). Skipping with the missing names says what to add; failing
would block the other store; skipping silently hides it.

**Apply.** `if: steps.ready.outputs.ready == 'true'` on every upload step.

**Check.** Remove one secret in a fork: the run warns, naming it.

## Signing in a throwaway keychain

**What.** The iOS job imports the distribution certificate (a `.p12`
secret) into a keychain made for that job, with a random password, and
deletes it, the API key and any credentials file in an `if: always()`
step. The Android keystore is decoded to the runner's temp folder and
removed the same way.

**Why.** On Obálka's first release, Apple refused every binary as
"Invalid Signature": Apple's cloud-managed certificate can't sign for an
account holder whose name has diacritics, because the name is stored in
two Unicode forms (NFC and NFD)
([4f59df8](https://github.com/dorhawk-software/obalka/commit/4f59df8)). A
regular certificate in a job keychain works, and nothing outlives the job.

**Apply.**
1. `security create-keychain`, `import`, `set-key-partition-list`, then
   check that the identity is present before building.
2. Archive with automatic signing through the App Store Connect API key,
   so no provisioning profiles are stored.
3. Write the certificate's expiry date in the release docs.

**Check.** The cleanup step runs on a failed build too (look at a red
run's log).

## Check the artefact before the store sees it

**What.** Before upload, a step checks the built `.ipa` or `.aab` for what
must be inside it and what must not: optional native parts present,
bitcode absent, every architecture there.

**Why.** An Obálka Android release once shipped without its native
transfer library: the feature hid itself, as designed, so nothing failed.
Apple also refuses bitcode, which one native toolchain embeds by default.

**Apply.** A script that looks for the native module's symbols in each
architecture and runs `otool -l | grep __LLVM` on iOS. Fail the job on
any miss.

**Check.** Build once without the optional part: the check fails.

## Store notes come from the changelog, limits checked after folding

**What.** Release notes for GitHub, TestFlight, the App Store and Google
Play are made from the version's changelog section, per language. Each
store's limit is checked on the final text: Google Play 500 characters per
language, Apple 4,000.

**Why.** Obálka's release script checked the changelog, then folded the
new lines into the section. The fold pushed the Czech notes to 506
characters, and only the workflow refused it, after the tag was public
([815c2b3](https://github.com/dorhawk-software/obalka/commit/815c2b3)).

**Apply.** Check again after every write. On failure, restore the files
and stop before tagging.

**Check.** A section of 501 characters fails the release script, not the
workflow.

## Every release goes to TestFlight testers; promotion is by hand

**What.** Each release is added to the external TestFlight group and
submitted for beta review automatically. Submitting to App Review, and
promoting on Google Play, are done by a person, for a build the owner has
used on a phone.

**Why.** The owner: a release warrants a beta version
([a4eefc8](https://github.com/dorhawk-software/obalka/commit/a4eefc8)).
Before the first approval, an App Store version only accepts builds of its
own version string. Obálka renamed its unsubmitted 1.0.0 to 1.1.0, so the
build the owner tested was the one submitted.

**Apply.**
1. Wait for Apple's processing with an honest timeout (Obálka: 60 min).
2. Treat "another build in review" as a warning that names the follow-up
   command.
3. Write the promotion rule in the release docs, not only in an agent's
   memory.

**Check.** No workflow step submits for App Review.

## Google Play: the first release is manual

**What.** The workflow builds a signed `.aab` and keeps it as an artefact.
The first release of a new app is uploaded by hand in the Play Console.
Uploads by API go to the internal track, as drafts until the app leaves
draft.

**Why.** Google's API can't create an app's first release. A new personal
developer account also needs a closed test with 12 testers for 14 days
before production.

**Apply.** Gate the upload on a repository variable (`PLAY_UPLOAD=true`),
set once the first release exists. Plan the closed test before the
release date.

**Check.** The release docs say which state the Play listing is in.
