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
- **Tray marks above the head** (design/proposals/tray-marks-above.html).
  The bar icon is a fixed square: room above the head shrinks the bird.
  The call marks stay off the beak and the "!" on the back of the head.

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

One accent. Teal means **"Crake made a decision"**, nothing else. Two status
colours, each with one meaning too: red, **something stopped or failed**;
amber, **it needs you, nothing has broken yet** (relaxed 2026-10-15, asked for
by the user: "it must be tasteful, but maybe some colour, and an icon here
and there"; design/proposals/leftover-warning.html). Amber colours a whole
notice (its tint, its hairline) and its icon, the bird's head with an amber
triangle; never an edge on one side of a box, never decoration.

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
| danger | `#B42318` | `#F97066` | destructive actions, failed step; a window's × pointed at |
| onDanger | `#FFFFFF` | `#0B1514` | the × on danger |
| warning | `#E39A1B` | `#F5B740` | the triangle on the bird's head: needs you |
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
- 1 px hairlines. No heavy borders, no offset shadows, no gradients.
- Switches are the standard rounded pill; that's the platform's language.
- The internet is never drawn. There's no map; the log and charts carry it.

## The signature: the two-stroke tick

Crake's one ownable mark is a **two-stroke tick**: a taller and a shorter
vertical bar side by side, the "crex crex" call. It means **routed**.

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

## Charts

One rule across every chart:

- **Grey bars = how much traffic.** Square ends, sitting on the baseline. The
  chart title says which traffic ("Last hour" in the panel = direct,
  "Today" in the log = everything, "Today through ACME" in tunnel detail).
- **Teal two-stroke ticks = the moment Crake routed a host.**
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

**In the bar: the head alone** (`design/icons/tray-head-*.svg`). Top bars are
small and app icons beside it are solid squares, so the tray icon is the
crake's head, solid, with the eye cut out (the bar shows through), cropped
square so it fills the slot. White; the states:

| State | Shape |
| --- | --- |
| off | asleep: the head tipped down 25° and dimmed to 45 %, eye closed (an eye alone was too small a difference at 16 px; design/proposals/tray-off-vs-on.html, D) |
| connecting | the head drawn dashed |
| listening | eye open |
| routed (a few seconds) | the two call marks over the corner, **teal** |
| trouble | a **red** badge with "!" over the corner of the head as it would be without it: asleep when nothing is up, dashed while another tunnel connects, awake while one is up (design/proposals/dismiss-trouble.html) |
| needs you | an **amber** triangle with "!" in the same corner, on the head as it is (`tray-head-warning.svg`, `-off-`, `-connecting-`): an OpenVPN 3 process left running, until it ends or "Got it" (design/proposals/leftover-warning.html). Red wins: a tunnel that stopped shows trouble's badge instead |

The head changes shape, never colour; only the state marks are coloured
(dark-mode accent, danger and warning, the bar is dark), with a gap cut into the head
around them so they read on any bar colour. The whole bird with grass
(`crake-*-symbolic.svg`) stays for places with room; `CRAKE_TRAY=bird` shows
it in the bar.

Whole bird (larger places):

| State | Shape |
| --- | --- |
| off | grass only |
| connecting | head drawn dashed |
| listening | head up, eye |
| routed (a few seconds) | head plus two call marks at the beak |
| trouble | head plus "!" at top left |

The panel header uses the listening bird with the call marks in accent.

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
- Segmented controls frame the chosen segment in its own colour (each
  segment draws its own edges): a grey rim around dark teal reads as blur
  in light mode, as it did in settings.html and log.html.

## Screens

| File | What it shows |
| --- | --- |
| `tray-panel.html` | The main surface: last hour, what Crake did, tunnels with policy, full log / copy for helpdesk |
| `new-tunnel.html` | Three steps: server and protocol (auto-detect, AnyConnect/ocserv, GlobalProtect, Ivanti/Pulse, Juniper NC, Fortinet, F5, Array), sign-in method, routing mode |
| `settings.html` | General, Learning, Names (DNS), Privacy & log, Managed by IT, About; plus Tunnels, first (`proposals/settings-tunnels.html`): the tunnels in traffic order, Edit / Remove…, + New tunnel |
| `log.html` | Day chart and decisions as sentences, filters, search, export |
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
