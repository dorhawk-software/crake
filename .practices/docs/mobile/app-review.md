# Mobile: App Review

What Apple's App Review refused, and the checks that keep each refusal
from coming back. Each rule here was a rejection of an iPhone app
(Screen Time limits, SwiftUI, XcodeGen) in November 2026. A rejection
costs days: the fix, a new build, and the queue again.

The app is private, so its commits aren't linked; the dates are. The
release rules are in [releases.md](releases.md), the permission
explanation in [lifecycle.md](lifecycle.md).

## Every rejection becomes a practice and a check before the resubmission

**What.** A rejection from App Review is fixed in the app, written here
as a rule, and given a check that would have caught it. All three land
before the build goes back to review.

**Why.** The same app was stopped three times in a week: for an
entitlement (13 Nov 2026), a button's words (16 Nov) and a crowded
screen (18 Nov). Apple found each one; no check did. A
fix with no check lets the next screen or target make the same mistake.

**Apply.**
1. Quote Apple's message and the guideline in the rule's Why.
2. Reproduce it first, where the reviewer saw it: the device, the text
   size, light or dark.
3. Write the check, break it once to see it fail, and wire it into the
   gate ([common/gates.md](../common/gates.md)).
4. In App Store Connect, a rejected item blocks "Resubmit to App Review"
   until it is marked resolved. Through the API: `PATCH
   /v1/reviewSubmissionItems/{id}` with `resolved: true`.
5. Reply in Resolution Center with what changed, in two or three lines.

**Check.** Review: the resubmission's commit range adds a rule here and
a check in the kit or the project's `.practices/lint.d/`.

## Every binary carries the entitlements its frameworks need

**What.** Every executable in the archive that links a framework gated
by an entitlement carries that entitlement: the app, and each extension
and widget. FamilyControls, ManagedSettings and DeviceActivity need
`com.apple.developer.family-controls`; HealthKit needs
`com.apple.developer.healthkit`; HomeKit `com.apple.developer.homekit`.

**Why.** On 13 Nov 2026, App Review's automated check stopped the app
before review could start: "the app uses Screen Time APIs without the
Family Controls entitlement". The app and four extensions had it. The
widget compiled the shared source folder, so its binary linked
ManagedSettings without it. Two days and a resubmission. Nothing in the
build failed: Xcode signs a binary with whatever entitlements it is given.

**Apply.**
1. Keep gated frameworks out of the shared code a widget compiles. Put
   the Screen Time calls in a folder only the targets that need them
   compile.
2. Or give each target that compiles them the entitlement, as the app
   did (an XcodeGen template per kind of target helps).
3. Before each upload, check the archive, not the project: a Swift
   package can link a gated framework too.

**Check.** `check-xcode` reads each target's sources and frameworks from
the `.xcodeproj` and finds a gated import without its entitlement. It
runs in lint, in claude-hook after an edit to the project, an
entitlements file or Swift, and in pre-commit. `check-ios-artefact`
reads the built `.xcarchive` or `.ipa` with `otool -L` and `codesign`.
claude-hook runs it after the agent's `xcodebuild archive` or
`-exportArchive`. Run it before every upload too: no kit release script
does that yet (ROADMAP.md).
`ENTITLED_FRAMEWORKS` in `.practices/config` adds pairs.

## The app is built for the devices it declares

**What.** Every target that ships (the app, each extension) is built for
the same device families, the ones the project says it supports. An
iPhone-only app says `TARGETED_DEVICE_FAMILY = 1` on every target, and
`UIDeviceFamily` in each bundle is `[1]`.

**Why.** Build 11 of the app shipped as an iPad app too, and App Store
Connect then demanded 13-inch iPad screenshots. XcodeGen's iOS preset
sets `"1,2"` on each target, which beats the project-level `"1"`. The
fix was to repeat `"1"` on every target.

**Apply.**
1. Set the family at the project level, and repeat it on every target
   when a generator writes its own.
2. Set `DEVICE_FAMILY` in `.practices/config` when the project level
   doesn't say it.
3. Check the archive's `Info.plist` files, not only the project.

**Check.** `check-xcode` compares each shipped target's
`TARGETED_DEVICE_FAMILY`, in each build configuration, with the declared
family. `check-ios-artefact` compares each bundle's `UIDeviceFamily`. They
run where the entitlement check runs, above.

## A button that leads to a permission prompt says Continue

**What.** The button on the app's own screen before a system permission
prompt says "Continue" or "Next". Never "Allow", "Grant", "Enable",
"Turn on" or "Give access": only the system's prompt asks. Swiping the
screen away still dismisses it.

