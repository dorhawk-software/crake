# Identity and delight: instruments with a personality

Researched October 2026 for Crake's design pass. The question: what makes a
quiet utility memorable without decoration, and what Crake, "an instrument,
not an app" named after a bird "heard far more often than seen"
(`design/DESIGN.md`), can take from six products admired for exactly that.

## How to read this

Each reference gets one section: the version or period described, what it
does in grouped bullets, and a **For Crake:** paragraph naming the pattern
and where it goes (screen and section, in Crake's copy voice). Every claim
about a product links to its source. Points from using or looking at a
product, with no published source found, are marked *(observed)*. Where a
source was looked for and not found, it says so. DESIGN.md's "Directions we
tried and rejected" overrides every reference: what resembles a rejected
direction is not recommended, and is listed under **Not for Crake** at the
end with the reason. Then **Moments Crake doesn't draw yet** proposes three
moments, checked against what craked and the app do today
(`docs/engineering/milestones.md`, the code named in each).

| # | Reference | Period described | Why it's here | What Crake takes |
| --- | --- | --- | --- | --- |
| 1 | Teenage Engineering | OP-1 field (2022), its guide v1.7.0; interviews 2024–2026 | Restraint with a personality; a manual's voice | One fact a sentence; rules set before designing |
| 2 | Panic | Transmit 5, Nova, Playdate (2019–2026) | A company whose tools are liked, not just used | The icon test; one sound for one event; jokes off the product |
| 3 | Flighty | ADA 2023 ("Behind the Design", June 2023); site as of October 2026 | A utility judged on the bad day | The reason is the product; shine when it goes wrong |
| 4 | Linear | Redesign of March 2024; "A calmer interface", March 2026 | Precision as the brand | Fewer icons; "if most people don't notice, good" |
| 5 | Things 3 | 3.0 (May 2017) onward | One act, one mark, a moment of motion | The act's mark is the identity; motion then rest |
| 6 | Little Snitch | 4 (2017, the map), 6 (May 2024), for Linux (April 2026) | The closest relative: a network instrument | Mark the moment, not the stream; candour about limits |

---

## 1. Teenage Engineering: the manual's voice, the designer's rules

*Period: OP-1 field (2022) and its online guide v1.7.0; Jesper Kouthoofd's
interviews, February 2024 (SFMOMA) and 2026 (Highsnobiety).*

**The voice**
- The OP-1 field guide (v1.7.0) is terse and matter-of-fact: "to keep the
  battery healthy, the unit should be charged at least every 6 months.";
  "clean the shell of the unit with a lightly damp cloth. let dry before
  usage." Sections are plain labels, not numbered chapters, and the body is
  lowercase throughout
  ([OP-1 field guide](https://teenage.engineering/guides/op-1)).
- The product page understates: "if it feels thinner. it is."; "we didn't
  want to alter the original OP–1 too much, because we think we got it more
  or less right the first time"; "we also meticulously reworked all
  graphics, screen by screen"
  ([OP-1 field](https://teenage.engineering/products/op-1)).

**The method**
- "I set up restrictions when I design." "I stick to rules and try to
  understand them, because then I can become free again." "I always work
  with simple geometric shapes." "A product can be entertaining and be a
  tool" (Kouthoofd, February 2024,
  [SFMOMA](https://www.sfmoma.org/read/stay-curious-stay-naive-an-interview-with-teenage-engineering-jesper-kouthoofd/)).
- The design mixes "German and Italian": a system that holds, with play
  inside it (same source).
- The personality lives in the screen: "The OP-1 is known for its cartoonish
  user interface," and the company pairs "minimalist industrial design with
  a quirky, almost childlike sense of humor"
  ([Highsnobiety, 2026](https://www.highsnobiety.com/p/teenage-engineering-interview/)).

**For Crake:** Take the guide's sentence shape, not its lowercase. Crake's
"?" help cards (Settings, the panel's "What Crake did") already do it best
when each sentence holds one fact and the example comes last, as Keep
internal names private's does: "Names like *.acme.internal never go to
your internet provider's DNS. Example: with ACME off, looking up
git.acme.internal simply fails at once." Write every card to that rule.
Take TE's understatement for What's new and About: the About line "Because a VPN shouldn't be a chore." is the right
amount, and none should be added. Take the habit of fixed rules: DESIGN.md
already is Crake's set of restrictions; a design pass adds to it rather
than around it.

---

## 2. Panic: liked tools, and where the jokes live

*Period: Transmit 5 (current page, macOS 13+), Nova (announced 2019, ADA
finalist 2021), Playdate (announced May 2019, designed with Teenage
Engineering; [Wikipedia](https://en.wikipedia.org/wiki/Panic_Inc.)).*

**Copy**
- Transmit's page praises itself in four words: "It's quite good." "It's
  very fast." "Never type a server address again."
  ([Transmit](https://panic.com/transmit/)).
- Playdate's FAQ answers like a person: "Is it a gimmick? Nah. Does it
  charge Playdate's battery? Nope." "Will you love them all? Probably not."
  ([play.date](https://play.date/)).
- The jokes have a page of their own: a mock proclamation "of the governor
  of Panictown" against "pixel-poachin', gif-grabbin', layout-liftin'
  bandits" ([Panic, ripoff](https://panic.com/extras/ripoff)).

**Icons and character**
- Neven Mrgan (Panic) on icons: "Will your icon idea make sense to a busy
  person working in a dark room?" On character: "Does it say something,
  mean something, is it an 'it' or an 'It'?"
  ([Shawn Blanc, 2010](https://shawnblanc.net/?p=2449)).
- Mrgan judges designs "in terms of interaction: how they behave under what
  circumstances" (same source).
- Transmit's icon is a truck carrying files *(observed)*. No published
  account of who drew Nova's icon or why was found.

**Playdate's system software**
- Settings are "intentionally utilitarian", with one illustration (cables
  showing how deep you are); the crank has a "delightful little sound
  effect" when flipped out; the design intent is "to surprise the user, as
  well as not overwhelming them"
  ([Clip Content, March 2023](https://clipcontent.substack.com/p/the-playful-design-details-of-the)).

**For Crake:** Three things. **The dark-room test** is the tray bird's
test, and it is already passed the hard way (bird-calling.html, four
rounds): keep using Mrgan's sentence as the bar for any new mark (a state,
a badge). **One sound for one physical moment**: Playdate's crank sound is
tied to one act. Crake's equivalent exists, the synthesized "crex crex" in
Settings → General, "When Crake routes a host", off by default. Keep it to
that one event; never a sound for connecting, a drop or a click. **Jokes
live off the product**: Panic's humour sits on web pages, not in Transmit's
transfer queue. Crake's copy rules ("No exclamation marks, no 'Oops'")
stay; the corncrake joke stays in About ("Named after the corncrake, *Crex
crex*.") and on dorhawk.software/crake. Playdate's "utilitarian settings
with one picture" is what Settings already is: the bird's own row is the
one picture.

---

## 3. Flighty: built for the bad day

*Period: Apple Design Award winner 2023, "Behind the Design" (5 June
2023); flighty.com as of October 2026.*

**The analogy**
- "That's our real-world analogy. Those airport boards have one line per
  flight, and that's a good guiding light — they've had 50 years of figuring
  out what's important." "We want Flighty to work so well that it feels
  almost boringly obvious." (Ryan Jones,
  [Apple, Behind the Design](https://developer.apple.com/news/?id=970ncww4)).
- The app icon is a flip board with "Flighty" on two pieces
  ([Basic Apple Guy](https://basicappleguy.com/basicappleblog/boardy)).

**When it goes wrong**
- "We really have to shine when things go awry." Before take-off the Live
  Activity switches to a minimal progress presentation, since "we have to
  assume that we won't see them again until they land"
  ([Apple](https://developer.apple.com/news/?id=970ncww4)).
- Alerts are "instant, clear, and unfiltered"; the app gives "the exact
  reason behind delays", and says why in plain words: "Late planes are the
  #1 cause of delays." It tracks the inbound aircraft "25 hours before your
  flight" ([flighty.com](https://flighty.com/)).

**For Crake:** **The reason is the product.** Flighty's identity is that it
says *why*, and Crake's is the same: "Sent git.acme.internal through ACME."
with "It didn't answer directly. Through ACME it answered in 38 ms." under
it. Every new message keeps that two-line shape, with the one number that
matters. **One line per tunnel**: the panel's Tunnels list (tray-panel.html)
is Crake's departure board: name, state, policy, one row each; resist
adding a second number to a row. **Shine when it goes awry**: Crake's bad
day is a tunnel that drops. Today craked logs "ACME lost the gateway.
Trying again." and, once up, "ACME is back.", while the panel row only says
"Connecting…", the same words as a first connection. That is the
unfinished moment, proposed below (Moments, 2). **Unfiltered**: Flighty
says it before the airline does; Crake's "No answer just then, directly or
through ACME. Nothing was changed." is that honesty, kept.

---

## 4. Linear: precision you feel, not see

*Period: the redesign of 28 March 2024; "A calmer interface for a product
in motion" (Charlie Aufmann and Maxime Heckel, 12 March 2026); the Linear
Method.*

**2024**
- The aim was "to reduce visual noise, maintain visual alignment, and
  increase the hierarchy", a "more neutral and timeless appearance", less
  chrome colour, labels, icons and buttons aligned "both vertically and
  horizontally": "something that you'll feel after a few minutes of using
  the app"
  ([Linear, How we redesigned the Linear UI](https://linear.app/now/how-we-redesigned-the-linear-ui)).

**2026**
- The sidebar went "a few notches dimmer" so the work leads; separators were
  softened; "The refresh reduces icon usage, scales their sizes down"; a
  warmer grey; and the test: "If most people don't immediately notice what
  changed, that's probably a good sign"
  ([Linear, A calmer interface](https://linear.app/now/behind-the-latest-design-refresh)).

**The Method**
- "Your tools should not make you the designer and maintainer of them." "A
  tool should work for you, not the other way around."
  ([Linear Method](https://linear.app/method/introduction)).

**For Crake:** Linear's 2026 move is the one Crake already made in its own
rules: fewer icons (Settings rows carry none but the bird's; the user's
"quiet over stock icons"), dimmer orientation, one title row measured to
the pixel ("Every window's starts 20 px from the left and its hairline
sits at 56"). Take **the test** as a rule for every polish pass in this
design round: if most people don't notice what changed, but the screen
reads faster on a helpdesk screen share, it worked. Take **the Method's
sentence** as the plain statement of Crake's whole idea, never to be quoted
in the app but to be argued from: the user doesn't write split-tunnel
rules; Crake learns them and shows its work. New tunnel's step 3 (routing
mode) is where that shows: "Learn as it goes" ("Only hosts that don't
answer directly.") stays the recommended card.

---

## 5. Things 3: one act, one mark

*Period: Things 3.0 (May 2017) and the 3.x line since.*

- "Everything you do in Things is nicely animated for pop." "The
  interactions are delightful. The animations are smooth." All of it
  "shipped with Things 3.0"
  ([Cultured Code, features](https://culturedcode.com/things/features/)).
- MacStories' review (May 2017): "Things is full of little delights"; "The
  app is dominated by white space"
  ([MacStories](https://www.macstories.net/reviews/things-3-beauty-and-delight-in-a-task-manager/)).
- The signature is a standard control with one extra behaviour: the Magic
  Plus button, dragged to where the new to-do goes (same sources).
- The app icon is the blue checkbox, the same mark you tap all day; a
  completed to-do stays in place, ticked, before it leaves for the Logbook
  *(observed)*. No Cultured Code source was found for the completion sound
  or the timing of the move to the Logbook.

**For Crake:** Things shows that **the mark of the act is the identity**:
the checkbox is the icon because it is the thing you do. Crake's
two-stroke tick is the same: it marks a route in the panel's "What Crake
did", in every chart, in tunnel detail's Learned tab, and the tray's call
marks are the same two strokes. Keep it the only teal mark. Things also
shows **motion, then rest**: the tick plays once, the row settles, the
record keeps it. Crake has the same sequence built (7c: a new decision row
slides in, 220 ms; a new tick grows in two beats, 260 ms; the tray calls
twice, then holds the open bill for 4 s, `ROUTED_SHAPE_FOR`), and the record
is the log. Nothing to add here but a rule: no motion on a row that wasn't
a decision.

---

## 6. Little Snitch: a network instrument, and what it draws

*Period: Little Snitch 4 (June 2017, the map), 6 (May 2024), Little Snitch
for Linux (April 2026, 1.0.8 in May 2026).*

**The map, and its effect**
- Version 4 made the Network Monitor's centrepiece "a global map, showing
  you effectively in real time all the recent and active connections"; the
  locations are "IP-derived or otherwise guessed"; "It's a little
  terrifying" (Glenn Fleishman, September 2017,
  [Macworld](https://www.macworld.com/article/230435/little-snitch-4-review-mac-app-excels-at-monitoring-and-controlling-network-activity.html)).

**Marking the moment**
- A blocked connection shows "in realtime in Little Snitch Network Monitor
  by a red flashing of that connection", with "Show Corresponding Rule" on
  it ([Objective Development support](https://obdev.at/support/littlesnitch/md3i7)).
- The menu bar icon can show "a compact chart showing the traffic amounts
  of the last five minutes" and "a flashing highlight in the menu bar icon
  whenever a network connection was blocked"
  ([Little Snitch 6 Help, status menu](https://help.obdev.at/littlesnitch6/pref-statusmenu)).
- Version 6 adds "Sound Notifications": "Stay informed about network
  activities through customizable acoustic notifications, just by
  listening" ([What's new in Little Snitch 6](https://obdev.at/littlesnitch/whatsnew.html)).

**Candour**
- The Linux edition (eBPF, a web UI on localhost:3031) is plain about its
  limits: "built for privacy, not security, and that distinction matters",
  and it is "not the right tool" against a determined adversary; it opens on
  apps that "open a network connection, quietly, without asking"
  ([Little Snitch for Linux](https://obdev.at/products/littlesnitch-linux)).

**For Crake:** Little Snitch is the closest relative and the clearest
warning. It draws the stream (a map, a chart in the icon, a flash per
block); Crake marks only **its own decisions** in a stream it leaves alone.
Take **the moment-mark**: Little Snitch's highlight on a block is the
tray's call on a route, and Crake's is rarer and so means more; keep it per
decision, never per connection. Take **"just by listening"** as written
proof that a network tool can be heard more than seen: the "crex crex"
notification sound is the corncrake's own idea of a status light. Take
**the candour**: "not the right tool" is the voice of Crake's Fortinet EMS
stance ("This gateway needs FortiClient managed by EMS…", never spoofed)
and of "Left 10.62.0.77 alone." Take **"Show Corresponding Rule"** as a
standard for the log: a "Left alone" line names what left it there (IT's
scope, the never list, or no answer), as the reasons already do.

---

## The four patterns that matter most

1. **The reason is the identity** (Flighty, Little Snitch's corresponding
   rule). Every line Crake writes says what happened and why, with one
   number. Nothing else Crake could draw is as recognisable.
2. **Mark the decision, never the stream** (Things' checkbox, Little
   Snitch as the counter-example). The two-stroke tick and the bird's call
   happen once per decision, then rest. Teal stays rare because of it.
3. **Shine on the bad day** (Flighty). A dropped tunnel is the moment a
   VPN client is judged; Crake's is half-drawn today (Moments, 2).
4. **If most people don't notice, it worked** (Linear 2026), with TE's
   rule-first method: polish removes rather than adds, and new marks pass
   the dark-room test (Panic) or don't ship.

---

## Moments Crake doesn't draw yet

Each proposal names its trigger, where it shows, how long, and the words.
"Built" means it exists today; **New work** marks what doesn't. Per the
user's process, each goes through a proposal with before and after before
it is built.

### 1. The first route Crake ever learns

**What exists.** With a tunnel up and no decisions, "What Crake did" says "Nothing yet. Crake routes nothing until
something needs a tunnel." (built, `panel/decisions.rs`). Every route by
Crake itself (not one the user made) gets a quiet notification for 8 s with
Undo and Open Crake (`notify::routed`, `ROUTED_SHOWN`), the "crex crex" if
chosen, the tray's call (twice, then the open bill for 4 s), a row sliding
into "What Crake did", and its log line. Nothing tells the first one apart.

**The gap.** The first route is when Crake's idea becomes visible: one
host, and only that one. Say it once, plainly, and never again.

**Trigger** (*new work*): the first log entry that `notify::worth_telling`
accepts (a route Crake decided, by probing or by DNS) on an install whose
host store had no learned hosts before; marked seen in app.json (as
`seen_version` is), so an upgrade from a version that already routed
never shows it.

**Where and how long.**
- **Panel**, under the new row in "What Crake did": a neutral notice in the
  notices' shape (surface tint, no amber: nothing needs you; no teal:
  nothing new was decided by the notice itself), until "Got it". The row
  above it stays the ordinary row, with its tick and Undo.
- **Notification**: the ordinary one, the ordinary 8 s, with one sentence
  added to its body, this once.
- **Tray bird and sound**: exactly as for every route. The first call is
  not louder; that is the point of an instrument.
- **Log**: the ordinary line. The log records decisions, not milestones.

**Words.** Notice title (600): "Crake's first route." Body:

> Only git.acme.internal goes through ACME. Everything else still goes
> direct. Each time Crake sends a host somewhere, it shows here, and Undo
> puts it back.

Action: "Got it" (the bordered button). With "Hide names and addresses"
on, the panel still names the host (that setting is for sharing). For a
host learned by DNS, the same words; the row's reason already says "Only
ACME's DNS knows the name. Routed before the first packet."

Notification, this once:

> **Sent git.acme.internal through ACME.**
> It didn't answer directly. Through ACME it answered in 38 ms. Everything
> else still goes direct.

### 2. A tunnel coming back after it dropped

**What exists.** On openconnect's `attempt-reconnect`, craked sets the
tunnel Connecting and logs "ACME lost the gateway. Trying again." with no
reason (`manager/hook.rs`); the tray shows the dashed bird going round
(`connecting_loop`); the panel row says "Connecting…", the same words as a
first connection (`model::tunnel_state`). When the hook says `connect`
again, craked re-applies routing, clears what was probed meanwhile so those
hosts are tried again at once, and logs "ACME is back." with the tunnel's
summary as the reason (`manager/tunnels.rs`, `tunnel_up`). OpenVPN 3 logs
"Globex is reconnecting." / "OpenVPN 3 is setting its session up again."
(`manager/attached.rs`). If openconnect gives up, the tunnel stops: the red
badge and a notification that stays until dismissed, with Switch on
(`notify::stopped`, built). No notification is shown for a drop that heals.

**The gap.** During the drop the panel can't tell "starting" from "lost
it"; after it, nobody learns how long it was gone or that the routes held,
which is exactly what someone whose ssh session froze wants to know.

**Trigger.** The existing `attempt-reconnect` (and OpenVPN 3's
reconnecting state), then the existing "is back".

**Where and how long.**
- **Panel row** (*new work*, wording only): while trying, the state line
  reads "Lost the gateway · trying again…" in muted, never red (nothing has
  stopped) and never amber (it doesn't need you). Back, it returns to
  "Connected · 5 hosts". No transient "back" text in the row: it would
  change the row's width and vanish under the pointer.
- **Tray bird**: the dashed bird while trying (built), then the ordinary
  return to listening. **No call**: the call means Crake decided something,
  and a reconnect is the gateway's doing.
- **Log** (*new work*: craked keeps the time of the drop): both lines get
  reasons.
- **Notification** (*new work*): only when the drop lasted a minute or
  more, low urgency, 8 s like a route, no sound, and only when Settings →
  General isn't "Say nothing". A shorter drop is heard in the log, not
  seen. The threshold is the user's call; a minute is the proposal.

**Words.** Log, on the drop:

> **ACME lost the gateway. Trying again.**
> Its 5 hosts can't be reached until it's back. Everything else goes
> direct as usual.

Log, on return (the tunnel's summary stays after it):

> **ACME is back.**
> Gone for 42 s. Its 5 hosts go through it again, and hosts that found no
> answer meanwhile will be tried again.

Notification, after a minute or more: "ACME is back." / "Gone for 3 min.
Its 5 hosts go through it again."

OpenVPN 3 should read the same ("Globex lost the gateway. Trying again.")
with its reason naming who is trying: "OpenVPN 3 is setting its session up
again." A full tunnel's drop says "Nothing goes out until it's back."
instead.

### 3. The line a user reads out to a helpdesk

**What exists.** "Copy for helpdesk" is in the panel's footer and the log's
title row, and on the sign-in window when it fails (built). It copies plain
text: "Crake, 2026-10-29 13:24 CET", then "Tunnels" ("ACME: Connected · 5
hosts, address 10.40.12.7"), "Recent decisions (newest first)" with each
reason indented, and "Crake's view of this computer"; names and addresses
become "a host" when Privacy & log says so (`model::helpdesk`). The sign-in
copy is "Couldn't sign in to Nordbank (gw.nordbank.example, anyconnect)."
and the reason (`signin_view`). "Copied" shows, muted, for 3 s.

**The gap.** The copy is written to be pasted. On a call, the helpdesk
asks "what does it say?", and the user reads from the screen. The person
listening has never heard of Crake, expects their company's client, and
thinks "everything goes through the VPN". The first thing they need is one
sentence that tells them what this client is and whether the tunnel is up,
without "·", abbreviations or a word they'd have to ask about.

**Trigger.** Always current (it is state, not an event): refreshed whenever
the panel or tunnel detail is.

**Where** (*new work*):
- **The first line of every helpdesk copy**, before "Tunnels".
- **Tunnel detail**, as one line under the facts (13 px, muted, selectable),
  the same sentence for that tunnel: the only place a person on a call
  already looks for "your address" and "gateway". No button beside it; the
  title row keeps its actions as they are.
- **A stopped tunnel**: the sentence is the panel's red words with the
  time, so the user reads the same thing they see.

**Words.** Connected, first line of the copy and tunnel detail's line:

> I'm using Crake 0.15.0, which connects with openconnect. ACME has been
> connected to vpn.acme.example since 08:31, address 10.40.12.7. Only the
> hosts that need ACME go through it: 6 today.

A host in question adds its log line as it stands, which already reads
aloud: "Left 10.62.0.77 alone. No answer just then, directly or through
ACME. Nothing was changed."

Stopped:

> ACME stopped at 13:40: the gateway didn't answer for 20 seconds. My
> internet works, and Nordbank is still connected.

With "Hide names and addresses when sharing" on: "ACME has been connected
since 08:31. Only the hosts that need ACME go through it: 6 today."

The first sentence naming openconnect is deliberate: it is what a helpdesk
can look up, and it says at once that Crake is not their sanctioned client,
which is honest (the same reason Crake never poses as FortiClient).

---

## Not for Crake

Patterns from these references that resemble a rejected direction, or
break the one-accent or read-on-sight rules.

- **The OP-1's cartoon screen, aluminium, knobs, tape reels, and TE's
  field-recorder look.** Skeuomorphic hardware: "reads as a toy, fails
  contrast, and won't pass IT review" (DESIGN.md).
- **TE's all-lowercase copy.** A typographic mannerism; Crake writes
  sentence case so a helpdesk can read it on a screen share.
- **Little Snitch's world map.** "The internet is never drawn. There's no
  map" (DESIGN.md, Shape). Its own reviewer called it "a little
  terrifying"; Crake is meant to calm.
- **A traffic chart in the tray icon, and a red flash per blocked
  connection.** The bird is the icon; a chart in the bar is a generic
  dashboard, a flash per event is a stream of blinks, and red means stopped
  or failed. One call per decision.
- **Flighty's flip-board icon and split-flap motion.** Hardware imitation,
  and colour on the icon; the app icon is the bird in one ink.
- **Lifetime stats in the manner of Flighty's Passport** ("212 hosts
  learned since March"). Numbers that answer no question: "Quiet by
  default" and the generic dashboard rejection.
- **Panic's illustrated objects in colour, the wanted-poster humour, and
  Playdate's "Nah." in the product.** The bird in colours was dropped (one
  ink everywhere), and Crake's copy has no jokes; the joke is the name, in
  About.
- **Playdate's angled menu swipe "to surprise".** Motion for its own sake;
  Crake's motion is for things that happen.
- **Things' "pop" on every interaction and its to-do turning into "a
  clear white piece of paper".** Motion on every act, and paper imitation.
  Crake animates decisions, the panel opening and switches, and stops
  there (7c).
- **Linear's user themes and accent choice.** One accent, fixed.
- **Confetti or a celebration for the first route** (Raycast's confetti,
  from the Godwit study). No exclamation, no marketing; the first route is
  told once, in the ordinary voice.
- **A pulsing dot for "trying again", or a coloured edge on the first-route
  notice.** Both rejected; the state is said in words and by the tray's
  dashed bird, the notice by its words and its surface.
- **Little Snitch's Once / Until Quit / Forever choice up front.** Not a
  look but a behaviour: Crake decides and lets you undo, except in the
  "Ask me" mode the user chose.

---

## Sources

Every fact above links its source inline. Crake's own: `design/DESIGN.md`,
`design/screens/*.html`, `design/proposals/first-run-bird.html`,
`docs/engineering/milestones.md` (6, 7, 7c), craked's `manager/hook.rs`,
`tunnels.rs`, `attached.rs`, and the app's `notify.rs`, `model/mod.rs`,
`panel/decisions.rs`, `signin_view/mod.rs`.

Not found: a Cultured Code source for Things' completion sound or Logbook
timing; who drew Nova's icon; a Panic write-up of the Transmit truck; a
Teenage Engineering statement on the OP-1's screen graphics beyond the
product page.
