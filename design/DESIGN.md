# Crake design

This file is the source of truth for how Crake looks, sounds and behaves. The
screens in `design/screens/` show it; this explains it. When the two disagree,
fix the screen.

## What Crake is

Crake is an **instrument, not an app**. You switch it on and forget it. It
connects to one or more VPN gateways, routes nothing by default, and only sends
a host through a tunnel when that host doesn't answer directly but does answer
through the tunnel. Then it tells you, in one plain sentence, and lets you undo.

You open it rarely: to see what it did, to switch a tunnel, or to read a line to
your helpdesk. So the main surface is a **compact panel that drops from the
tray**, not a big window. Everything else (log, settings, tunnel detail, new
tunnel) is a secondary window opened from that panel.

The name: the corncrake (*Crex crex*) is a shy meadow bird, heard far more often
than seen. Crake works the same way. Its personality comes from behaviour
(quiet, honest, explains itself, always undoable), not from decoration.

## Directions we tried and rejected

Don't drift back into these. Each was tried and dropped for a reason.

- **The dorhawk.software website look** (mountains, sky, flags, condensed
  uppercase, orange striped tags). Crake is a sibling of that site, not a copy.
- **A "tern" reskin** (three-pane inbox, sidebar switcher, status card at the
  bottom). That shape is right for an app you live in. Crake isn't one.
- **Skeuomorphic hardware** (screws, LEDs, paper recorder, typewriter type).
  Charming, but it reads as a toy, fails contrast, and won't pass IT review.
- **Generic dashboard** (grey plus three saturated primaries, cards with numbers).
  Correct and lifeless.
- **Imperfection for its own sake.** Every irregularity must mean something.
- **Breathing or pulsing status dots** (the `breath` animation in
  tray-panel.html). Reads as an AI-made-UI tell. Status is said in words;
  motion is kept for things that happen.