**Why.** On 16 Nov 2026 App Review refused the app under Guideline
5.1.1(iv): "A custom message appears before the permission request, and
to proceed users press an 'ALLOW SCREEN TIME ACCESS' button. Use words
like 'Continue' or 'Next' on the button instead." Four buttons changed:
onboarding for oneself and for a child, a status card's "Allow", and a
notification primer. The primer's "Allow notifications" and "Not now"
became one Continue.

**Apply.**
1. The screen says, in one line, why the app needs the permission
   ([lifecycle.md](lifecycle.md#explain-before-the-permission-prompt-and-ask-once)).
2. Its one button says Continue, in each language: "Pokračovat",
   "Continuar", "Tiếp tục".
3. Delete catalog entries no code uses any more. Xcode marks them
   `"extractionState": "stale"` only after a build, and they still ship.

**Check.** `check-permission-words` reads the copy files and the code.
A whole string that is a grant verb and a permission is a finding
anywhere: "Allow notifications", "Povolit Čas u obrazovky", "Permitir
acceso a tus fotos". A bare "Allow" is one in a file that asks
the OS for a permission. It knows English, Czech, Spanish and
Vietnamese; `PERMISSION_VERBS` and `PERMISSION_NOUNS` add words. A
catalog entry marked stale is skipped; every other entry counts. A step
in a guide that names a setting is marked `practices-allow permission:
<reason>`. It runs in lint, claude-hook (edits, Stop) and pre-commit.
Whether a button with other words leads to a prompt is review.

## Every screen fits a small iPhone at a large text size

**What.** Every screen works in a 375 × 667 pt window, at the default
text size and at xxLarge, in light and in dark. Its main action is on
screen without scrolling: pinned at the bottom, never below the fold.

**Why.** On 18 Nov 2026 App Review refused the app under Guideline 4:
parts of it were "crowded, laid out, or displayed in a way that made it
difficult to use" on an iPad Air 11-inch (M3). An iPhone-only app on an
iPad runs in an iPhone SE-sized window, 375 × 667 pt. The reviewer had a
larger text size and dark mode. The onboarding was one VStack that
didn't scroll, taller than the window, so SwiftUI cut off both ends. The
step counter and Continue were off screen; app names read "Instag…". The
reviewer couldn't go on. The iPad simulator on iPadOS 26 opens iPhone
apps in a tall, resizable window, which hid it. An iPhone SE (3rd
generation) simulator showed it at once, even at the default text size.

**Apply.**
1. Every page scrolls. The main action and the choice sit in a bar
   pinned at the bottom (`safeAreaInset(edge: .bottom)`).
2. Decoration goes first when a page doesn't fit (`ViewThatFits(in:
   .vertical)`). Dense rows stack from xxLarge up.
3. A UI test on an iPhone SE simulator asserts the main button exists,
   `isHittable`, and lies inside the window, at xxLarge too.
4. Never trust the iPad simulator for compatibility mode; use an iPhone
   SE.

**Check.** `check-ui-tests` fails when no test run names an iPhone SE
simulator, or none sets a large text size (`xcrun simctl ui <device>
content_size extra-extra-large`, or the launch argument
`-UIPreferredContentSizeCategoryName UICTContentSizeCategoryXXL`). It
runs in lint, claude-hook and pre-commit. That each screen is reached
and its button asserted, in dark too, is the tests' job and review's.

## Xcode's accessibility audit runs on every screen, the first-run ones too

**What.** A UI test runs `performAccessibilityAudit()` on every screen,
including onboarding and other screens only a new user sees.

**Why.** The audit ran on the app's tabs, not on its onboarding. Once it
did, it found fixed-size type and grey text on a gradient there: the
screens the reviewer saw first.

**Apply.** Launch the app fresh for the first-run screens, walk each
step, and audit each one. Fix what it finds; an exception is a closure
that names the issue and says why.

**Check.** `check-ui-tests` fails when no UI test calls
`performAccessibilityAudit`. Whether every screen is audited is review:
list the screens next to the test.

## UI tests pin the app's language

**What.** Every UI test launches the app in a named language and locale:
`-AppleLanguages (en) -AppleLocale en_US`, or the app's own switch.

**Why.** On 18 Nov 2026, while the iPad fix was checked, UI tests that
found buttons by their words failed on a simulator set to Czech. They
passed on an English one: nothing pinned the app's language.

**Apply.** Put the arguments in the one helper that launches the app.
A test of another language passes that language.

**Check.** `check-ui-tests` fails on a Swift file that launches
`XCUIApplication()` without a match of `UITEST_LANGUAGE` (by default
`-AppleLanguages|-AppleLocale`; add the app's own switch). It runs in
lint, claude-hook and pre-commit. Espresso, Detox and Maestro tests
aren't read yet.
