# Changelog

What changed for the projects that use the practices. Written by hand,
newest first, in the kinds common/changelog.md describes.

## 1.12.0 (2026-11-23)

### New

- **Proposal pages pass a checklist before review**
  (`ui/design-before-build.md`): each question beside its own images,
  before/after pairs within one phone screen, every option drawn at its
  best, every sentence true on its screen, each language's rules. One
  project's pages went through three review rounds that found what the
  list now catches first.
- **`check-page`** reads each proposal page (`UI_PAGES`:
  `design/proposals/` and `design/changes/`) at 375 and 390 px, as the
  owner reads it on a phone: sideways scroll, a one-letter word at a
  line's end where its language ties it (an option's "C" too), images
  past the screen or drawn wider than their pixels at 2x, the doctype,
  the charset and the language. It runs after edits, on Stop and before
  each commit. Pages read on a desktop set `PAGE_WIDTHS` or `UI_PAGES`
  in `.practices/config`.
- **A subset iterates; only the full run gates** (`common/gates.md`): a
  slow gate may run a few screens by name while you work, and says it
  was partial; the hooks run the full set. A shared view changed for one
  screen moved another, and only the full run saw it.
- **Cache by content, never by time** (`common/gates.md`): a render or a
  shot is reused only under a hash of everything it is made of, the app
  as built included. A screenshot match with the app unchanged fell from
  8.8 min to 35 s and boots no simulator. The commit proves the cache
  three ways before it is trusted.
- **A slow gate prints each phase's time** (`common/gates.md`), so a
  claim about its speed is measured.
- **A screenshot gate launches the app once** (`ui/design-before-build.md`):
  later screens are switched in place under a debug-only flag, each shot
  waits for the app's ready mark, and a guard compares a switched screen
  with a launched one, within 2 levels of dither. A shot fell from 7.47 s
  to 1.26 s.
- **A stuck renderer is asked once more; a failed comparison never is**
  (`common/testing.md`): a capture that never answers is rendered again,
  said, and a second miss fails naming the input.
- **Simulators run one at a time until the machine is measured** and
  **Don't edit a script while it runs** (`common/agent-working-rules.md`):
  three simulators at once drove a 64 GB laptop to a load average of 387
  and 103 °C; a script edited mid-run died after minutes of work.

## 1.11.0 (2026-11-22)

### New

- **The type is chosen for the product, never a tool's default**
  (`ui/identity-pass.md`): three or four faces and the current one made
  lighter, on the same screens with only the type changed, shown blind to
  real people, each with its cost. A rounded heavy face read as
  "AI-made"; the owner chose after a blind page.
- **Roles by meaning** (`ui/identity-pass.md`): the voice face takes what
  the product says, the interface face what you tap or scan and every time
  and number, in tabular figures; never by size. check-ui's new `tabular`
  rule measures each time's figures in its own font.
- **A face change keeps the ink** (`ui/identity-pass.md`): small text
  within 5 % of the agreed face's ink, measured.
- **Every face draws the product's own language** (`ui/identity-pass.md`):
  check-ui's new `fallback` rule asks Chrome which fonts it really drew
  each text in, and finds letters a face lacks drawn by a system font.
  Projects that let the system draw a sign on purpose (an arrow) mark it
  `data-allow="fallback: <reason>"`.

### Better