- **A coloured edge down one side of a box** (a 3 px amber line on a
  notice's left, design/proposals/leftover-warning.html, 2). An AI-made-UI
  tell, like the breathing dots: a notice says what it is with its words,
  its icon and, when it needs you, its whole surface.
- **Two OpenVPNs to choose from** ("OpenVPN" beside "OpenVPN 3 profile",
  design/proposals/openvpn-tunnel.html's first draft, B), and **an action
  inside a list's menu** ("Choose a .ovpn file…" as the menu's first row,
  its A): two cards that sound alike, and one action in two places. Chosen
  by the user (2026-10-18): one "OpenVPN" card, "Import .ovpn…" in the
  label's row (as ovpn-import.html, A), and importing a .ovpn picks
  OpenVPN by itself.
- **"Import .ovpn…" in every state, and a protocol list that stays open
  after a choice** (design/proposals/ipsec-tunnel.html, 1): with IPsec
  chosen, the import pointed at a file that had nothing to do with it
  ("why would I still want to import a .ovpn file? This is confusing
  design", the user, 2026-10-19), and the open list pushed what the
  chosen protocol needs (IPsec's key) 600 px down, under the fold. Chosen
  by the user: the list folds once a protocol is chosen ("Choose the
  protocol yourself" opens it again), and "Import .ovpn…" shows only
  while the protocol is "Detect automatically" or OpenVPN.
- **Teal text actions** (Import .ovpn…, Copy for helpdesk, About's links,
  the new-version notice's Restart and Download, "Copied"; text-actions.html,
  C: teal written into the rule as "something you can click"), and **small
  bordered buttons for them** (its B, the panel's Dismiss everywhere: two
  looks, since a link in a sentence stays text, and a faint fill in dark).
  A was chosen (see Colour).
- **The gateway's certificate behind a link under the key** ("No key? It has
  a certificate", design/proposals/ipsec-certificates.html, 1, B), and
  **"and a password" as a tick under Certificate** (its 2, B): the second
  way hidden in a help line, so someone with no key stops at a field they
  can't fill, and a second decision hidden inside the first. Chosen by the
  user (2026-10-21): two cards in step 1, "A key" / "Its certificate",
  with IT's certificate authority in the key's slot; a third card in step 2,
  "Certificate and password".
- **An IPsec tunnel's domains in step 3** (ipsec-tunnel.html, 2, B):
  beside "Only what I list", whose list takes names for another purpose
  (what may go through, not where a name is asked). Chosen: step 1, under
  the gateway's key (A).
- **The tray's badge behind the head** (9/32 of the icon, between pixels:
  4.5 px across at 16 px, its "!" a smudge of half-inked pixels; "extremely
  teeny tiny", the user, 2026-10-18), the same corner on whole pixels (no
  room for a "!" up to 22 px), and **half the icon at the bottom right**
  (it takes the breast and the sleeping bill's tip): design/proposals/
  tray-badge.html, Today, A and C; B was chosen.
- **Tray marks above the head** (design/proposals/tray-marks-above.html).
  The bar icon is a fixed square: room above the head shrinks the bird.
  The call marks stay off the beak and the "!" on the back of the head.
- **A debugging dump beside the everyday copy** ("Save a report…" next to
  "Copy for helpdesk" in the log's title row, design/proposals/report.html,
  A): two answers to "what do I send?", side by side, and a tool for
  someone helping you shown to everyone who opens the log. The report is
  in Settings → Diagnostics (design/proposals/report-in-settings.html, A,
  chosen by the user 2026-10-19), where a helper sends you. Also there:
  a button that changes its words while working ("Saving…"), which
  resizes it and moves the row; the button stays, and what's under way is
  said in a box of its own with the dashed head.
- **What's new as a link in the panel's header** ("What's new in 0.15.0"
  beside "Listening"), and **a word before each change in a mono badge**
  ("New", "Fixed"; design/proposals/whats-new.html, round one, B): the
  header carries only status and the link had no dismissal; the badges were
  an icon per kind in another form (the user chose "quieter"), and mono is
  for machine facts. Chosen (2026-10-21): the notices' shape in the panel
  ("Crake is now 0.15.0: 9 changes since 0.13.1.", What's new, Not now), and
  the kinds as small muted headings in the window.

## Principles

1. **Say what happened, in a sentence.** "Sent git.acme.internal through ACME."
   with the reason under it: "It didn't answer directly. Through ACME it
   answered in 38 ms."
2. **Every decision is undoable** where it's shown.
3. **Policy is visible.** Every tunnel says who decides and what's allowed.
   Managed tunnels say so. A full-tunnel policy says Crake won't learn there.
4. **Quiet by default.** No numbers or charts that don't answer a real question.
5. **Nothing leaves the machine.** Say it where it matters (privacy, log); don't
   repeat it everywhere.
6. **Readable on a helpdesk screen share.** Body text 14 px, nothing under
   12 px, AA contrast or better.

## Colour

One accent. Teal means **"Crake made a decision"**: the ticks, "Sent …
through"; and with it the **on or chosen state** of a control (a switch, the
Listening dot, a chosen filter or step card, "Recommended") and the **one
primary action**. Never information: a path in a graph, an acknowledgement
("Got it"), a confirmation ("Found a gateway" wears a plain check), or
something under way (the dashed head beside "Asking the gateway…" and
"Saving the report…" is ink; the user, 2026-10-19: "teal is for
decisions"). Where
a way is told apart, it's by its name (the audit of 2026-10-17,
design/proposals/audit-2026-10-17.html, settled this: the Traffic graph's
teal paths had made teal mean "through a tunnel"). Two status colours,
each with one meaning too: red, **something stopped or failed**, and a
destructive action only when it's **armed** (pointed at, or in its confirm
step; quiet until then);
amber, **it needs you, nothing has broken yet** (relaxed 2026-10-15, asked for
by the user: "it must be tasteful, but maybe some colour, and an icon here
and there"; design/proposals/leftover-warning.html). Amber colours a whole
notice (its tint, its hairline) and its icon, the bird's head with an amber
triangle; never an edge on one side of a box, never decoration.

**Small actions and links** (chosen by the user 2026-10-19,
design/proposals/text-actions.html, A): ink, 13 px, semibold, with an ink
underline, as Settings → Diagnostics' Open and Show in folder; the hand
when pointed at; nothing changes size. One look for an action in a label's
row (Import .ovpn…, Edit…, Show, Change…), beside a sentence it acts on
(Copy for helpdesk) and a link to the web (About's). Never teal: the only
teal you click is the one primary action. A dismissal ("Not now") stays
muted and plain, quieter than the action beside it. A command line's Copy
is the leftover notice's segment (a hairline, the clipboard, Copy, then
the check and Copied in place). "Copied" after a host's name, and after
Copy for helpdesk for 3 s, is muted with the app's check (an
acknowledgement). Keyboard focus keeps the 2 px accent ring, with 2 px of
room inside an underlined action so it clears the letters.

**Notices** (the leftover process, "acting as AnyConnect", no tray, a new
version, the service's approval on macOS) share one shape: a title at 600,
a body, and an actions row of a bordered action and a muted text dismiss
("Not now"); "Got it" is that bordered button. The leftover notice's
command line, with Copy and Got it on it, is the user's choice (A2,
2026-10-15) and stays.

**What's new** (design/proposals/whats-new.html, chosen by the user
2026-10-21): after an update, the panel's notice, neutral (the surface
tint: nothing needs you, nothing was decided). The window: every window's
title row, its subtitle "Since 0.13.1, the version you had"; each version's
number (17, 700) and date (mono, as the changelog writes it), its changes
under New, Better, Changed, Fixed and Underneath (13, 600, muted), each a
headline (15, 600) and its sentences (14, muted), whole, as CHANGELOG.md has
them; three versions in full, then "Show earlier versions". No teal.

| Token | Light | Dark | Use |
| --- | --- | --- | --- |
| desk | `#DCDCD8` | `#121314` | behind windows (mockups only) |
| panel | `#FFFFFF` | `#1E2022` | window and panel background |
| surface | `#F1F1EE` | `#2A2D30` | grouped areas, badges, sidebar |
| line | `#E2E2DE` | `#33363A` | hairlines, control borders |
| lineStrong | `#BDBDB8` | `#4A4E53` | input borders, chart baseline, off switch |
| ink | `#1A1C1E` | `#ECEDEE` | text |
| muted | `#5A5F64` | `#A9AEB4` | secondary text (passes AA on panel) |
| bars | `#D8D9D5` | `#3E4246` | chart volume bars |
| accent | `#0F766E` | `#2DD4BF` | decisions, on-state, primary button |
| accentInk | `#FFFFFF` | `#0B1514` | text on accent |
| accentSoft | `#E2F1EF` | `#17332F` | selected row background |
| danger | `#B42318` | `#F97066` | a destructive action when it's armed (pointed at, or in its confirm step; quiet until then), a failed step; a window's × pointed at |
| onDanger | `#FFFFFF` | `#0B1514` | the × on danger |
| warning | `#BF7E12` | `#F5B740` | the triangle on the bird's head: needs you (light was #E39A1B, 2.4:1 on the panel; audit-2026-10-17, 9) |
| warningSoft | `#FCF4E4` | `#2C2618` | a notice that needs you: its tint |
| warningLine | `#EFD9AE` | `#5C4A22` | its hairline, and the border of a field in it |
| onWarning | `#1A1C1E` | `#1A1408` | the "!" on the triangle |
| navHover | `#F9F9F7` | `#242729` | a Settings section pointed at: halfway from the sidebar to the current section |

Follow the system light/dark setting. Never add a second accent, or another
status colour without one meaning of its own. Tunnels are
told apart by name, not colour.

## Type

- **Atkinson Hyperlegible Next** for everything you read (400, 600, 700).
- **Atkinson Hyperlegible Mono** for machine facts only: hosts, addresses,
  times, paths, protocol codes.
- Sizes: 26–28 window title, 22 step title, 17 panel/window name, 15 emphasised
  row, 14 body, 13 secondary, 12 minimum (mono labels, badges).

## Shape

- Window 12 px radius, panel 10, groups and cards 8, controls 6, badges 4.
- **One title row.** Every window's starts 20 px from the left and its
  hairline sits at 56; the same × in the same corner; a window differs
  only in its actions (measured 2026-10-17: the app's windows had begun
  at 20, 22, 24, 22, 25, 23 and 29).
- 1 px hairlines. No heavy borders, no offset shadows, no gradients.
- Switches are the standard rounded pill; that's the platform's language.
- The internet is never drawn. There's no map; the log and charts carry it.

## The signature: the two-stroke tick

Crake's one ownable mark is a **two-stroke tick**: a taller and a shorter
vertical bar side by side, the "crex crex" call. It means **routed**.
Beside a host (Traffic's graph and its Way column) it marks every host
Crake itself decided to send through, and stays when the user pins it; a
host the user added, a network, or one sent direct has none (wiki was
learned by DNS, then pinned, and lost its tick: the user, 2026-10-17).

- **Icon (18 px grid):** `M6 15V4M12 15V8`, stroke 2.4, round caps, accent.
- **In charts:** two filled bars, **flat bottom sitting exactly on the
  baseline, rounded top**, accent colour. Widths 3–3.2, gap 4.5–5, second bar
  about 75 % the height of the first. Use the helper below; never use
  `stroke-linecap="round"` for chart bars (round caps poke through the baseline).

```js
const bar = (x, top, base, w) =>
  `M${x - w / 2} ${base}V${top + w / 2}a${w / 2} ${w / 2} 0 0 1 ${w} 0V${base}Z`;
```

Related marks in the same family (18 px grid, stroke 2.4, round caps):

| Meaning | Path | Colour |
| --- | --- | --- |
| routed | `M6 15V4M12 15V8` | accent |
| left alone | `M4 9.5h10` | muted |
| undone | `M7 5L3.5 8.5L7 12M3.5 8.5H11a3.5 3.5 0 0 1 0 7H9` | muted |
| pinned (kept) | `M6 3h6M7.5 3v4.5L5 10h8l-2.5-2.5V3M9 10v5.5` | ink |
| set by gateway | `M4 9h10v7H4zM6.5 9V6.5a2.5 2.5 0 0 1 5 0V9` | muted |
| unreachable (chart) | hollow circle r 4–4.5, muted stroke | muted |

## Keyboard

Every control is reachable from the keyboard (critique 2026-10-15): Tab
and Shift+Tab move through a window's controls in reading order, Enter or
Space acts as a click does, Escape closes. The control in focus wears a
2 px ring in the accent with a 2 px gap in the panel colour, following the
control's own corners, and only when the focus came from the keyboard (a
mouse click shows no ring). New tunnel moves between its fields as before.

## Charts

One rule across every chart:

- **Grey bars = how much traffic.** Square ends, sitting on the baseline. The
  chart title says which traffic ("Last hour" in the panel = direct,
  "Today" in the log = everything, "Today through ACME" in tunnel detail).
- **Teal two-stroke ticks = the moment Crake routed a host.**
- **Two kinds of traffic in one chart: two greys.** Traffic's "Last 5
  minutes" stacks through tunnels (`muted`, on the baseline) under direct
  (`bars`); never teal for volume, which would make teal mean "through a
  tunnel" instead of "Crake decided" (critique 2026-10-15,
  `design/proposals/critique-2026-10-15/`). Its 5 s bars are narrower than
  a tick, so a tick stands at its slot's right edge, notching the
  neighbouring bars rather than hiding one.
