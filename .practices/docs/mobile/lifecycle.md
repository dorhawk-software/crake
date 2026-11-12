# Mobile: lifecycle and screens

A phone app goes to the background, gets tapped twice, asks for
permissions and is shown in the app switcher. These rules keep each of
those from leaking data or doing something twice. The drawing rules are
in [ui/visual-craft.md](../ui/visual-craft.md).

Links point at commits in [Obálka](https://github.com/dorhawk-software/obalka).

## Modals close when the app goes to the background

**What.** Sheets, dialogs and menus close when the app leaves the
foreground.

**Why.** In Obálka, a modal drew above the lock screen's privacy cover, so
the app switcher could show a message while the lock was on.

**Apply.** A shared hook (`useCloseOnBackground`) on every modal,
listening to `AppState`.

**Check.** Open a sheet, go home, look at the app switcher: the cover
shows, not the sheet.

## The privacy cover lives where the OS calls it

**What.** The cover that hides the screen in the app switcher is shown and
hidden in the callbacks the OS actually calls. On iOS with scenes, those
are the scene delegate's.

**Why.** Under the iOS 27 SDK an app must adopt scenes. The app delegate's
hooks then stop being called, and the cover silently stopped appearing
([3cbb48f](https://github.com/dorhawk-software/obalka/commit/3cbb48f)).

**Apply.** Put the cover in `sceneWillResignActive` and
`sceneDidBecomeActive`. A test asserts the scene delegate has both.

**Check.** Walk it on a device after every Xcode upgrade.

## An action with no idempotency key runs once

**What.** A button whose action the server can't de-duplicate (sending a
message, paying) is guarded by a single-flight flag held in a plain
variable, not in component state.

**Why.** Obálka's server has no idempotency key: a second send is a second
official message. State updates are asynchronous, so two taps in one frame
both saw "not sending". The test helper presses the same handler twice in
one tick; two `fireEvent.press` calls only test a slow double tap.

**Apply.** `useSingleFlight` with a `useRef` flag; disable the button too.

**Check.** A test calls the handler twice synchronously; the server fake
sees one call.

## Explain before the permission prompt, and ask once

**What.** Before the OS asks for a permission (notifications, camera), the
app says in one line why it needs it. It asks only when the user starts
the feature, and only once.

**Why.** The OS prompt can be shown only once; a refusal then sends the
user to Settings. Obálka also stopped reporting the "not authorized yet"
state as an error
([87a249f](https://github.com/dorhawk-software/obalka/commit/87a249f)).

**Apply.** A small screen or sheet with the reason and a button that names
the action ("Turn on reminders"), then the OS prompt.

**Check.** Fresh install: no prompt appears until the feature is used.

## A cold-start tap is queued, not lost

**What.** A notification or link that launches the app is held until
navigation is ready, then handled.

**Why.** On a cold start, the tap arrives before the navigator exists.
Handled at once, it is lost, and the app opens on its home screen. Obálka
queues it.

**Apply.** A router that queues the first intent and replays it when the
navigator reports ready.

**Check.** Kill the app, tap a notification: the right screen opens.

## No layout jumps

**What.** Nothing moves when something appears late: a spinner, a sync
line, a badge, an error. Its space is reserved from the start, and a state
change keeps the same size.

**Why.** The owner strongly dislikes content that shifts. Reported on
2026-06-16: a sync spinner changed a row's height and moved its text.
Obálka's constitution made it a hard rule.

**Apply.**
1. Give late elements a fixed-height slot, shown or not.
2. Prefer a text status of constant line height ("Updating…" / "Updated
   just now") to a spinner.
3. Keep selected and unselected, loading and loaded, the same size.

**Check.** Watch each screen while it loads: nothing below the first
element moves.

## The largest text size and Reduce Motion

**What.** Every screen works at the system's largest text size, and every
animation yields to Reduce Motion. Touch targets are at least 44 pt, with
their extra hit area derived from the drawn size.

**Why.** An Obálka design audit (2026-09-09) found nine boxes that clipped
text at the largest font size. On an iPhone, the top of a digit was cut
off while Android and every test looked right.

**Apply.** Set `minHeight`, not `height`, on anything holding text. Check
`useReducedMotion` in every animation. Render the screen tests at a large
font scale.

**Check.** Turn on the largest text size and Reduce Motion, and walk every
screen.
