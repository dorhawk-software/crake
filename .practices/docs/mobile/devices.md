# Mobile: devices, test accounts and walks

What may run on which phone or emulator, how a feature is proven on a real
device, and how what happens on people's phones comes back as tests.

Links point at commits in [Obálka](https://github.com/dorhawk-software/obalka).

## Never test on a device that holds real data

**What.** Development and tests use a dedicated demo device or emulator.
A device signed in to real accounts is never wiped, reset or used for a
test run.

**Why.** An Obálka emulator holding the owner's real accounts was wiped by
mistake on 2026-09-26, for a request to start Android clean. A demo
emulator (with a demo PIN) has been used for every test since.

**Apply.**
1. Name the demo device so it reads as one (`<App>_Demo`).
2. Write the real device's name in the project's local notes as hands-off.
3. "Start clean" means wiping the demo device, or making a new one.

**Check.** Every device command in the docs names the demo device.

## The service's test environment, never production

**What.** Features are developed against the service's official test
environment. Each stored account remembers which environment it belongs
to, and the app marks test accounts on screen.

**Why.** In Obálka, signing in to a production account is a legal act. Its
constitution makes the test environment the rule, and a banner marks test
accounts. A test credential tried against production fails with a
misleading message, so the environment is chosen on the sign-in form.

**Apply.** Test-account passwords never go in the repository: take them
from environment variables (a store's review account included). Write
test-account IDs only in local notes.

**Check.** `git grep` for the test environment's account IDs finds
nothing.

## A feature is done once it is walked on a device

**What.** A feature is done when it has been used on a real phone or
emulator, on each platform. The record is dated per platform: "walked on
an iPhone 2026-11-10", "not walked on a device", "walk owed (Android)".

**Why.** Walks find what tests can't. One Obálka walk on an iPhone found
twelve problems
([94efeca](https://github.com/dorhawk-software/obalka/commit/94efeca)).
Others found a line cut mid-word, a share sheet that opened a file instead
of sharing it, and a footer hidden behind the keyboard on Android 15.

**Apply.** Keep the records in one place per platform, the project's
[tried-by-people checklist](../common/claude-md.md#tried-by-real-people-a-checklist-per-platform),
not scattered across feature task lists.

**Check.** The checklist names, for each feature, the last walk on each
platform.

## A live switch draws what a fresh start draws

**What.** A setting that changes the look while the app runs (the
theme, the language, the text size) is walked both ways: switched inside
the running app, and from a fresh start in that setting. The same screen
is the same, pixel for pixel, outside the status bar.

**Why.** In Obálka the "Testovací" pill on a test box read "Testovac"
after a switch to the dark theme, and was whole after a restart
(2026-11-16). The owner had seen it two days before it was found. Its box
kept its size; Android drew the text a hair wider and lost the last
glyph. Every walk and check until then looked at fresh starts.

**Apply.**
1. Shoot each screen of the walk after a fresh start in the setting, into
   one folder. On Android: `adb shell am force-stop`, start the app, and
   `adb exec-out screencap -p > fresh/NAME.png`.
2. Switch the setting inside the running app (or, with the app following
   the system, `adb shell cmd uimode night yes`; on an iOS simulator,
   `xcrun simctl ui booted appearance dark`), and shoot the same screens,
   with the same names, into a second folder.
3. Compare them. A difference is a fault the live switch leaves: a
   measured box, a cached style or face that kept its old value.

**Check.** `.practices/bin/check-shots --skip-top PX fresh/ live/` lists
every region that differs (`7×21 px differ at 636,1766`) and fails on
any; `--skip-top` leaves out the status bar's clock. Shooting the
screens is the project's own walk.

## A crash report from the field becomes a test

**What.** An error seen in crash reports from a beta build becomes a test
in the same commit as the fix.

**Why.** A TestFlight build reported a Keychain error raised while the
phone was locked. It became two tests: the locked phone isn't reported,
and every other Keychain failure still is
([451fe8f](https://github.com/dorhawk-software/obalka/commit/451fe8f)).

**Apply.** Copy the error's exact type and message into the test's fake.
Cite the report's date and build in the test's header.

**Check.** Each fixed crash-report issue has a test naming it.

## Expected refusals from the OS aren't reported

**What.** When the OS refuses for a normal reason (a locked phone's
Keychain, a notification permission not yet answered), the app treats it
as "not available now" and doesn't report it as a crash.

**Why.** Obálka reported both as errors, so crash reports filled with
noise that hid real failures
([451fe8f](https://github.com/dorhawk-software/obalka/commit/451fe8f),
[87a249f](https://github.com/dorhawk-software/obalka/commit/87a249f)).
See [common/crash-reports.md](../common/crash-reports.md).

**Apply.** Match the refusal by its exact code. Report every other code.

**Check.** A test per refusal shows it unreported, and one shows a
different code still reported.

## Config that native tools rewrite is held by tests

**What.** Settings in native files that tools rewrite silently are
asserted by tests: Info.plist keys, Android backup rules, the iOS device
family, entitlements.

**Why.** A regenerated Info.plist drops the encryption-export key, and then
every TestFlight build stops at Apple's export question again (Obálka
owner decision, 2026-11-10). On Android 12 and later, `allowBackup="false"`
alone still allows device-to-device migration.

**Apply.** Parse the file in a test and assert the key's value. Say in the
test's header that it checks the setting, and that a device walk checked
the behaviour.

**Check.** Change the key by hand: the test fails.