- **Hollow grey circle = a host Crake tried and left alone.**
- Baseline: 1 px lineStrong. Time labels in mono 11 px, muted, under the
  baseline: start, middle, "now".
- **A tick keeps clear of the bars.** A two-stroke tick gets a 1.5 px gap in
  the panel colour around it, so it never runs into the traffic, and the
  bars stay: hiding them would suggest no traffic happened then
  (`design/proposals/tick-over-bars.html`, option A).
- **Decisions close together share one tick.** Ticks (or circles) less than
  about a minute and a half apart merge into one mark; when it stands for
  more than one, its count sits above it in mono 11 px, accent.
- **The log's day chart zooms** (`design/proposals/log-zoom.html`): scroll
  over it to zoom around the pointer (whole day down to 15 minutes), drag to
  move along the day, double-click or "Whole day" to go back. Bars get finer
  as it zooms (the finest of 1, 2, 5, 10, 15 or 30 minutes that keeps them
  about 6 px apart); merged ticks separate. Zoomed, the header says what's
  shown ("09:00–11:00 · 1-minute bars", mono 13 px muted) with a "Whole day"
  button; "Scroll to zoom · drag to move" sits under the chart (12 px,
  muted). Labels stay at round times. The list below doesn't follow.
- **Pointing at a chart reads it** (`design/proposals/chart-hover.html`): a
  dotted 1 px lineStrong hairline marks the minute, the bar there turns
  muted, and the time row under the chart becomes one line about that
  minute, in the log's words: mono time, then e.g. "6.7 MB went direct",
  "Sent git.acme.internal through ACME.", "Left 10.62.0.77 alone.", or for a
  merged tick "Sent git, iam.intranet and wiki through ACME." Nothing is
  drawn over the chart and the panel keeps its size.

