# Rust: GPUI desktop apps

What applies to apps built on GPUI (directly or through `gpui-kit`): build profiles that keep frames smooth in development, headless window tests, a journey suite that drives the real app shell, a demo mode, window buttons on macOS, CI packages, and how to keep a patched GPUI current.

Links point at commits in [Crake](https://github.com/dorhawk-software/crake).

## Dev profile: optimise a little

**What.** In development builds, the app's own code builds at opt-level 1 and its dependencies at opt-level 2.

**Why.** Unoptimised GPUI stutters. In tern, building the inbox's elements took about 40 ms a frame, and animations ran at about 20 fps.

**Apply.**
```toml
[profile.dev]
opt-level = 1

[profile.dev.package."*"]
opt-level = 2
```
For headless test binaries in CI, `CARGO_PROFILE_DEV_OPT_LEVEL=0` builds faster, and nothing needs to look smooth there ([8bab495](https://github.com/dorhawk-software/crake/commit/8bab495)).

**Check.** Frame timing (below) in a dev build stays under the frame budget.

## Headless window tests

**What.**
- Every window has a `tests.rs` using `#[gpui_kit::test]` and the test-window helpers (click, type, press, scroll).
- An element is clickable in tests once `.test_support()` is its last call. Without the dev feature, that call does nothing.
- GPUI's `test-support` feature is on only in dev-dependencies.

**Why.** Every window in Crake got tests on the first day ([20a738b](https://github.com/dorhawk-software/crake/commit/20a738b), [6beb084](https://github.com/dorhawk-software/crake/commit/6beb084)). It has 153 `.test_support()` calls.

**Apply.**
- Tests that do real I/O call `cx.executor().allow_parking()`.
- Nothing of the machine leaks in: fixed discovery results, and prefs in a scratch mode that writes nothing.
- Tests see no monitors ([1ba22e2](https://github.com/dorhawk-software/crake/commit/1ba22e2)).

**Check.** A review question for a new window: where is its `tests.rs`?

## The journey kit

**What.** `main()` is `start(cx, Setup)`. The journey suite runs that real app shell on GPUI's headless test platform, through a small kit:
- `until(cx, what, deadline, pred)`
- `click(id)`
- `type_into(id, text)`
- `wait_window::<V>()`
- `settle()`

**Why.** See [common/testing.md](../common/testing.md#a-journey-suite-is-the-regression-suite) ([7a28ba2](https://github.com/dorhawk-software/crake/commit/7a28ba2)). The test platform never sends the next frame or resize events by itself, so `settle()` draws frames by hand and calls `simulate_window_resize`. Windows that fit their content need a fixed tall size in tests.

**Apply.**
- Copy the kit from Crake's `journey_kit.rs` until it is published (planned as a crate, or an addition to gpui-kit).
- `<APP>_JOURNEY_DEBUG=1` prints each click target's bounds and whether it is visible.

**Check.** CI runs the journey on every system.

## Demo, show and appearance switches

**What.** Environment variables make the app show the design's sample data, open a given window, and pin the appearance:
- `<APP>_DEMO=<variant>`: sample data, with no network, keyring or daemon.
- `<APP>_SHOW=<window>[:<arg>]`: open that window at start.
- `<APP>_APPEARANCE=light|dark`.
- `<APP>_MOTION=slow|off`.
- A pseudo locale.

Each variable is parsed once, into an enum that refuses unknown values with the list of valid ones.

**Why.**
- They let the app be compared side by side with the design's mockups, and filmed.
- In Crake they grew as scattered string matches in `main.rs`. An unknown value silently fell back, and nothing listed them.

**Apply.**
```rust
enum Show { Settings, Log, Detail(String) }
impl FromStr for Show { /* "settings" | "log" | "detail:<x>" ; else Err(list of valid values) */ }
```
Keep a table of the variables in the project's docs.

**Check.** `<APP>_SHOW=nonsense` exits with the list of valid values.

## Frame timing

**What.** A switch prints frame timings, so performance claims come from measurements.

**Why.** tern's `TERN_BENCH=1` (with `ZED_MEASUREMENTS=1`) is the template. Runs vary by about ±30%, so compare medians of several runs. Use a frame-pointer build for `perf`.

**Apply.** Add `<APP>_BENCH=1`, which logs build, layout and paint time per frame and a summary on exit. Document how to profile.

**Check.** A "faster" commit quotes before and after medians.

## Window buttons on macOS

**What.** The practice is [ui/window-buttons.md](../ui/window-buttons.md):
on a Mac the system draws its buttons and the app's header makes room. In
GPUI that's one module (Crake's `chrome.rs`): the buttons' position, the
width they take, a header helper, and a switch tests can set.

**Why.** With `appears_transparent: true` and no position, macOS draws its
buttons wherever its default puts them, over the app's header (Crake's
Cancel, [407cf71](https://github.com/dorhawk-software/crake/commit/407cf71)).
Titles that only started after them still stood 4 px above them
([c75a781](https://github.com/dorhawk-software/crake/commit/c75a781)).

**Apply.**
```rust
pub struct Chrome { pub system_buttons: bool } // tests set it either way
impl Global for Chrome {}

pub fn system_buttons(cx: &App) -> bool {
    cx.try_global::<Chrome>()
        .map_or(cfg!(target_os = "macos"), |c| c.system_buttons)
}

pub const HEADER_HEIGHT: f32 = 56.;
const BUTTON: f32 = 14.;        // macOS's buttons are this tall
const BUTTONS_LEFT: f32 = 18.;  // window edge to the first button
const BUTTONS_SPAN: f32 = 60.;  // three 14 px buttons 9 px apart (macOS 26)
const BUTTONS_CLEAR: f32 = 18.; // as much after them as before them
/// The room every header leaves at its left on a Mac: 96 px.
pub const BUTTONS_WIDTH: f32 = BUTTONS_LEFT + BUTTONS_SPAN + BUTTONS_CLEAR;

/// Centred on a header HEADER_HEIGHT tall.
pub fn traffic_lights() -> Option<Point<Pixels>> {
    Some(point(px(BUTTONS_LEFT), px((HEADER_HEIGHT - BUTTON) / 2.)))
}

/// A header on the buttons' line: nothing in it starts above them.
pub fn on_buttons_line<E: Styled>(header: E) -> E {
    header.h(px(HEADER_HEIGHT)).py(px(0.))
}
```
Every window's options use it:
```rust
titlebar: Some(TitlebarOptions {
    title: Some("Traffic".into()),
    appears_transparent: true,
    traffic_light_position: chrome::traffic_lights(),
}),
```
In the header: when `system_buttons(cx)`, a left padding of
`BUTTONS_WIDTH`, no close button of the app's own, and a top-left action
(Cancel) moved to the right; the header goes through `on_buttons_line`.

**Check.** A test per window with the system's buttons on (Crake's
`traffic_view/tests.rs`):
```rust
#[gpui_kit::test]
fn the_title_is_level_with_the_systems_buttons(cx: &mut TestAppContext) {
    let w = open(cx);
    cx.update(|cx| cx.set_global(chrome::Chrome { system_buttons: true }));
    let title = cx
        .update_window(w.window, |_, window, cx| {
            window.refresh();
            window.render_frame(cx);
            window.find("traffic-title").bounds()
        })
        .unwrap();
    assert!(f32::from(title.origin.x) >= chrome::BUTTONS_WIDTH);
    let middle = f32::from(title.origin.y + title.size.height / 2.);
    let line = chrome::HEADER_HEIGHT / 2. + 1.;
    assert!((middle - line).abs() <= 1., "title at {middle:.1}, buttons at {line:.1}");
}
```
and one that `window.try_find("close")` is `None` there. The other tests
set `system_buttons: false`, so they run the same on every system.

## Keyboard shortcuts per system

**What.** The practice is [ui/keyboard-shortcuts.md](../ui/keyboard-shortcuts.md):
bind the system's main modifier, and show shortcuts the way each system
writes them. In GPUI, the binding is `secondary-`, and the label comes
from a small function of the app's own.

**Why.** `ctrl-k` in a `KeyBinding` means Control on every system, so on
a Mac ⌘K did nothing (tern, 2026-11-11). GPUI's own `Display` for a
keystroke is meant for developers. Off a Mac it prints `ctrl-K`, and on a
Mac it shows Control as `^`, not ⌃.

**Apply.**
```rust
cx.bind_keys([
    KeyBinding::new("secondary-k", OpenSearch, None), // ⌘K on a Mac, Ctrl+K elsewhere
    KeyBinding::new("e", MarkDone, Some("List")),    // the same everywhere
]);

#[derive(Clone, Copy)]
pub enum System { Mac, Other }

impl System {
    pub const fn current() -> Self {
        if cfg!(target_os = "macos") { Self::Mac } else { Self::Other }
    }
}

/// "⇧⌘K" on a Mac, "Ctrl+Shift+K" elsewhere.
pub fn label(keys: &Keystroke, system: System) -> String {
    let m = &keys.modifiers;
    let key = match (keys.key.as_str(), system) {
        ("enter", System::Mac) => "↩".into(),
        ("backspace", System::Mac) => "⌫".into(),
        ("escape", System::Mac) => "⎋".into(),
        ("tab", System::Mac) => "⇥".into(),
        ("enter", System::Other) => "Enter".into(),
        ("escape", System::Other) => "Esc".into(),
        (k, _) => k.to_uppercase(),
    };
    match system {
        System::Mac => [(m.control, "⌃"), (m.alt, "⌥"), (m.shift, "⇧"), (m.platform, "⌘")]
            .iter().filter(|(on, _)| *on).map(|(_, s)| *s).chain([key.as_str()]).collect(),
        System::Other => [(m.control, "Ctrl"), (m.alt, "Alt"), (m.shift, "Shift")]
            .iter().filter(|(on, _)| *on).map(|(_, s)| *s).chain([key.as_str()])
            .collect::<Vec<_>>().join("+"),
    }
}
```
A tooltip gets its keys from the binding. Use `window.bindings_for_action`,
or keep the bindings in one table the app reads. The translated text has
only a `{ $keys }` placeable.

**Check.**
```rust
#[test]
fn a_shortcut_reads_as_each_system_writes_it() {
    let k = Keystroke::parse("ctrl-shift-k").unwrap();
    assert_eq!(label(&k, System::Other), "Ctrl+Shift+K");
    let k = Keystroke { modifiers: Modifiers { platform: true, shift: true, ..Default::default() },
                        key: "k".into(), key_char: None };
    assert_eq!(label(&k, System::Mac), "⇧⌘K");
}

#[test]
fn no_translation_names_a_modifier() {
    for (file, text) in locales() {
        for word in ["Ctrl", "Alt", "Cmd", "Option", "⌘", "⌥", "⌃", "⇧"] {
            assert!(!text.contains(word), "{file} names {word}; use {{ $keys }}");
        }
    }
}
```

## CI packages for GPUI on Linux

**What.** GPUI links X11, Wayland, keyboard, font and Vulkan libraries.

**Apply.** `actions/rust-setup` with `gpui: true` installs:
```text
build-essential pkg-config cmake clang
libxcb1-dev libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev
libfontconfig-dev libfreetype-dev libvulkan-dev
```
A real window in CI runs on Xvfb with Mesa's lavapipe in a container with no network ([ca48892](https://github.com/dorhawk-software/crake/commit/ca48892), [900f0b9](https://github.com/dorhawk-software/crake/commit/900f0b9)).

**Check.** The CI job builds with no other apt step.

## Dependabot leaves GPUI alone

**What.** Dependabot ignores `gpui-kit*`, `gpui-pre*` and `gpui-fast*`. GPUI moves together with its patch and the app's API changes, by hand or by the update bot.

**Why.** A grouped bump can quietly drop a vendored patch. GPUI updates also break APIs, as GPUI 0.9's scroll rename did ([01ff76e](https://github.com/dorhawk-software/crake/commit/01ff76e)).

**Apply.** It's in [templates/dependabot.yml](templates/dependabot.yml) as an ignore list.

**Check.** No Dependabot pull request touches GPUI.

## Keeping a patched GPUI current

**What.** When the app needs a change GPUI doesn't have yet:
- The patched crate is vendored and wired in with `[patch.crates-io]`.
- A `.patch` file holds the canonical diff, and a `*-PATCH.md` explains each change.
- An update script re-applies the patch to the newest GPUI each week. If the patch applies in reverse, upstream has taken it, so the vendored copy is dropped.
- A weekly workflow opens a pull request, or an issue when the patch no longer applies.
- A lint step fails if Cargo.lock doesn't use the patched copy.

**Why.** A `[patch]` that no longer matches is only a Cargo warning. The unpatched crate would ship with every test green ([6b0026c](https://github.com/dorhawk-software/crake/commit/6b0026c)). A pull request opened with `GITHUB_TOKEN` doesn't start CI, so the workflow dispatches CI on the branch itself.

**Apply.**
- Copy Crake's `scripts/update-gpui.sh`, `scripts/check-gpui-patch.sh` and `.github/workflows/gpui.yml` (to be generalised in a later phase).
- Put the check in `.practices/lint.d/`.
- Offer the change upstream.

**Check.** The lint step, plus the weekly workflow's pull request or issue.