- **Czech copy ties one-letter words to the next** (`common/voice.md`):
  check-copy's new `one-letter-end` rule asks for a no-break space after
  k, s, v, z, o, u, a and i, so no line ends on one ("…často s /
  plačícím"). Existing Czech copy will have many findings: tie them
  where the copy is written (a generator or the strings catalog), not
  one by one.

### Fixed

- **Commits from a linked worktree** (`git worktree add`) were refused: in a hook git sets GIT_DIR, the kit took its own bin folder for the project's root, and check-private looked for the git-ignored terms in the worktree, which never has them. The hooks now check the tree being committed, with the main worktree's terms.

## 1.10.0 (2026-11-21)

### New

- **The owner answers one question at a time** (`ui/design-before-build.md`):
  open questions go in the agent's dialog, a few per round, each step
  with the page's link, the board's name and the recommendation first.
  The page with the boards is for looking.
- **Cross each new event with each state before asking**
  (`ui/design-before-build.md`): every pair the spec leaves undefined is a
  question in the same round, not the next one. One audit's 22 answers
  led to 8, 4 and then 3 more questions.
- **Every control records an event or only navigates**
  (`ui/design-before-build.md`): a choice that changes what happens next
  and records nothing is forgotten on restart and never synced.
- **Long gates run in the main session** and **Only the lock's own script
  waits for the lock** (`common/agent-working-rules.md`): a sub-agent
  waiting on a simulator suite is stopped as stalled, an orphan agent
  holds the shared lock, and a hand-written wait loop never takes over a
  stale lock.

### Better

- **Pixel by pixel** measures text line by line, and tuned type takes
  each line's tracking from that line's browser width: a per-style
  average left one line 1.3 pt short, and a paragraph's box hid another.
- **Review a proposal in its place** reads every option against the
  spec's text; an option that contradicts the spec says how the spec
  changes.
- **A rule a machine can check runs where the agent can't skip it**: a
  slow gate on Stop skips while its lock is held, and when nothing it
  checks changed since its last clean run.

## 1.9.2 (2026-11-19)

### Changed

- **A companion may say an inclusive "we"** (`common/voice.md`). When
  the product is a companion and "we" means it and the person together,
  the line stays, marked `practices-allow voice: inclusive we, <who>
  together`. The app, the company or a server still never says "we", and
  a promise about data still names who acts. `check-voice` flagged
  "Pojďme na to" (let's do this), a sleep-training owl's word to a tired
  parent; naming one actor would remove what the line is for.

## 1.9.1 (2026-11-18)

### Fixed

- **`check-voice` reads an Xcode app's English.** In a string catalog the
  English copy is each entry's key, and keys were never read: an app's
  "Contact us" and "kept our phones down" passed. A key is now its
  `sourceLanguage` copy when the entry has no localization of that
  language; a stale entry is skipped. A real quote is marked in the
  entry's comment (`practices-allow voice: <reason>`), where Xcode keeps
  it.
- **Czech "nevezme" and "přijme" are he, not we.** The third person of
  vzít and of the -jmout verbs ends in -me too; "žádný z nich ti už nikdo
  nevezme" was a finding.

## 1.9.0 (2026-11-18)

### New

- **Rounded corners don't clip** (`rust/gpui.md`): GPUI clips children
  to a rectangle, so a child that fills a rounded element's corner rounds
  that corner itself, with the parent's radius minus its border.
  `check-gpui-clip`, in the lint gate of a GPUI app, finds an element
  that rounds its corners and clips its children; a clip kept on purpose
  says why with `// practices-allow clip: <reason>`. A GPUI app's
  Settings pane showed a sidebar's square corners past its picture's
  rounded ones, and the HTML board, whose CSS clips round, looked right.
- **App Review** (`mobile/app-review.md`): three rejections of an iPhone
  app in a week, each now a rule with a check. Every rejection becomes a
  rule and a check before the build goes back to review.
- **`check-permission-words`: "Continue", never "Allow", before a
  permission prompt.** It reads the copy and the code, in English,
  Czech, Spanish and Vietnamese. App Review refused an "ALLOW SCREEN
  TIME ACCESS" button (Guideline 5.1.1(iv)). A string catalog's stale
  entries are skipped; the rest count, used or not.
- **`check-xcode`: each Xcode target that ships is built for the
  declared devices, with the entitlements its frameworks need.** It
  reads `project.pbxproj`, so it runs on Linux too. A widget that linked
  ManagedSettings without Family Controls was stopped by App Review's
  automated check; an earlier build shipped for iPad because XcodeGen
  set "1,2" on each target. Set `DEVICE_FAMILY` when the project level
  doesn't say it.
- **`check-ios-artefact`: the archive as App Store Connect sees it.**
  Each bundle's `UIDeviceFamily`, and each binary's linked frameworks
  against its signed entitlements (`otool -L`, `codesign`, macOS).
  claude-hook runs it after the agent's `xcodebuild archive` or
  `-exportArchive`. Run on the app's archives, it finds both faults in
  the builds App Review saw and none in the fixed one.
- **`check-ui-tests`: UI tests that run where the reviewer looks.** A
  file that launches the app pins its language (`UITEST_LANGUAGE`), the
  accessibility audit runs, and a test run uses an iPhone SE simulator
  at a large text size. An iPhone app on an iPad gets a 375 × 667 window;
  the onboarding's Continue fell off it.

### Changed

- **The button before a permission prompt says "Continue"**
  (`mobile/lifecycle.md`). The rule had asked for a button that names
  the action ("Turn on reminders"); App Review refuses that.
- **A native iOS project gets the mobile rules.** `bin/install` digests
  `mobile/` into `rules.md` for a `project.yml` or an `.xcodeproj` at the
  root, not only for `ios/` and `android/` folders.

## 1.8.0 (2026-11-18)

### New

- **Feedback while it works** (`ui/feedback.md`): work that takes over a
  second shows the product's own motion until its result shows, and a
  pressed control says what is happening ("Opening Enterprise
  Architect…"), shows that motion and takes no more presses until the
  outcome shows. `check-ui` measures it on the boards: a control marked
  `aria-busy="true"` must be disabled, show no pointer cursor, end its
  words in "…" and have something moving in it. The Windows-app runner
  showed a still egg for minutes of work, and its Open button gave no
  sign for the 10 to 30 s an app took to show, so it could be pressed
  again and again.
- **`check-ui` finds a loop that doesn't stop for reduced motion**
  (motion, `ui/visual-craft.md`, "Motion with a reason"): every board is
  measured with reduced motion on, and an animation still looping there
  is a finding, unless the loop is the content and is marked.
- **`check-proven`: a commit that changes a check says how it was
  proven.** Set `CHECK_FILES` to the project's own checks, and a commit
  that adds or changes one needs a `Proven:` line: what was broken, and
  which case failed (`common/gates.md`, "A new check is broken once,
  then trusted"). `check-commit-msg` runs it, so the commit-msg hook
  does.
- **A resize never shows a squashed frame** (`ui/visual-craft.md`):
  when a window or panel changes size, its content is laid out at every
  frame. `check-ui` (squash) finds words drawn scaled out of their shape
  on a board, now or by a `@keyframes` rule. The Windows-app runner's
  Settings window showed the Privacy tab's words squashed for a moment
  while it shrank to fit.

### Changed

- **Mutation testing is a check broken once, while it is added.** The
  break is undone and the commit says how it went; nothing keeps or
  reruns it. The kit's `tests/mutate`, 141 stored breaks rerun in twelve
  CI shards on every push, is gone: a run took about an hour and proved
  nothing new after its first. `tests/run WORD` runs one section, so a
  break is seen failing in seconds.

## 1.7.3 (2026-11-18)

### Fixed

- **`check-voice` missed a Czech "we" on a web page.** A page's language
  now comes from `<html lang>` (and a `lang` on any element), and copy
  that names no language takes `COPY_LANG`, as `check-copy` reads it. A
  Czech page said "a říkáme to rovnou" (and we say so plainly): its path
  named no language, so only the English page's "we" was found.
- **A Markdown page names its language in its front matter** (`lang:
  cs`), for `check-voice` and `check-copy`, over `COPY_LANG`. A website
  with English `index.md` pages and one Czech one could set neither
  language for all of them: `COPY_LANG=cs` read "Crake's service" as
  Czech and asked for "se service".

### Changed

- **`check-voice` reads web pages and design boards** (`UI_BOARDS`): their
  text, alt, title, aria-label and placeholder, never a script, a style or
  a comment. Before, a board was read for grammar but never for "we", and
  a page was read line by line, markup and scripts included.

## 1.7.2 (2026-11-16)

### Fixed

- **`check-copy` asked for "ze SMS".** An abbreviation with no vowel is
  read letter by letter, and "SMS" is "esemes", so "z SMS" and "v FTP"
  are right; one read as a word keeps the rule, "ze SÚKL". Obálka's three
  "kód z SMS" strings were findings.
- **`check-copy` read code as copy in no language.** A module's path
  (`from './strings'`), a type's members (`'app' | 'isds'`) and, in a
  code catalogue with `cs` and `en` blocks, the ids and plural categories
  outside the blocks are code. Obálka had 56 such findings in two
  catalogues.
- **The Claude Code hook kept checking files its gate no longer reads.**
  A file that once failed was checked again after every tool call, even
  after `COPY_FILES` (or `UI_BOARDS`, `VOICE_FILES`) left it out. In
  Obálka, narrowing the copy to its catalogues still read all 327 source
  files on every call.

## 1.7.1 (2026-11-16)

### New

- **A live switch draws what a fresh start draws** (mobile/devices.md).
  A setting that changes the look while the app runs is walked both ways,
  and the same screen is the same pixel for pixel. Obálka's "Testovací"
  pill lost its "í" after a switch to dark and was whole after a restart;
  every walk and check had looked at fresh starts.
  - `check-shots FRESH LIVE` compares two screenshots, or two folders of
    them by name, and lists each region that differs; `--skip-top PX`
    leaves out the status bar. On Obálka's switcher it finds the lost
    glyph, 7×21 px, and nothing between two fresh starts.

## 1.7.0 (2026-11-16)

### New

- **`check-copy`: each language's grammar, checked by its own rules**
  (common/voice.md, "Each language's grammar is checked"). Scops showed
  "Ve 10:20 ho vzbuďte"; a time is said "v deset dvacet", so it is "V
  10:20". The owner caught it on a screenshot, and nothing could have.
  - Rules files per language, `copy-rules/<lang>.mjs`: Czech (v/ve before
    a time and a weekday, ve before v and f, se/ze, ke), Spanish (al, del,
    e before i, u before o) and English (a and an). A project adds its own
    in `.practices/copy-rules/`.
  - Each piece of copy gets its own language's rules: a board's `<html
    lang>`, a catalogue's path (`cs.json`, `values-cs/`) or block (`cs:
    {`). Copy that names no language is a finding, and so is a language
    without a rules file: a new locale ships with its rules.
  - A fact several boards show, marked `data-fact="KEY"`, reads the same
    on each (ui/design-before-build.md, "Every fact a screen uses was
    entered earlier"). Scops' "start Wednesday" screen and its week tab
    disagreed about one Friday.
  - It runs on the boards and the copy files, where `check-voice` runs:
    after each edit and before the turn ends, before each commit, in
    lint. An exception is marked with its reason, `data-allow="copy: …"`
    or `practices-allow copy: …`.
- **`check-ui` measures both safe areas of a phone** (safe). In a frame
  marked `data-frame="phone"`, nothing to tap ends less than 34 + 8 px
  above the bottom (a tab bar 34) or starts under the status bar. A frame
  without its home indicator, and a phone-width board without a frame,
  are findings. Scops had 21 of 85 screens with a button in the home
  indicator's area.
- **`check-ui` keeps labels on one line** (wrap). A button, chip or tab
  whose label wraps is a finding; nothing overflowed when Scops' "Připojit
  se k rodině, která už Scops má" took two lines.
- **`check-ui` wants the page behind a sheet marked inert** (sheet). An
  open dialog with live targets drawn before it in its screen is a
  finding: mark the page `aria-hidden="true"` in one place.
- **New practices** in ui/visual-craft.md, "A button label stays on one
  line" and "A sheet's page is inert, and the board says so", and in
  ui/design-before-build.md, "Every fact a screen uses was entered
  earlier". "One anatomy per job" asks for a class per component, "A
  drawn icon set" for one map from action to icon and a radiogroup for a
  single choice, and "Depth and texture" says glows aren't depth and a
  sheet casts no shadow upward.

### Better

- **`check-ui` renders a board only as often as it differs.** A board
  fixed at a phone's width (`<meta name="viewport" content="width=390">`,
  or `canvas.json`'s `boards[file].w`) renders at that width, and a board
  with no dark rules in one theme. Scops' 87 boards took 348 renders where
  87 would do.
- **`check-ui` measures a halo to the text's ink**, not its line box. Lamp
  digits with no descenders read as 1.9 px into a glow 18 px from them.
- **`check-ui` knows a weekday picker is one control.** A `role="group"`
  of toggles that each carry `aria-pressed` is segmented, like a
  radiogroup, for gap and icon. Scops needed three marks for one picker.
- **`check-ui` outcome reads the accessible name**, and each language's
  words. "Ne" with `aria-label="Neděle"` is Sunday, not "no". The words no
  button may say alone are each language's `vagueButtons`, in the same
  rules files as check-copy's, so Spanish gets "Aceptar" and "Cancelar".
- **`check-ui` asks unclassed buttons for a class.** same-look compared
  every unclassed button with every other (71 findings on Scops, mostly
  noise); its finding now says to name each component first.
- **`check-ui` opens Chrome's page itself, and tries three times.** In a
  container Chrome's start failed half the time with "Cannot read
  properties of undefined (reading 'targetId')"; it now says "Chrome
  didn't open a page" when it gives up. Run as root (a dev container, a
  cloud session), it passes `--no-sandbox`, as in CI.

### Changed

- **Copy that names no language now fails `check-copy`**, a project's
  changelog among them. Name it once: `COPY_LANG=en` in
  `.practices/config` for the language of copy that names none.

### Fixed

- **`check-ui` read text under an inert page's scrim.** Chrome's hit test
  skips inert elements, so a scrim inside the page no longer covered its
  text, and Scops' boards had contrast findings of 1.9:1 there. Inert
  pages are skipped like `aria-hidden` ones, targets and text alike.

## 1.6.5 (2026-11-16)

### New

- **What covers a surface's edge takes room from its padding**
  (ui/visual-craft.md, "Spacing is measured from what you see"). Obálka
  tucked its letter 10 px under its envelope and kept the letter's
  padding of 14, so 4 to 6 px showed above the subject against 14 below;
  the owner saw the title "kinda hidden under the blue top bar".
  - `check-ui` (inset) measures each filled box painted over a filled
    surface's edge: from that box's edge to the surface's nearest text
    must be at least the surface's smallest padding on its other sides.
    On Obálka's 24 boards it flagged the 15 tucked letters before the fix
    (9 findings: like letters fold into one) and nothing after it. Mark a deliberate cover with `data-allow="inset: <why>"`.

## 1.6.4 (2026-11-16)

### Fixed

- **`check-ui` sees what covers a wide board's right side.** It renders at
  1100 and 1440 px and looked under each line only by scrolling down, so
  on a board wider than the window, text past the right edge always read
  as uncovered: on Obálka's 3150 px dialogs board 640 runs under sheets
  were measured, and a ring's halo was held to an avatar's initials under
  a sheet instead of the label beside it. It now scrolls both ways.

## 1.6.3 (2026-11-16)

### New

- **A glyph keeps one ink** (ui/visual-craft.md). A glyph's plane or fill
  is a tint of its stroke's hue, or a neutral. Obálka's danger bin on a
  neutral button kept the family's blue plane under a red stroke, and the
  owner saw the two inks fight; on the red button, where the plane was
  white, the same bin was right.
  - `check-ui` (ink) flags a glyph up to 64 px whose stroke and fill are
    two distinct hues, symbols drawn through `<use>` included. Mark a
    deliberate two-colour glyph with `data-allow="ink: <why>"`.

## 1.6.2 (2026-11-16)

### New

- **A touch-up gets its own before and after** (ui/design-before-build.md).
  A change to an agreed design is shown on a small page of its own: that
  one change, before beside after, cropped to the object and naming where
  to find it. A touch-up to Obálka's postmark went out as a new version of
  a 70-screen canvas, and its owner couldn't see where to look; on its
  own page it was accepted at a glance.
  - `check-commit-msg` (the commit-msg hook) refuses a commit that changes
    an existing board in `UI_BOARDS` without a `Before-after:` trailer
    naming the page, a file in the repository or an https link. A change
    that leaves the look alone says `Before-after: none, <why>`. A new
    board needs none.

### Fixed

- **`check-ui` read unscaled type as shrunk.** 1.6.1 measured a scale
  as an element's drawn box over its `offsetWidth`/`offsetHeight`, which
  round to whole pixels: an 11 px badge 19.6 px high read as 0.98 of
  itself, 10.8 px, on Gentes's boards. The scale now comes from the
  element's and its ancestors' `transform` and `scale`, multiplied, so a
  tile in a scaled tile is measured too.

## 1.6.1 (2026-11-16)

### Fixed

- **`check-voice` missed "we" it should have read.** Obálka's update to
  1.6.0 passed its 13 default copy files, while its FAQ said "not even
  us" and its changelog "we fixed". Widened to its real copy, the check
  found 19 lines. It now reads the changelog and a `content/` folder by
  default, any case ("WE NEVER SELL IT"), a ternary's branches (`ok ?
  'We keep it' : …` read as a key), and the text between JSX tags.

- **`check-voice` flagged words that aren't "we".** "Chrome", "Home"
  and "Time" read as Czech verbs; "nám." (náměstí, a square) in an
  address read as "to us"; "/us/" in a store link, and a word quoted as a
  word ("the word “we”"), were findings. All are skipped now, as are
  TypeScript generics and comments between JSX tags.

- **`check-ui` flagged boards that follow the rules.** Obálka's 24
  boards gave 248 findings, and these were the check's own:
  - Every phone's bottom bar is a surface padded like the screen's text.
    Its buttons read as "16 px in" from the phone's frame (15 findings).
    A row that draws its own surface, or is placed by hand, is now its
    buttons' column.
  - A column centred by a flex column (`align-items: center`) counted as
    leading-aligned.
  - A screen drawn mid-scroll inside a clipping frame was reported twice,
    by clip and by contain, so one `data-allow` mark couldn't cover it.
  - A segmented control's options, marked `role="tablist"` or
    `role="radiogroup"`, needed a mark for gap and an icon each.
  - same-look compared an icon button's unset type with a labelled
    button's, and a tab with every other unclassed button. Icon buttons
    and roles are kinds of their own now.

- **`check-ui` passed a Czech "Zrušit".** The outcome check knew only
  English, so a Czech board's bare "Zrušit" (Cancel), "Ano" and "Ne"
  passed. It now knows their Czech words too.

- **`check-ui` passed type a transform shrinks.** 14 px text in a preview
  scaled to a half is 7 px on the screen. It now measures HTML type at
  its drawn size, as it did for SVG.

## 1.6.0 (2026-11-15)

### New

- **Rules a machine can check are gates the agent can't skip.** On
  2026-11-15 an agent called a design board ready, and the owner saw a
  button 3 px out of its panel within seconds. The rule was written;
  nothing ran it. The new rule: every rule a script can decide runs
  after each edit, before the agent's turn ends, before each commit and
  in CI, and its findings go back to the agent as an error to fix. Only
  judgement stays with reviewers (`common/gates.md`, and this
  repository's CLAUDE.md for contributors).

- **`check-ui` measures design boards.** It renders every board under
  `design/` in headless Chrome, at each width, light and dark, and
  checks nothing runs past its surface or gets cut off. It also checks
  the rest of `ui/visual-craft.md`'s measurable rules: buttons that
  follow their column, 8 px between targets, the 11 px floor, rings and
  shadows, halos near text, centring, states that don't move content,
  icons on buttons, like controls alike, and contrast, measured against
  the pixels behind text on gradients and pictures. A deliberate
  exception is marked in place, `data-allow="rule: reason"`; a mark
  without a reason is a finding. Node 18 and Chrome; no packages.
  Settings: `UI_*` in `templates/config`.

- **`claude-hook` runs after edits and on Stop.** After Bash, Edit and
  Write it runs `check-ui` on boards that changed; on Stop and
  SubagentStop the agent can't end its turn while a board fails.
  `bin/install` updates settings that still match the old template, and
  the new `check-hooks` fails a commit whose `.claude/settings.json`
  stops running the hook. pre-commit runs both.

- **Buttons follow the column.** A row of buttons starts where the text
  above it starts, fills the column, or is centred in a centred column.
  One forward action may sit at the trailing edge; one button never
  stretches across a wide surface (`ui/visual-craft.md`, checked by
  `check-ui`).

- **The product speaks as itself, never as "we".** A data-box app told
  people "we sent you the code by SMS" when the state's system had sent
  it, and promised "nobody opens it, not even us" without saying who
  "us" was. Copy now names who acts (the app, the developers, the
  service) or says it impersonally. The new `check-voice` reads every
  string people see, in English and Czech (where the verb alone says
  "we": "upozorníme"), and runs after edits, on Stop, in pre-commit and
  in lint (new `common/voice.md`). On that app it found 37 strings; a
  careful review had found eight.

- **A mark holds at its smallest size.** A wordmark may be cut, sampled
  or mixed from several typefaces; only what fails where it ships is
  ruled out. A drawing of a Mac app's wordmark set its egg as a speck the
  owner couldn't make out, and a later cut left strokes of 0.9 px, grey
  fuzz at the smallest size; the designer saw neither at the board's
  sizes. `check-ui` now finds each `svg[data-mark]`, draws it alone and
  large, and measures it at the smallest size the board shows: strokes
  1 px or more, no leftover piece of a cut, the brand element
  (`data-mark-brand`) 10 px or taller and touching the letters or a
  quarter of a stroke clear (`ui/identity-pass.md`).

- **Every practice reaches `rules.md`.** The digest each session loads
  missed `it-just-works`, `consent`, `drag-and-drop` and `pickers`, so
  no session saw their rules. `tests/enforcement` now fails on a practice
  that `bin/install` doesn't digest.

- **Every button names its outcome.** "Remove" beside "Cancel" left
  the owner unsure: cancel the removal, or cancel the app? The way out
  now names what stays ("Keep the app", "Keep editing"), and `check-ui`
  flags any button labelled only Cancel, OK, Yes, No, Confirm, Submit
  or another word that fits either answer (`ui/visual-craft.md`).

- **Nothing runs past its surface.** A button, label or badge stays
  inside the panel, card or sheet that holds it, at every width
  (`ui/visual-craft.md`, checked by `check-ui`).

- **ENFORCEMENT.md lists every rule and how it's enforced**: gate,
  partial, todo or review, audited rule by rule against the scripts.
  The todo rows are the work list. `tests/enforcement` fails when a rule
  isn't listed.

- **A test of a check is proven by breaking the check.** `tests/mutate`
  breaks each check in a copy of the kit, and `tests/run` must fail on
  that check's own case. It guards itself: an unmutated baseline first,
  no-op and unparseable mutants refused, and a failure for another
  reason isn't counted (`common/gates.md`). CI runs it on macOS.

### Fixed

- `rust/conventions.md` and `rust/lints.md` named the unsafe-code
  setting `UNSAFE_PATHS`; the scripts read `UNSAFE_ALLOWED`.

## 1.5.0 (2026-11-15)

### New

- **Geometry is measured, not eyeballed.** Scops, a baby-sleep app,
  stacked "Edit" tiles 2 px apart and put buttons on 21 of 85 screens
  inside the home indicator. Its lamp halo came within 4 px of text,
  and a centred line hugged one side. Reviewers on scaled-down boards
  missed all of them. Separate targets now keep 8 px apart, both safe
  areas belong to the frame, spacing counts from the visible edge, and
  centred text sits on its axis. Each check is a measurement, and
  reviewers get 1:1 crops (`ui/visual-craft.md`). A shared
  `scripts/ui-audit` is on the roadmap.

- **Dates, times and numbers are picked, never typed.** Scops typed a
  birth date, and the age beside it never changed (it was wrong on the
  board too). Values are now picked and what's derived recomputes.
  Calendars jump to a year and month and show the neighbouring days,
  muted. Suggested choices never fence anyone in, and a class of bug is
  fixed across every board in one pass (new `ui/pickers.md`).

- **Consent without pressure.** Never pre-selected, two equal answers,
  "No, thanks" without guilt, and what is sent said next to them (new
  `ui/consent.md`).

- **One reading only.** "At most 20 minutes" left Scops' owner unsure
  what it limited. Every number and limit now names its subject and
  what happens when it runs out. Partial views say they are partial,
  and the next question is answered on the screen where it arises
  (`ui/visual-craft.md`).

- **Depth from the product's own concept.** Scops was "very flat"; its
  depth now comes from the owl's night lamp, never from another
  product's signature, and shadows fall away from the light. A global
  layer may not erase a meaningful ring: draw those with `outline`
  (`ui/visual-craft.md`).

- **The identity pass asks how the user feels.** The brief names the
  user's emotional state, and every screen is read as the user in their
  worst moment. Claims about health, safety or results cite a source
  and promise nothing. The inventory lists none, one, many and at once,
  and a failed state for every attempt. Fonts live in the repository,
  every copy change is rendered again, and a companion mark may stay
  through a long moment (`ui/identity-pass.md`).

- **Words and controls that agree.** Full forms in gender and case,
  declined names, past tenses in the user's gender, and every gender on
  the board. A choice changes everything it owns, icons and dependent
  fields too, and like controls keep one look after a click. Icons go
  on every row and main button (`ui/visual-craft.md`).

- **Independent review before every round** is now one of the UI
  skill's Musts, with the claims verified before anything is fixed.

- **Drag and drop: one calm panel says what will happen.** Taken from
  tern's compose window, which the owner called "the optimal
  representation of drag and drop", after the Windows-app runner's
  light-up strip. A dashed panel over the target in the soft accent,
  the action's icon in a circle, a verb phrase and one line of scope;
  the resting hint fades (`ui/drag-and-drop.md`).

- **Hard stop: the name and the mark read clean.** A capital letter in
  the Windows-app runner's first name, and the egg that replaced it in
  the wordmark, set a derogatory slang word apart; seven rounds and
  their reviewers missed it. Every cut of a name and mark is now read in
  slang and in each language before it is agreed, and a bad reading
  withdraws the mark (`ui/identity-pass.md`, the UI skill's first Must).

- **A number says what it counts.** The Windows-app runner's footer
  read "Technical log 2.1 GB": the app's size looked like the log's, and
  two reviewers missed it. Every number now carries its noun, and facts
  stay apart from actions (`ui/visual-craft.md`; step 10 of "Before you
  show anything").

- **Decoration never half-hides what you read or click.** The
  Windows-app runner's rows scrolled behind a drawn nest: the label was
  barely readable on it, and half-hidden rows looked clipped. Lists now
  end above drawings with the system's fade, and text sits on flat
  surfaces (`ui/visual-craft.md`).

- **What it leaves behind says whose it is.** The Windows-app runner's
  board put each app's work in "Documents › Enterprise Architect"; the
  owner pointed out nobody would know it was the runner's and would
  delete it. Folders a product makes where the user looks now sit under
  one folder named after it, with its icon (`common/it-just-works.md`).

- **Install it and it just works.** The owner's promise for every
  Dorhawk product, written down: the product decides what it can from a
  signal, says why and can undo; settings are preferences, never
  technical workarounds (`common/it-just-works.md`). From the
  Windows-app runner, after CrossOver's renderer, DLSS and MSync
  switches. Its checks cover every surface the user sees, the splash and
  dialogs too: CrossOver shows Enterprise Architect's splash huge and
  its main window right.

- **Every variant carries the identity, from the first round.** The
  Windows-app runner's first identity round read "quiet like Apple's
  utilities" as plain: greys, stock symbols, none of the idea's objects.
  The owner called it "virtually no identity". Iteration 2 now requires
  the drawn icons, palette, type and the idea's objects in every
  variant; layout-only options are a labelled wireframe
  (`ui/identity-pass.md`, the UI skill's Musts).
- **The real things the product handles.** The Windows-app runner's
  mocks showed drawn stand-ins for real Windows apps for five rounds;
  the owner had to ask for the real icons, which raised design questions
  the stand-ins hid. Real third-party assets now come with sources,
  credits, written derivation rules and a made-up product for failure
  states (`ui/identity-pass.md`).
- **Before you show anything: three more checks.** The named typeface
  must render (Chrome showed Helvetica for SF Pro Rounded), every state
  is drawn in every theme (dark broke on the unshown states), and the
  mock's own product copy must be true (`ui/identity-pass.md`, steps 7–9).

## 1.4.0 (2026-11-13)

### Changed

- **Every string names its face, and Android proves it.** A UI kit's
  buttons and fields drew in the system's face; handing the kit the face
  passed the test and still drew Roboto on Android. Wrap the kit once,
  draw the label in the platform's Text if the kit re-resolves it, and
  check letter shapes on the device (`mobile/lifecycle.md`).
- **A dimmed control fades as one piece.** Android fades each child of a
  view on its own, so a dimmed button's label came out grey on grey;
  `needsOffscreenAlphaCompositing` fades it whole (`mobile/lifecycle.md`).
- **A drawing laid over an object reaches the object's edges.** Folds and
  sheens go in a layer pinned to all four sides, not in the padded
  content. An envelope's folds stopped 16 px short of every corner
  because the drawing was sized inside the padding (`ui/visual-craft.md`).
- **What a screen says is ranked.** Text that carries different facts is
  split: the fact a person looks for leads, the rest goes quieter, short
  or behind a tap, and nothing is dropped. A sender's legal form had the
  name's weight and three lines of it (`ui/visual-craft.md`).
- **A built screen is checked pixel by pixel before it is called done.**
  Each object is cropped at full resolution beside the design's crop and
  checked for edges, lines, glyphs, the rendered typeface and every
  string. The owner found five such faults in one walk the agent had
  called matching (`ui/design-before-build.md`, the UI skill's Musts). A
  fix is checked against every design that shows the part: one took a
  line off every header when only one design left it out.
- **A control for a system act shows that system's sign.** Unlock shows
  Face ID on an iPhone and a fingerprint on Android; both drawn in the
  product's own icon family, chosen by the platform in one place, tested
  for each (`ui/visual-craft.md`).
- **The rendered check runs before the owner sees anything, whatever the
  tool says.** A canvas type's "never verify unless asked" made the pass
  look optional; the owner's standing request wins, and the handoff says
  the checks ran (`ui/identity-pass.md`, the UI skill's Musts).
- **Every product gets its own drawn icon set.** One family grown from the
  product's existing drawings and the idea's object, two families shown,
  real third-party logos instead of stand-ins; a rare mark no longer
  leaves the screens plain (`ui/visual-craft.md`, `ui/identity-pass.md`).
- **A size limit is met by splitting the file, never by trimming it.**
  A file at its allowlist count is split first; shortened wording or a
  dropped plural form to make room is refused (`common/gates.md`).
- **A value the design's scale lacks goes to the owner.** Add the step or
  move the board; never hide the value where the lint doesn't look
  (`ui/design-before-build.md`).
- **"Match one to one" measures as well as looks.** Render the board at
  the device's pixel ratio and compare rows of pixels: a frame drawn twice
  as wide passed side by side.
- **Red before green, for a design port.** The code may come first; each
  assertion is then proven by a mutation, listed in the commit
  (`common/testing.md`).
- **Text takes its container's width.** A line beside something else
  gets `flex: 1`, a centred line stretches to its column: Android cut
  the last word of text sized to its own words (`mobile/lifecycle.md`).

## 1.3.1 (2026-11-12)

### Fixed

- **An identity pass's inventory proves it misses nothing.** Iteration 3
  said "build an inventory from the code" with no way to check it, and a
  list written by reading passed. Obálka's missed 22 dialogs, two flows
  and nine kinds of surface. A new section in `ui/identity-pass.md`
  sweeps every UI file and string group, and counts every use of a shared
  dialog, sheet or menu. Screens split by state. A second inventory, made
  without seeing the first, is compared line by line. "Matches" needs a
  capture or a board. Iteration 3 also lists the system's surfaces, the
  largest text size and every orientation.

## 1.3.0 (2026-11-12)

### New

- **A quiet pre-commit.** It prints the tests' output only when they fail.
- **The plugin's version is the practices' version.** It said 1.1.0
  through 1.2.0 and 1.3.0; it now says 1.3.0, and `tests/versions` fails
  when plugin.json or marketplace.json disagrees with the newest release.
- **README stays the map.** `tests/readme` fails when "What's in the
  repository" misses a top-level folder or file, or when a practice, guide
  or script isn't listed. The tree had gone a release without
  `typescript/`, `mobile/` and `templates/`.
- **This repository has a local gate.** `tests/check` runs shellcheck on
  every script, the links and `tests/run`; `.githooks/pre-commit` runs it
  and CI's static job calls it. A shellcheck failure in `tests/run` had
  reached main because the checks ran only in CI.
- **JavaScript and TypeScript projects get the gate.** In a project with a
  package.json, `.practices/bin/lint` runs `tsc --noEmit` (with a
  tsconfig.json) and the npm scripts in `NPM_CHECKS` (default `lint`);
  pre-commit runs the gate and `NPM_TEST` (default `test`) when JS or TS
  files are committed; the Claude Code hook runs the gate after `npm test`,
  `npx jest`, `npx tsc` and the like. `check-size` counts `.ts`, `.tsx`,
  `.js` and `.mjs` files too (tests up to `SIZE_TESTS`), and `SIZE_EXCLUDE`
  leaves generated files out. Rust's checks run only where there's a
  Cargo.toml, and disk-guard only guards cargo's build folder.
- **TypeScript and mobile practices** (`typescript/`, `mobile/`), from the
  Obálka fold-in: conventions, lints and Jest testing; store releases,
  devices and walks, CI for iOS and Android, the app's lifecycle. Projects
  with a package.json, an ios/ or an android/ folder get their rules in
  `rules.md`.
- **Gates that can fail** (`common/gates.md`) and **crash reports and
  logs** (`common/crash-reports.md`), with new sections in testing,
  commits, CLAUDE.md and design before build.
- **The Claude Code hook settings for every project** (`--templates`), not
  only Rust ones: `templates/claude-settings.json`.

### Fixed

- **No example headings in `rules.md`.** The digest listed a heading from
  a code example ("Unreleased" in common/changelog.md) as a rule; it
  reads headings outside code blocks only.
- **The guides and the adopt skill know JS and TS projects.** They still
  said the kit had no checks for TypeScript.

## 1.2.0 (2026-11-12)

### New

- **Agents get the practices with the kit.** `bin/install` now copies the
  practices into `.practices/docs/`, writes `.practices/rules.md` (a line
  per rule, about 120) and the line `@.practices/rules.md` in CLAUDE.md,
  so every session loads them, and puts the skills in `.claude/skills/`:
  new ones for UI design, testing, commits, releases, CI and docs, beside
  the gate and adopt skills. It ends by saying to restart the agent
  session. Before, only the scripts reached a project, and agents saw the
  rules only where someone had copied them into CLAUDE.md.
- **Each project pulls its own updates.** `bin/install` adds
  `.github/workflows/practices.yml`, which calls the new reusable
  `practices-update.yml` weekly: the newest release of the same major, as
  a pull request, with CI started on it. No token and no list of
  projects. `projects.json` and `sync-projects.yml` are gone: the push
  sync needed a token that was never set, so no project ever got a sync
  pull request.
- **An identity pass, through the whole app** (`ui/identity-pass.md`).
  Ideally the identity is researched before the first screen: audience,
  the product's job and process, inspiration, the market, and the facts
  behind its metaphor. Then three iterations, each agreed with the owner:
  establish the identity, redesign a few parts, then redesign the whole
  app, re-audit and polish. How far to push depends on the identity
  already there, but every screen is reviewed. From the Obálka, Crake and
  tern passes.
- **Visual craft** (`ui/visual-craft.md`). Tone instead of outlines, every
  stroke end matching, one drawing per object, a line weight per family,
  compact screens, an 11 px floor, one anatomy per job, real context in
  proposals, the product's voice, motion with a reason.
- **UI design practices, in `ui/`.** Window buttons where each system puts
  them: macOS draws its own on the left and the header makes room, from
  Crake's Mac fixes, with the GPUI how-to in `rust/gpui.md`. Design before
  build moved here from `common/` (its old link is gone).
- **Keyboard shortcuts the way each system writes them** (`ui/`). Bind
  the system's main modifier (⌘ on a Mac, Ctrl elsewhere) and show
  shortcuts as each system does: ⇧⌘K on a Mac, Ctrl+Shift+K elsewhere.
  Translations hold no key names. From tern's first Mac run, where the
  search field said "Ctrl K" and ⌘K did nothing. The GPUI how-to is in
  `rust/gpui.md`.

### Better

- **Window buttons on macOS take 96 px, not 88.** macOS 26 draws its
  three buttons 60 px across, not 54, so 88 px left 10 px before the
  header's content against 18 px before the first button. The room now
  keeps as much after them as before.
- **Dependabot in a private repository.** The template shows how to group
  major updates into one pull request with at most two open, so the first
  run doesn't open one build per crate (tern's opened six).

## 1.1.1 (2026-11-11)

### Fixed

- **The version a project records.** `bin/install` wrote `v1` into
  `.practices/VERSION` when the moving `v1` tag was newer than the release
  tag on the same commit; it names the release (`v1.1.1`) now.

## 1.1.0 (2026-11-11)

### Better

- **Starting a project in any language.** The start guide is now for every
  project (identity, CLAUDE.md, privacy, the git hooks, CI's cost,
  changelog, not-now), with the Rust steps on a page of their own,
  `guides/start-a-rust-project.md`. `bin/install` puts the Rust parts
  (coverage floors, templates) only into a project with a `Cargo.toml`;
  the hooks work in any repository and never call cargo, which a test now
  checks.

### Fixed

- **Closed-source apps' own crates in cargo deny.** The closed-source
  licence list in the deny.toml template now comes with
  `[licenses.private] ignore = true`; without it, cargo deny called each
  of the app's own unpublished crates unlicensed.
- **Cache pruning in private repositories.** The `prune-caches` action
  asked GitHub's API for the default branch, which the job's token may not
  read in a private repository ("Resource not accessible by
  integration"); the workflow now passes the branch in. Found by tern's
  first CI run.
- **A quiet pre-commit hook.** check-coverage listed every test binary
  and its result even when all passed, filling an agent's context on each
  commit (found adopting tern). It lists them only in CI now.

## 1.0.0 (2026-11-11)

### New

- **The practices, from Crake.** Ten common practices and four Rust ones,
  each with the incident that taught it, how to apply it and how to check
  it, plus guides, a worked example and a roadmap.
- **The kit.** `bin/install` puts the lint gate and its checks (private
  names, file sizes, unsafe code, coverage floors, commit messages, the
  disk floor) and git hooks into a project's `.practices/`, keeping what
  the project owns.
- **Actions and reusable workflows.** `rust-setup`, `rust-lint`,
  `rust-coverage`, `prune-caches`; `rust-ci.yml`, `rust-advisories.yml`,
  `rust-toolchain-bump.yml`; and `sync-projects.yml`, which brings kit
  updates to each project as a pull request.
- **A Claude Code plugin** with skills to adopt the kit, fix what the gate
  finds, and fold in another project's practices.

### Better

- **Checks that fail closed** where Crake's could pass silently: the
  privacy check without its terms file, a lowered coverage floor, a
  skipped scenario, `#[cfg(all(test, ...))]` test modules, untracked files
  with unsafe code.