## Icons

24 px grid, stroke 1.8 (2.0–2.2 when drawn at 12–16 px), round caps and joins,
`currentColor`. Shield = policy from IT, person = your own profile, circle with
slash = full tunnel required, lock = managed.

### Tray icon

**In the bar: the crake, calling** (design/proposals/bird-calling.html,
chosen 2026-10-17 after four rounds: richer-bird.html, bird-redrawn.html).
Top bars are small and app icons beside it are solid squares, so the tray
icon is the bird's bust, solid, filling the slot: the head raised in the
corncrake's call, a short stout bill parted at the tip (a mouth, even at
bar size), the eye a square hole on whole pixels (the bar shows through),
the folded wing one straight cut across the breast, the base running off
the bottom edge. Its richness is cuts, not shading, and each cut is a
feature: the mouth, the eye, the wing. One geometry, drawn by
`crates/crake/src/tray/art.rs` at every size a tray asks for with the eye
snapped to that size's pixels, anti-aliased at every size, 16 px included
(the spec copies are `design/icons/tray-head-*.svg`). Rejected: 16 px
placed pixel by pixel with no anti-aliasing (0.10.0): beside the bar's
other icons it read as pixelated, and it lost the wing's cut (the user,
2026-10-17).
White; the states:

| State | Shape |
| --- | --- |
| off | asleep: the head drops on its neck to 25° below level, the eye a slit, the ink dimmed to 45 % (an eye alone was too small a difference at 16 px; design/proposals/tray-off-vs-on.html, D; the base stays put, so nothing swings out of the slot) |
| connecting | the outline dashed, the bill shut |
| listening | the bird, eye open, bill parted |
| routed (a few seconds) | the bill open in the call, the two call marks in front of it, **teal** |
| trouble | a **red** badge with "!" at the bottom right, over the breast (14/32 of the icon: 7 px at 16 px), clear of the eye and the bill, on the bird as it would be without it: asleep when nothing is up, dashed while another tunnel connects, awake while one is up (design/proposals/dismiss-trouble.html, tray-badge.html B) |
| needs you | an **amber** triangle with "!" in the same place, on the bird as it is: an OpenVPN 3 process left running, until it ends or "Got it" (design/proposals/leftover-warning.html). Red wins: a tunnel that stopped shows trouble's badge instead |

The badge is drawn on each size's whole pixels, as the eye is: the circle
an odd or even number of pixels across and centred to match, its "!" a
stem a sixth of the circle wide, a space and a dot as wide as the stem,
on the circle's middle (at 16 px: a 1 px stem three rows tall, a gap, a
1 px dot), and none where the circle has no room for one. On a light bar
(Windows' light taskbar, macOS' light menu bar) the bird is drawn dark and
the badge takes the light theme's danger and warning (#B42318 with a white
"!", #BF7E12 with a dark one): the dark ones are 2.4:1 and 1.5:1 against a
light bar.

The bird changes shape, never colour; only the state marks are coloured
(dark-mode accent, danger and warning, the bar is dark), with a gap cut
into the bird around them so they read on any bar colour. Motion
(`tray/motion.rs`): routed, the bill opens as the first mark draws and the
second follows, twice (crex, crex); going to sleep, the eye closes, then
the head drops and dims. The whole bird with grass
(`crake-*-symbolic.svg`: the same bust standing in the grass) stays for
places with room; `CRAKE_TRAY=bird` shows it in the bar.

Rejected for the bar (bird-calling.html and the rounds before it): detail
on the old dome (cut lines, a shaded throat, nape streaks, feather
chevrons: still a ghost, thin cuts grey at 16 px); a bust with the bill
level (a penguin in one ink); long straight cuts along straight edges (a
door frame with a play button: cuts have to cross curves); bars across the
wing for the barred flank (stripes on a sail at real size); the head raised
at 16 px (the grid can't hold the angle); tipping the whole bird to sleep
(its base swung out of the slot); the bird in the corncrake's colours, in
the bar (the bill fades into the breast, a light bar needs a second colour
rule, and the one-ink rule above goes) and on the app icon (dropped by the
user 2026-10-17: one ink everywhere).

Whole bird (larger places):

| State | Shape |
| --- | --- |
| off | grass only |
| connecting | the outline dashed |
| listening | the bird in the grass |
| routed (a few seconds) | the bird calling, two marks in front of the bill |
| trouble | the bird plus the red badge at its bottom right |

**One drawing everywhere.** The panel header's bird (`bird.svg`, with the
call marks in accent or the bar's badge over it), the leftover notice's
head with the triangle cut out, the first-run bird that rises from the
grass, About's bird, the whole-bird symbolic icons and the app icon all
come from the tray's geometry: `cargo test -p crake write_bird_assets --
--ignored` writes the files and `the_bird_assets_are_current` keeps them
so. The app icon (`assets/icons/app/crake.svg`, then `scripts/make-icons.sh`)
is the panel header's bird in the dark theme's text colour (#ECEDEE) on
the dark panel tile, its eye and wing cut, calling in accent: the same
mark as dorhawk.software/crake's.

## Copy voice

- Short declaratives. Subject first. "Left 10.62.0.77 alone."
- The reason on the second line, in plain words, with the one number that
  matters. "Refused directly. Through Nordbank it answered in 61 ms."
- Name the actor: "ACME IT allows learning for *.acme.internal", "Full tunnel
  required by Globex IT. Crake won't learn here."
- Buttons name the action, not the state: Pin / Unpin, Undo / Redo,
  "Send it direct instead", "Copy for helpdesk".
- No exclamation marks, no "Oops", no emoji, no marketing.
- Settings that need more than their one-line explanation get a "?" after
  their name (15 px, muted; teal while open). Hovering or clicking it opens
  a card below it: 340 px, panel colour, `lineStrong` border, 8 px radius,
  13/19 px text. Plain words anyone can follow, then an example ("Example:
  with ACME off, looking up git.acme.internal simply fails at once."). VPN, DNS
  ("the internet's phone book") and the like are explained the first time.
  Requested by the user: some settings read as jargon.
- Settings rows carry no icons, with one exception: the bird's own row
  ("Show the bird in the tray", "…in the menu bar" on macOS; never "top
  bar" or "panel", which is Crake's popover) shows the bird after its
  title, 18 px on a 4 px-rounded square in the bar's colour (dark
  #2B2D30, or light #EEEEEE with the head dark where Windows and macOS
  draw it so), and its "?" lists each look of the bird with a sentence
  (design/proposals/tray-setting-after.html; impeccable critique
  2026-10-16).
- When nothing shows tray icons (design/proposals/no-tray-after.html): the
  bird row keeps its switch and says why the bird isn't there, with what
  would help under it (a bordered "Switch on AppIndicator" or "Open
  Extensions", or the install command on one line with Copy). Started by
  hand, the panel opens near the top of the screen with a neutral note
  (surface, no amber: the tray is optional; no teal: a bordered action and
  a muted "Not now"); at login, one notification instead of a window.
- Segmented controls frame the chosen segment in its own colour (each
  segment draws its own edges): a grey rim around dark teal reads as blur
  in light mode, as it did in settings.html and log.html.

## Screens

| File | What it shows |
| --- | --- |
| `tray-panel.html` | The main surface: last hour, what Crake did, tunnels with policy, full log / copy for helpdesk |
| `new-tunnel.html` | Three steps: server and protocol (auto-detect, AnyConnect/ocserv, GlobalProtect, Ivanti/Pulse, Juniper NC, Fortinet, F5, Array), sign-in method, routing mode |
| `settings.html` | General, Learning, Names (DNS), Privacy & log, Managed by IT, Diagnostics (the report), About; plus Tunnels, first (`proposals/settings-tunnels.html`): the tunnels in traffic order, Edit / Remove…, + New tunnel |
| `log.html` | Day chart and decisions as sentences, filters, search, Copy for helpdesk |
| `tunnel-detail.html` | Facts, policy, today's chart, learned / pinned / gateway hosts, add a host |
| `sign-in.html` | Browser sign-on waiting, one-time code, couldn't reach gateway |

Open any of them directly in a browser. Add `?dark=true` or `?dark=false` to
force a theme.

## Platform notes (Ubuntu / GNOME)

- The tray uses StatusNotifierItem/AppIndicator. Its native menu can't be
  styled, so the rich panel is Crake's own popover window anchored under the
  icon. Keep a plain native menu as a fallback (tunnels with check marks, Open
  Crake, Quit).
- Notifications use the system's. Default: a quiet notification per routed
  host with an Undo action. Optional "crex crex" sound, off by default.
- Passwords live in the system keyring (Secret Service), never in Crake's files.
