# Crake: the identity pass's inventory

What this is: every surface, state, control, transient surface, system-drawn surface, process and
motion the app has, as a checklist for the identity pass. Nothing in it is a design grade.

Method: two inventories were made from the code alone on 2026-11-27, one by sweeping every file,
one by following the entry points and the strings. They were compared line by line, and every
difference was settled by opening the code (nothing was built or run). Section 8 says how.

Grades: every line starts `- [ ] not seen` until it is graded: matches, drifts (the identity is
there, details differ), misses, or not drawn in the identity (the canvas has no board for it).
The eight windows captured on 2026-11-27 are graded as wholes; their parts and states are not.
The profile editor is still not seen (demo mode did not open it).

At first: Nobody has graded anything yet. A line is ticked and
its grade written in place of "not seen" once someone has looked at that item in the running app.

Paths are relative to `crates/crake/src/` unless they start with `craked/`, `crake-proto/`,
`crake/` (then relative to `crates/`), `packaging/` or `assets/` (the repository root). Line
numbers are of the working tree on 2026-11-27. On-screen strings are quoted from the code, with
plain `'` and `…` where the code mostly writes the typographic ones (`\u{2019}`, `\u{2026}`).
`<x>` stands for a value filled in at run time.

## 1. Windows and views

### Windows (9, one per `open_window` call)

- [x] drifts (captured 2026-11-27, macOS, demo, light and dark): "What Crake did" has marks but no line; the foot is three bordered buttons (Traffic too), the canvas has one bar of three · Panel: undecorated pop-up at the tray icon, 452 px wide, height fitted, closes on
  focus loss · windows.rs:167 (opened in `open_panel`, :99), view panel/mod.rs:163
- [x] drifts (captured 2026-11-27: Tunnels, General, Learning): rows sit in outlined cards, the canvas has hairlines; General, Names, Privacy, Diagnostics and About carry stock icons; "+ Add" is dashed with a plus · Settings ("Crake settings", 1040 × 820, not resizable, never taller than the
  screen) · windows.rs:257, view settings_view/mod.rs:190
- [x] drifts (captured 2026-11-27, light and dark): the four facts sit in an outlined grid and the policy in a tinted box; "Forget all…" and the counts over the bars are not on the canvas · Tunnel detail ("Crake tunnel", 900 wide, one at a time, re-used for another
  tunnel) · windows.rs:316, view tunnel_detail/mod.rs:85
- [x] drawn on the canvas 2026-11-27, not built (the app captured 2026-11-27, step 1, light and dark): no board on the canvas · New tunnel / Edit tunnel ("Crake new tunnel", 720 × 860, shrinks per step, one at
  a time) · windows.rs:381, view new_tunnel/view.rs:123
- [x] drawn on the canvas 2026-11-27, not built (the app captured 2026-11-27: the "Couldn't reach" stage only): no board on the canvas · Sign-in ("Sign in to <tunnel>", 340 wide, fits its card, minimizable, one per
  tunnel) · windows.rs:444 (options :672), view signin_view/mod.rs:138
- [x] misses (captured 2026-11-27, light and dark): no line, branch, dot or race; filters are an outlined segment row; a hairline under every row · Log ("Crake log", 980 wide) · windows.rs:492, view log_view/mod.rs:79
- [x] drawn on the canvas 2026-11-27, not built (the app captured 2026-11-27, light and dark): no board on the canvas; its bars and marks already follow the chart rules · Traffic ("Crake traffic", 920 × 860) · windows.rs:553, view traffic_view/mod.rs:469
- [ ] not seen · Profile editor ("<tunnel>'s profile", 820 × 640, the only resizable window) ·
  windows.rs:622 (`is_resizable: true` :634), view profile_editor/mod.rs:447
- [x] drawn on the canvas 2026-11-27, not built (the app captured 2026-11-27, light and dark): no board on the canvas · What's new ("What's new in Crake", 600 × 640) · whats_new/open.rs:29, view
  whats_new/view.rs:184

### Surfaces that are not windows

- [ ] not seen · Tray icon with its fallback menu, Linux (StatusNotifierItem) ·
  sys/linux/tray.rs:26-150
- [ ] not seen · Tray icon with its menu, Windows and macOS · sys/menu_tray.rs:83, start :119
- [ ] not seen · Terminal: `crake report [file] [--hide-hosts|--show-hosts]` (no window at all) ·
  report.rs:300-340, main.rs:309-311
- [ ] not seen · Terminal: `craked --connect <tunnel>` console and `--dev` lines (the service's
  own, not the app's) · craked/src/main.rs:157-403

### Panel: parts

- [ ] not seen · Header: bird with its marks (call, red badge, amber triangle), "Crake", status dot
  and word, settings button; drags the window · panel/mod.rs:519-594, marks :147-160
- [ ] not seen · No-tray note above everything ("Crake has no icon in the tray.") ·
  panel/no_tray.rs:10-73
- [ ] not seen · First-run body: bird in grass, "No tunnels yet", sentence, "Add a tunnel…" ·
  panel/first_run.rs:69-139
- [ ] not seen · Service-not-there body (replaces middle and footer) · panel/mod.rs:254-324
- [ ] not seen · Scrolling middle, in order: questions, version, leftovers, staying, chart,
  decisions, tunnels · panel/mod.rs:198-219
- [ ] not seen · Question notice (a short name waiting) · panel/question.rs:14-117
- [ ] not seen · Version notice · panel/version.rs:9-83
- [ ] not seen · Leftover OpenVPN 3 process notice with its command chip · panel/leftover.rs:13-110,
  chip :115
- [ ] not seen · "Stays on OpenVPN 3" notice · panel/staying.rs:10-103
- [ ] not seen · "Last hour" chart with its legend (direct, routed) · panel/chart.rs:18-85, element
  timeline.rs:167
- [ ] not seen · "What Crake did" heading with its "?" · panel/decisions.rs:71-141
- [ ] not seen · Decision rows (at most 3) · panel/decisions.rs:143-256, limit panel/mod.rs:69
- [ ] not seen · First-route notice under its decision · panel/moments.rs:13-87
- [ ] not seen · "<X> answers for it too." line under a route · panel/moments.rs:91-143
- [ ] not seen · Short-name choices on a decision row · panel/decisions.rs:261-287
- [ ] not seen · "Tunnels" section: a row per tunnel (name, state line, Dismiss, Managed chip,
  policy line, switch) · panel/tunnels.rs:6-118
- [ ] not seen · "Acts as AnyConnect" notice under a tunnel · panel/tunnels.rs:122-163
- [ ] not seen · Browser DNS-over-HTTPS notice under the first learning tunnel ·
  panel/tunnels.rs:167-208, text :267-293
- [ ] not seen · Failure sentence at the foot (red, 8 s) · panel/mod.rs:224-235
- [ ] not seen · Footer: Traffic, Full log, Copy for helpdesk · panel/mod.rs:597-659

### Settings: parts

- [ ] not seen · Sidebar: bird, "Settings", 8 section rows · settings_view/mod.rs:531-611, names
  and subtitles :80-122
- [ ] not seen · Main column: drag strip with close, section title and subtitle, body, failure
  sentence · settings_view/mod.rs:242-338
- [ ] not seen · About's subtitle "Named after the corncrake, Crex crex" with its link ·
  settings_view/mod.rs:292-312
- [ ] not seen · Tunnels: ordered rows, "Everything else goes through:" group, "+ New tunnel",
  "Drag to change the order." · settings_view/tunnels.rs:13-105, row :110-290
- [ ] not seen · General: bird row, Start when you log in, Switch on the tunnels you left on, When
  Crake routes a host, Tell me when there's a new version, Appearance ·
  settings_view/general.rs:11-122, bird row settings_view/tray_row.rs:126-156
- [ ] not seen · Learning: Wait for a direct answer, When two tunnels answer, Forget hosts unused
  for, Never send through a tunnel · settings_view/learning.rs:7-185
- [ ] not seen · Names: Ask a tunnel's DNS for these names, Keep internal names private, Ask every
  tunnel when a name is unknown · settings_view/names.rs:7-92
- [ ] not seen · Privacy & log: statement card, Keep the log for, Hide names and addresses when
  sharing, Clear the log · settings_view/privacy.rs:7-155
- [ ] not seen · Managed by IT: a card per managed tunnel, footer sentence ·
  settings_view/managed.rs:5-100
- [ ] not seen · Diagnostics: "Save a report for someone helping you", button, "Or in a terminal:"
  command, status box · settings_view/diagnostics.rs:90-240
- [ ] not seen · About: bird, version, two sentences, links (What's new, Source on GitHub,
  dorhawk.software), "Is it safe?" (4 questions), coffee card · settings_view/about.rs:23-192

### Tunnel detail: parts

- [ ] not seen · Header: "Tunnels" back, Switch on/off, close · tunnel_detail/mod.rs:138-205
- [ ] not seen · Title row: name, state dot and words, Managed chip · tunnel_detail/facts.rs:6-61
- [ ] not seen · "Why" sentence under the title · tunnel_detail/facts.rs:330-366
- [ ] not seen · Facts: Connected for, Your address, Protocol, Gateway or Profile ·
  tunnel_detail/facts.rs:68-180
- [ ] not seen · Policy card (who decides) · tunnel_detail/facts.rs:186-287
- [ ] not seen · "Today through <tunnel>" chart · tunnel_detail/chart.rs:9-83
- [ ] not seen · Host tabs with counts: Learned, Pinned, From the gateway, Short names ·
  tunnel_detail/hosts.rs:44-95
- [ ] not seen · "Kept" line under the tabs · tunnel_detail/hosts.rs:421-432
- [ ] not seen · Host row (mark, name, how learned, last used, Pin/Unpin, Forget) ·
  tunnel_detail/hosts.rs:330-404
- [ ] not seen · Gateway rows (DNS server, pushed networks) · tunnel_detail/hosts.rs:277-296,
  :676-697
- [ ] not seen · Short-name row · tunnel_detail/short_names.rs:9-50
- [ ] not seen · "Forget all…" with its question in place · tunnel_detail/hosts.rs:499-617
- [ ] not seen · "Add a host" field with "Send through <tunnel>" · tunnel_detail/hosts.rs:157-201
- [ ] not seen · Failure sentence under the form · tunnel_detail/hosts.rs:203

### New tunnel: parts

- [ ] not seen · Header: "Don't add it", title, close · new_tunnel/view.rs:76-121
- [ ] not seen · Steps bar: Server, Sign-in, Routing · new_tunnel/view.rs:6-72
- [ ] not seen · Step 1 Server: "Where does it connect?", Name, Gateway, detection line, Protocol
  list, Beta note, missing-engine note · new_tunnel/server.rs:111-249
- [ ] not seen · Step 1, protocol list and its fold "Choose the protocol yourself" ·
  new_tunnel/server.rs:256-358, fold :361-396
- [ ] not seen · Step 1, Group row · new_tunnel/server.rs:537-586
- [ ] not seen · Step 1, OpenVPN: Profile field and import line in place of Gateway ·
  new_tunnel/server.rs:13-109
- [ ] not seen · Step 1, kept gateway when editing · new_tunnel/server.rs:510-535
- [ ] not seen · Step 1, IPsec: "How the gateway proves itself" (A key / Its certificate) ·
  new_tunnel/ipsec_proof.rs:69-131
- [ ] not seen · Step 1, IPsec: "Gateway's key" field · new_tunnel/ipsec.rs:290-408
- [ ] not seen · Step 1, IPsec: "IT's certificate authority" file · new_tunnel/ipsec_proof.rs:133-216
- [ ] not seen · Step 1, IPsec: "Company domains" · new_tunnel/ipsec.rs:391-401
- [ ] not seen · Step 1, IPsec: strongSwan install command with Copy · new_tunnel/ipsec.rs:228-288
- [ ] not seen · Step 2 Sign-in: "How do you sign in?", method cards, detail, Advanced, "Stay
  signed in for 12 hours, if the gateway allows it" · new_tunnel/sign_in.rs:6-229
- [ ] not seen · Step 2, OpenVPN: "As the profile says" · new_tunnel/sign_in.rs:12-30
- [ ] not seen · Step 2, IPsec: methods, username, identity, certificate file ·
  new_tunnel/ipsec.rs:410-546, new_tunnel/ipsec_proof.rs:273-328
- [ ] not seen · Step 3 Routing: "What should go through it?", mode cards, list editor, "The
  gateway suggests", "Forget hosts unused for", summary sentence · new_tunnel/routing.rs:7-242
- [ ] not seen · Problem line (red) under the body · new_tunnel/view.rs:215
- [ ] not seen · Footer: Back, "Step n of 3", Continue / Add and connect / Save ·
  new_tunnel/view.rs:230-290

### Sign-in: stages

- [ ] not seen · Progress card with the steps list and "Don't connect" · signin_view/mod.rs:182-193,
  steps signin_view/widgets.rs:47-115
- [ ] not seen · Browser card · signin_view/mod.rs:153-181
- [ ] not seen · Question card: a choice list · signin_view/question.rs:34-66
- [ ] not seen · Question card: certificate password · signin_view/question.rs:67-89
- [ ] not seen · Question card: secret or text answer · signin_view/question.rs:90-136
- [ ] not seen · Question card: remember and keep checkboxes · signin_view/question.rs:141-190
- [ ] not seen · Trust question card · signin_view/question.rs:211-268
- [ ] not seen · Failed card · signin_view/failed.rs:9-80
- [ ] not seen · Failed card's checks list · signin_view/failed.rs:113-148, rows
  signin_view/checks.rs:73-110

### Log: parts

- [ ] not seen · Header: bird, "Log", retention note, Copy for helpdesk, close ·
  log_view/mod.rs:317-380
- [ ] not seen · Filters: Everything / Routed / Left alone / Undone, tunnel picker, search ·
  log_view/mod.rs:382-509
- [ ] not seen · Today head (summary; span and "Whole day" when zoomed) · log_view/day_chart.rs:41-92
- [ ] not seen · Day chart · log_view/day_chart.rs:98-303
- [ ] not seen · Day groups · log_view/rows.rs:9-57
- [ ] not seen · Row (time, kind mark, text, why, tunnel badge) · log_view/rows.rs:176-242
- [ ] not seen · "Show earlier days" · log_view/mod.rs:130-141

### Traffic: parts

- [ ] not seen · Header: "Traffic", "Live", "<rate> in all", close · traffic_view/mod.rs:484-516
- [ ] not seen · Graph: "This computer", the ways out (Direct and tunnels), destinations, routed
  ticks, moving dots · traffic_view/mod.rs:126-293, painting traffic_view/graph.rs:413
- [ ] not seen · "Last 5 minutes" strip with legend · traffic_view/mod.rs:295-370, paint
  traffic_view/strip.rs:86
- [ ] not seen · Tabs "Right now" / "Where things go" with counts · traffic_view/mod.rs:372-429
- [ ] not seen · Table "Right now" (Destination, Way, Now, Since) · traffic_view/mod.rs:448-453,
  traffic_view/tables.rs:180-255
- [ ] not seen · Table "Where things go" (Destination, Way, Why) · traffic_view/mod.rs:455

### Profile editor: parts

- [ ] not seen · Header: "<tunnel>'s profile", "<profile> · in OpenVPN 3", close ·
  profile_editor/mod.rs:455-490
- [ ] not seen · Body: the editor with line numbers, or a sentence · profile_editor/mod.rs:492-512
- [ ] not seen · Footer: note or error, Cancel, "Save and reconnect" · profile_editor/mod.rs:376-419
- [ ] not seen · "Leave without saving?" footer · profile_editor/mod.rs:333-374

### What's new: parts

- [ ] not seen · Header: "What's new", "Since <version>, the version you had", close ·
  whats_new/view.rs:72-119
- [ ] not seen · Release blocks (version, date, changes under New / Better / Changed / Fixed /
  Underneath) · whats_new/view.rs:122-181, kinds whats_new/changelog.rs:11
- [ ] not seen · "Show earlier versions" · whats_new/view.rs:196-216

### Popovers and overlays inside windows (5 deferred layers, a tooltip, a drag ghost)

- [ ] not seen · Panel "What Crake did" help card · panel/decisions.rs:101-128
- [ ] not seen · Settings "?" help card · settings_view/mod.rs:500-526
- [ ] not seen · Settings select menu · settings_view/widgets.rs:210-244
- [ ] not seen · New tunnel dropdown menu · new_tunnel/widgets.rs:54-96
- [ ] not seen · Log tunnel picker menu · log_view/mod.rs:433-471
- [ ] not seen · Full host name card (the one tooltip) · full_name.rs:71, :125-160
- [ ] not seen · Drag ghost of a tunnel row in Settings · settings_view/mod.rs:127-160

## 2. States

### The app as a whole

- [ ] not seen · Starting, service not yet found: status "Starting", tray tooltip "Crake", panel
  "Looking for Crake's service…" · model/mod.rs:529, :563, panel/mod.rs:259
- [ ] not seen · Service not running: status "Not running", tooltip "Crake isn't running", panel
  "Crake's service isn't running." + "Nothing is routed. This panel fills in by itself once it's
  back."; tray asleep, no badge · model/mod.rs:530, :562, :494-496, panel/mod.rs:272-275
- [ ] not seen · Service restarted or connection ended: the client connects again, nothing said
  (the tunnels are cleared meanwhile) · client.rs:468, :472, model/mod.rs:402-407
- [ ] not seen · macOS, service needs approval: "Crake's service needs your OK." + sentence + "Open
  Login Items" · panel/mod.rs:260-262, :301-322, decided service.rs:58-61
- [ ] not seen · macOS, service switched off later: "Crake's service is switched off." + button ·
  panel/mod.rs:263-265
- [ ] not seen · Windows, service refused: "Windows wouldn't start Crake's service." · service.rs:118
- [ ] not seen · Windows, service program missing: "Crake's service is missing." · service.rs:125
- [ ] not seen · Windows, start error: "Windows couldn't start Crake's service." / "Windows said:
  error <code>." · service.rs:129
- [ ] not seen · Windows, timed out: "Crake's service didn't start in time." · service.rs:133
- [ ] not seen · Windows, crashed: "Crake's service stopped unexpectedly." · service.rs:137
- [ ] not seen · Connected to the service, no tunnels: first run · panel/mod.rs:192-193
- [ ] not seen · Connected, tunnels: the full panel · panel/mod.rs:194-220
- [ ] not seen · Status word "Off" (grey dot) · model/mod.rs:527-536, dot panel/mod.rs:563
- [ ] not seen · Status word "Connecting" · model/mod.rs:527-536
- [ ] not seen · Status word "Listening" (teal dot) · model/mod.rs:527-536
- [ ] not seen · Status word "Trouble" (red dot, red badge on the header's bird) ·
  model/mod.rs:527-536, panel/mod.rs:523, :151
- [ ] not seen · Warning: amber triangle on the header's bird · panel/mod.rs:155
- [ ] not seen · Summary line "No tunnel is on" (tray tooltip and menu) · model/mod.rs:551
- [ ] not seen · Summary line "Listening · <names> · last route HH:MM" / "nothing routed today",
  with " · a question about <name>" · model/mod.rs:541-581, model/waiting.rs:50-55
- [ ] not seen · First start: the version is recorded as seen, so no What's new on a fresh
  install · whats_new/mod.rs:135, called main.rs:352
- [ ] not seen · Second start while running: opens the first one's panel and exits ·
  main.rs:549-562, :403-405
- [ ] not seen · Started by Restart: waits up to 10 s for the old one to go · main.rs:551-555
- [ ] not seen · No tray: panel note, Settings row or login notification, unless the bird is
  hidden · tray_host.rs:45, :119-141
- [ ] not seen · Bird hidden by the user: no tray icon, the panel opens from the app launcher ·
  requests.rs:106
- [ ] not seen · Appearance: follow the system · settings_view/general.rs:39-41, prefs.rs:27
- [ ] not seen · Appearance: light · settings_view/general.rs:39-41
- [ ] not seen · Appearance: dark · settings_view/general.rs:39-41
- [ ] not seen · Started at login (`--autostart`): no window; the tunnels left on are switched on
  once the service answers · autostart.rs:8, requests.rs:77-102

### Panel

- [ ] not seen · Decisions empty: "Nothing yet. Crake routes nothing until something needs a
  tunnel." · panel/decisions.rs:24-31
- [ ] not seen · Decision, routed: teal mark, Undo · panel/decisions.rs:154-201
- [ ] not seen · Decision, pinned: pin mark, no action · panel/decisions.rs:154-201
- [ ] not seen · Decision, undone: muted, Redo · panel/decisions.rs:154-201
- [ ] not seen · Decision, asked: help mark, "Send through <tunnel>" · panel/decisions.rs:154-201
- [ ] not seen · Decision, left alone: muted · panel/decisions.rs:154-201
- [ ] not seen · Decision, short name chosen / forgotten · panel/decisions.rs:154-201
- [ ] not seen · Decision, short-name question: "Use <tunnel>'s", one or several ·
  panel/decisions.rs:261-287
- [ ] not seen · Decision superseded: no button · model/mod.rs:607-627
- [ ] not seen · Decision arriving while the panel is open: slides in · panel/decisions.rs:53-61
- [ ] not seen · Chart with nothing direct: "Nothing went direct" · panel/chart.rs:50
- [ ] not seen · Chart with bars and marks; marks close together merge, with a count ·
  timeline.rs:291-312
- [ ] not seen · Chart pointed at: hairline and readout · timeline.rs:120-163, :240-287
- [ ] not seen · Tunnel row, off: name muted, "Off" · panel/tunnels.rs:38, model/mod.rs:328-337
- [ ] not seen · Tunnel row, "Connecting…" (also while it signs in in this app) ·
  model/mod.rs:328-337, :121-124
- [ ] not seen · Tunnel row, "Connected · 1 host" · model/mod.rs:331
- [ ] not seen · Tunnel row, "Connected · <n> hosts" · model/mod.rs:332
- [ ] not seen · Tunnel row, "Connected · its DNS doesn't answer" (red) · model/mod.rs:328-337
- [ ] not seen · Tunnel row, "Lost the gateway · trying again…" with the dashed head ·
  model/mod.rs:333, panel/tunnels.rs:244-256
- [ ] not seen · Tunnel row, trouble: the service's label or "Couldn't stay connected" (red), with
  Dismiss · model/mod.rs:317-319, panel/tunnels.rs:44-46, :217-237
- [ ] not seen · Tunnel row, managed: lock + "Managed" chip, policy sentence with a shield ·
  panel/tunnels.rs:47-80
- [ ] not seen · Tunnel row, managed full tunnel: block icon, sentence in ink ·
  panel/tunnels.rs:47-80
- [ ] not seen · Tunnel row, the user's own: person icon, "Your profile · <mode>" ·
  panel/tunnels.rs:81-92
- [ ] not seen · Notices: none (each returns nothing) · panel/mod.rs:208-211
- [ ] not seen · Notices: one · panel/mod.rs:208-211
- [ ] not seen · Notices: several at once, stacked questions, version, leftovers, staying ·
  panel/mod.rs:208-211
- [ ] not seen · Help card closed / open · panel/decisions.rs:72
- [ ] not seen · Failure sentence shown for 8 s after the service refused something ·
  panel/mod.rs:224-235, model/mod.rs:50
- [ ] not seen · Clash on switching on, said at once (the service's "Turn <other> off to use
  <tunnel>.") · requests.rs:127-139
- [ ] not seen · Footer button says "Copied" for 2 s · panel/mod.rs:598-600, :632-636
- [ ] not seen · Short screen: the middle scrolls, the footer stays · panel/mod.rs:195-198, :504-513
- [ ] not seen · Opened upward from a bottom bar · windows.rs:137-166, panel/mod.rs:238
- [ ] not seen · Opened downward from a top bar · windows.rs:137-166
- [ ] not seen · Not placeable (no click position): a normal window, centred · windows.rs:113, :164
- [ ] not seen · No tray: opens near the top · windows.rs:137-165

### Settings

- [ ] not seen · Tunnels empty: "No tunnels yet. Add the first one below." ·
  settings_view/tunnels.rs:28-34
- [ ] not seen · Tunnels, one: no "Drag to change the order." · settings_view/tunnels.rs:102-103
- [ ] not seen · Tunnels, many: numbered, grip, draggable · settings_view/tunnels.rs:41-58, :145-153
- [ ] not seen · Tunnels, a row dragged over another: tinted · settings_view/tunnels.rs:41-58
- [ ] not seen · Full tunnels in their own group "Everything else goes through:", each with its
  sentence · settings_view/tunnels.rs:61-71, :225-227
- [ ] not seen · Tunnel row with the Managed chip · settings_view/tunnels.rs:191-201
- [ ] not seen · Tunnel row with the Beta tag · settings_view/tunnels.rs:204
- [ ] not seen · Tunnel row's state in the panel's words · settings_view/tunnels.rs:207
- [ ] not seen · Tunnel row, off and clashing: the reason · settings_view/tunnels.rs:233
- [ ] not seen · Tunnel row added in the app: Edit, Remove… · settings_view/tunnels.rs:301-357
- [ ] not seen · Tunnel row from IT: "Set up by <org>" · settings_view/tunnels.rs:301-357
- [ ] not seen · Tunnel row from the config file: "From the config file" ·
  settings_view/tunnels.rs:301-357
- [ ] not seen · Remove asked: "Remove <name>?" + "What Crake learned for it and its saved sign-in
  go too." · settings_view/tunnels.rs:363-425
- [ ] not seen · General: "Switch on the tunnels you left on" dimmed (0.6) and inert while Start
  when you log in is off · settings_view/general.rs:75-87
- [ ] not seen · General: "Tell me when there's a new version" only where nothing else updates
  Crake · settings_view/general.rs:14, :99
- [ ] not seen · General, bird row with a tray: switch and "Without it, open Crake from <place>." ·
  settings_view/tray_row.rs:152
- [ ] not seen · General, bird row with no tray: reason line and remedy (button, Open Extensions or
  command chip) · settings_view/tray_row.rs:143-151, remedies no_tray.rs:54-83
- [ ] not seen · Learning, never list empty: only "+ Add" · settings_view/learning.rs:66-150
- [ ] not seen · Learning, never list with chips · settings_view/learning.rs:66-150
- [ ] not seen · Learning, never list adding: the input shown · settings_view/learning.rs:66-150
- [ ] not seen · Names, none: "None yet. Gateways push them when they connect." ·
  settings_view/names.rs:37
- [ ] not seen · Names, listed: "*.<domain> → <tunnel>" · settings_view/names.rs:28
- [ ] not seen · Names: "Not kept private right now. <why>" · settings_view/names.rs:81-89
- [ ] not seen · Privacy, clear log idle (quiet, red on hover) · settings_view/privacy.rs:28-78
- [ ] not seen · Privacy, clear log asked: "Keep it" / "Clear log" · settings_view/privacy.rs:30-58
- [ ] not seen · Managed, none: "Your organisation hasn't set up any tunnels here." ·
  settings_view/managed.rs:20
- [ ] not seen · Managed card: "Full tunnel required. Crake sends everything through and learns
  nothing." · settings_view/managed.rs:27-39
- [ ] not seen · Managed card: "Crake may learn anything that needs this tunnel." ·
  settings_view/managed.rs:27-39
- [ ] not seen · Managed card: "Learning allowed for" + list · settings_view/managed.rs:27-39
- [ ] not seen · Diagnostics, no report yet · settings_view/diagnostics.rs:165-240
- [ ] not seen · Diagnostics, saving: dashed head, "Saving the report…", then with seconds, "Crake
  is looking at this computer and trying the internet." · settings_view/diagnostics.rs:165-180,
  :310-318
- [ ] not seen · Diagnostics, saving: the button muted and inert · settings_view/diagnostics.rs:98-106
- [ ] not seen · Diagnostics, saved: "Saved <file> in <folder>[, names and addresses left out]." +
  Open, Show in folder · settings_view/diagnostics.rs:56-75, :220-236
- [ ] not seen · Diagnostics, failed: "Couldn't save the report." + reason, or "Crake couldn't ask
  its service." · settings_view/diagnostics.rs:180, :272
- [ ] not seen · Diagnostics sentence differs with hide-hosts · settings_view/diagnostics.rs:93-95
- [ ] not seen · Diagnostics terminal command per platform · settings_view/diagnostics.rs:79-86
- [ ] not seen · Any section, a select open (one at a time; Esc closes it first) ·
  settings_view/mod.rs:229-236
- [ ] not seen · Any section, a help card open · settings_view/mod.rs:467-526
- [ ] not seen · Failure sentence under the body for 8 s · settings_view/mod.rs:206-210, :318-321
- [ ] not seen · Service out of reach: no state of its own (see "Found on the way") ·
  settings_view/mod.rs:190
- [ ] not seen · Short screen: height capped, the main column scrolls · windows.rs:254
- [ ] not seen · macOS: no close button, empty drag strip kept, sidebar heading lower ·
  settings_view/mod.rs:258-271, :532-537

### Tunnel detail

- [ ] not seen · Tunnel removed meanwhile: an empty window · tunnel_detail/mod.rs:95-96
- [ ] not seen · Before the service answers: no loading state, the rows are empty until it does ·
  tunnel_detail/mod.rs:98-102
- [ ] not seen · Title: Connected · tunnel_detail/facts.rs:7-14
- [ ] not seen · Title: "Connected · its DNS doesn't answer" · tunnel_detail/facts.rs:7-14
- [ ] not seen · Title: "Connecting…" · tunnel_detail/facts.rs:7-14
- [ ] not seen · Title: trouble words · tunnel_detail/facts.rs:7-14
- [ ] not seen · Title: Off · tunnel_detail/facts.rs:7-14
- [ ] not seen · Why it stopped: the service's sentence, in trouble or off after Dismiss ·
  tunnel_detail/facts.rs:330-332
- [ ] not seen · Facts when off: "—" for Connected for and Your address ·
  tunnel_detail/facts.rs:79-84
- [ ] not seen · Facts, OpenVPN 3 tunnel: "Profile" with "Edit…" · tunnel_detail/facts.rs:149-161
- [ ] not seen · Facts, OpenVPN 3 tunnel, locked profile: "Locked" · tunnel_detail/facts.rs:149-161
- [ ] not seen · Policy card: managed full tunnel · tunnel_detail/facts.rs:232-260
- [ ] not seen · Policy card: own full tunnel · tunnel_detail/facts.rs:232-260
- [ ] not seen · Policy card: managed learn list ("You can pin and forget hosts, but not widen the
  rule.") · tunnel_detail/facts.rs:232-260, :246
- [ ] not seen · Policy card: own list ("Only these go through") · tunnel_detail/facts.rs:232-260
- [ ] not seen · Policy card: ask first · tunnel_detail/facts.rs:232-260
- [ ] not seen · Policy card: learn as it goes · tunnel_detail/facts.rs:232-260
- [ ] not seen · Chart: "Nothing through <tunnel>" · tunnel_detail/chart.rs:67
- [ ] not seen · Chart: "<bytes> through <tunnel>", "1 host" / "<n> hosts" ·
  tunnel_detail/chart.rs:20-75
- [ ] not seen · Tabs: Short names only once there are any · tunnel_detail/hosts.rs:1-3, :44-47
- [ ] not seen · Tab empty: "Nothing learned through <tunnel> yet." · tunnel_detail/hosts.rs:107-109
- [ ] not seen · Tab empty: "Nothing here." · tunnel_detail/hosts.rs:107-109
- [ ] not seen · Kept line per tab (a day, <n> days, until forgotten, pinned, gateway, short
  names) · tunnel_detail/hosts.rs:421-432
- [ ] not seen · Host row last used: "Used now" · tunnel_detail/hosts.rs:339-349
- [ ] not seen · Host row last used: "<n> min ago" · tunnel_detail/hosts.rs:339-349
- [ ] not seen · Host row last used: "<n> h ago" · tunnel_detail/hosts.rs:339-349
- [ ] not seen · Host row last used: "Yesterday" · tunnel_detail/hosts.rs:339-349
- [ ] not seen · Host row last used: "<n> days ago" · tunnel_detail/hosts.rs:339-349
- [ ] not seen · Host row pinned: "Pinned · kept until you forget it" · tunnel_detail/hosts.rs:339-349
- [ ] not seen · Host row's learned-how line (its variants) · tunnel_detail/hosts.rs:646-672
- [ ] not seen · Host shown from the log: tab chosen, row tinted, scrolled into view ·
  tunnel_detail/mod.rs:363-377, tunnel_detail/hosts.rs:232-250
- [ ] not seen · Many hosts: only the rows in view are built · tunnel_detail/hosts.rs:214-216,
  :406-419
- [ ] not seen · Gateway tab rows: "Set by the gateway", "<tunnel>'s DNS server", "Used now" / "—",
  "Pushed when <tunnel> connects" · tunnel_detail/hosts.rs:291, :685-692
- [ ] not seen · "Forget all…" only on Learned, when something was learned or undone ·
  tunnel_detail/hosts.rs:496-512
- [ ] not seen · Forget-all question open; its words vary with the learned, undone and pinned
  counts · tunnel_detail/hosts.rs:537-560
- [ ] not seen · Full tunnel: no "Add a host" · tunnel_detail/hosts.rs:157-160
- [ ] not seen · Add a host's placeholder, from the tunnel's domain and network ·
  tunnel_detail/hosts.rs:112-134
- [ ] not seen · Failure sentence under the form for 8 s · tunnel_detail/mod.rs:112-116,
  tunnel_detail/hosts.rs:203
- [ ] not seen · Refreshes every 5 s while open · tunnel_detail/mod.rs:42, :300-312
- [ ] not seen · Service out of reach: no state of its own · tunnel_detail/mod.rs:85
- [ ] not seen · macOS: header after the system's buttons, no × · tunnel_detail/mod.rs:138-152, :195

### New tunnel

- [ ] not seen · New: title "New tunnel", last button "Add and connect" · new_tunnel/view.rs:132-145
- [ ] not seen · Editing: title "Edit <name>", last button "Save" · new_tunnel/view.rs:132-145
- [ ] not seen · Detection idle: "Crake asks the gateway." · new_tunnel/server.rs:112-139
- [ ] not seen · Detection asking: dashed head, "Asking the gateway…", then with seconds ·
  new_tunnel/server.rs:198, :615-623
- [ ] not seen · Detection found AnyConnect, with the certificate sentence (expired / expires /
  valid until) · new_tunnel/server.rs:112-139, detect.rs:60-76
- [ ] not seen · Detection answered but unknown: "It answered, but not as AnyConnect. Pick its
  protocol below." · new_tunnel/server.rs:129
- [ ] not seen · Detection unreachable: "No answer from that gateway. Check the name, or pick the
  protocol below." · new_tunnel/server.rs:131-134
- [ ] not seen · Protocol list unfolded: "Used with real gateways" and "Beta: not yet tried with a
  real gateway" · new_tunnel/server.rs:268-358
- [ ] not seen · Protocol list folded: "Choose the protocol yourself", "<n> more" ·
  new_tunnel/server.rs:361-396
- [ ] not seen · Protocol list after an import: "From the profile you imported" ·
  new_tunnel/server.rs:268-358
- [ ] not seen · Protocol tag "Recommended" · new_tunnel/server.rs:442-457
- [ ] not seen · Protocol tag "Beta" · new_tunnel/server.rs:442-457, :588-613
- [ ] not seen · Protocol tag "Not installed" · new_tunnel/server.rs:442-457
- [ ] not seen · Protocol tag "Not available" (IPsec off Linux) · new_tunnel/server.rs:442-457
- [ ] not seen · Missing-engine sentence per protocol and platform · engines.rs:105-139
- [ ] not seen · Beta note for the chosen protocol · new_tunnel/server.rs:237-244
- [ ] not seen · OpenVPN, no profile: the Import action only · new_tunnel/server.rs:13-109
- [ ] not seen · OpenVPN, OpenVPN 3 profiles listed, locked ones marked · new_tunnel/server.rs:30-66
- [ ] not seen · OpenVPN, imported: "From <file>. Crake keeps its own copy…" ·
  new_tunnel/import.rs:89-128
- [ ] not seen · OpenVPN, import failed: the sentence · new_tunnel/import.rs:54-67, :140-145
- [ ] not seen · Group row, only when the gateway offered groups: "The gateway offers <n> groups.
  Ask your IT team if you're not sure." · new_tunnel/server.rs:537-578
- [ ] not seen · Sign-in method unavailable: "Not available" tag · new_tunnel/sign_in.rs:35-39,
  engines.rs:79-100
- [ ] not seen · Certificate from: "Smart card (PKCS#11)" · new_tunnel/sign_in.rs:152-209
- [ ] not seen · Certificate from: "System keyring" · new_tunnel/sign_in.rs:152-209
- [ ] not seen · Certificate from: "A file…", none chosen ("Choose a file…") ·
  new_tunnel/sign_in.rs:152-209
- [ ] not seen · Certificate from: "A file…", chosen ("Choose another…") ·
  new_tunnel/sign_in.rs:152-209
- [ ] not seen · Advanced folded · new_tunnel/sign_in.rs:73-124
- [ ] not seen · Advanced unfolded ("Report a different client version", "Allow weak ciphers") ·
  new_tunnel/sign_in.rs:73-124
- [ ] not seen · IPsec key: empty · new_tunnel/ipsec.rs:290-353
- [ ] not seen · IPsec key: typed, shown or hidden · new_tunnel/ipsec.rs:290-353
- [ ] not seen · IPsec key: "Kept by Crake's service" (Change…, "Keep the old key") ·
  new_tunnel/ipsec.rs:290-353
- [ ] not seen · IPsec authority file: none / chosen / "Kept by Crake's service" ·
  new_tunnel/ipsec_proof.rs:150-189
- [ ] not seen · IPsec certificate file: none / chosen / "Kept in your keyring" ·
  new_tunnel/ipsec_proof.rs:273-305
- [ ] not seen · Routing "Only what I list": list editor (chips, input, add) ·
  new_tunnel/routing.rs:40-117
- [ ] not seen · Routing: "The gateway suggests" box, when editing a tunnel whose gateway pushed
  networks · new_tunnel/routing.rs:147-160
- [ ] not seen · Step forward refused until the earlier steps are complete ·
  new_tunnel/view.rs:60-66
- [ ] not seen · Problem line: a validation sentence (list in section 6) · new_tunnel/steps.rs:62-96
- [ ] not seen · Service not running at the last step: stays open, "Crake's service isn't running,
  so this can't be saved yet. …" · new_tunnel/steps.rs:13, :172-180
- [ ] not seen · File errors: "Couldn't read that file: <why>", "“<x>” isn't a certificate…" ·
  new_tunnel/steps.rs:229, new_tunnel/ipsec_proof.rs:258-262
- [ ] not seen · A dropdown open (one at a time; Esc closes it first) · new_tunnel/view.rs:191-197
- [ ] not seen · macOS: "Don't add it" on the right, no × · new_tunnel/view.rs:74-97
- [ ] not seen · Linux only: IPsec, with the strongSwan install command per distribution ·
  new_tunnel/server.rs:219-224, engines.rs:174-195

### Sign-in

- [ ] not seen · Reaching · signin_view/mod.rs:41-71
- [ ] not seen · Signing in · signin_view/mod.rs:41-71
- [ ] not seen · Bringing the tunnel up · signin_view/mod.rs:41-71
- [ ] not seen · "Tunnel up", shown 0.9 s, then the window closes · signin_view/mod.rs:31-32, :380
- [ ] not seen · Browser: "Finish in the browser tab Crake opened. This window updates by itself.",
  "Waiting for you in <browser>", "Open the page again" · signin_view/mod.rs:153-175
- [ ] not seen · Asking: a choice · signin_view/question.rs:34-66
- [ ] not seen · Asking: certificate password, first time ("Unlock your certificate") ·
  signin_view/question.rs:67-86
- [ ] not seen · Asking: certificate password, wrong ("That password didn't open it. Type it
  again.") · signin_view/question.rs:67-86
- [ ] not seen · Asking: certificate password, the keyring's didn't open it ·
  signin_view/question.rs:67-86
- [ ] not seen · Asking: a secret (with the mask toggle) · signin_view/question.rs:90-136
- [ ] not seen · Asking: plain text · signin_view/question.rs:90-136
- [ ] not seen · Asked again after a refusal: "<X> didn't accept that username or password." ·
  signin_view/question.rs:98-100
- [ ] not seen · The gateway's own words quoted · signin_view/question.rs:40-42, :104-106
- [ ] not seen · Trust, unknown authority: "Its certificate isn't signed by an authority this
  computer knows. Trust it only if your IT says this is the right one." ·
  signin_view/question.rs:227-230
- [ ] not seen · Trust, changed: "It shows a different certificate from the one you trusted on
  <day>. Trust it only if your IT says it was renewed." · signin_view/question.rs:219-226
- [ ] not seen · Failed at a step: red cross on that row, later rows dropped ·
  signin_view/widgets.rs:82-91
- [ ] not seen · Failed, OpenVPN 3 has no such profile: "Couldn't start <tunnel>" ·
  signin_view/explain.rs:21
- [ ] not seen · Failed, OpenVPN 3 didn't start a session: "Couldn't start <tunnel>" ·
  signin_view/explain.rs:34
- [ ] not seen · Failed, OpenVPN 3 sign-in not accepted: "Couldn't sign in to <tunnel>" ·
  signin_view/explain.rs:42
- [ ] not seen · Failed, OpenVPN 3 otherwise: "<tunnel> didn't come up" · signin_view/explain.rs:51
- [ ] not seen · Failed: "openconnect isn't installed" · signin_view/explain.rs:70
- [ ] not seen · Failed: "Couldn't reach <tunnel>" · signin_view/explain.rs:93
- [ ] not seen · Failed: "<tunnel> didn't stay up" · signin_view/explain.rs:101
- [ ] not seen · Failed, the certificate didn't open: "Couldn't sign in to <tunnel>" ·
  signin_view/explain.rs:114
- [ ] not seen · Failed, sign-in not accepted: "Couldn't sign in to <tunnel>" ·
  signin_view/explain.rs:121
- [ ] not seen · Failed: "<tunnel> accepted the sign-in, but the tunnel didn't stay up." ·
  signin_view/mod.rs:399
- [ ] not seen · Failed, unreachable, checks under way ("Checking the internet", "Looking up",
  "Trying port <n>") · signin_view/checks.rs:82-106
- [ ] not seen · Failed, unreachable, checks done ("No internet", "Internet is up", "Name not
  found:", "Name found:", "Port <n> answers", "Refused on port <n>", "No answer on port <n>") ·
  signin_view/checks.rs:82-106
- [ ] not seen · Failed, unreachable, the sentence (7 variants) · signin_view/checks.rs:112-134
- [ ] not seen · Failed: "<program> said: <reason>" · signin_view/failed.rs:34
- [ ] not seen · Failed: "Other tunnels are unaffected. <X> is/are still connected." ·
  signin_view/failed.rs:84-109
- [ ] not seen · Failed with a key mismatch: "Change the key…" in place of "Try again" ·
  signin_view/failed.rs:65-76
- [ ] not seen · "Copied" beside Copy for helpdesk, 3 s · signin_view/failed.rs:52-53,
  signin_view/mod.rs:8
- [ ] not seen · Cancelled: closes without a word ("Cancelled." is internal) ·
  signin_view/mod.rs:496
- [ ] not seen · Long content scrolls inside the card, never past the screen ·
  signin_view/mod.rs:213-240, :259-278
- [ ] not seen · macOS: the room is worked out differently · signin_view/mod.rs:255-271

### Log

- [ ] not seen · Today empty: "Nothing to tell today. No host needed a decision." ·
  log_view/mod.rs:87-106
- [ ] not seen · Earlier days: one shown, the rest behind "Show earlier days" ·
  log_view/mod.rs:112-141
- [ ] not seen · Filtered (show, tunnel, search): Today still heads the list · log_view/mod.rs:87-93
- [ ] not seen · Retention note: "Not kept: only this session" · log_view/mod.rs:196-201
- [ ] not seen · Retention note: "Kept for a day…" · log_view/mod.rs:196-201
- [ ] not seen · Retention note: "Kept for <n> days on this machine" · log_view/mod.rs:196-201
- [ ] not seen · Row: routed (teal) · log_view/rows.rs:177-187
- [ ] not seen · Row: left alone, no badge · log_view/rows.rs:177-187, :240-242
- [ ] not seen · Row: undone · log_view/rows.rs:177-187
- [ ] not seen · Row: system, badge "System" · log_view/rows.rs:177-187, model/days.rs:9, :37
- [ ] not seen · Row: asked (amber while waiting, then muted or teal) · log_view/rows.rs:177-187
- [ ] not seen · Routed row pointed at or focused: tinted to the edges, the badge becomes "Show in
  <tunnel>" · log_view/rows.rs:62-116
- [ ] not seen · Question row: "Use <tunnel>'s" while answerable; two tunnels' chips ·
  log_view/rows.rs:151-172, :230-239
- [ ] not seen · Day titles: Today, Yesterday, "<Weekday> <d> <Month>" · model/days.rs:70-74
- [ ] not seen · Today summary: "<n> host(s) routed, <n> left alone, <n> undone" ·
  model/days.rs:91-113
- [ ] not seen · Day chart zoomed: "HH:MM–HH:MM · <n>-minute bars" + "Whole day" ·
  log_view/day_chart.rs:65-90
- [ ] not seen · Day chart pointed at: "<bytes> of traffic", "No traffic", "… in <n> minutes" ·
  log_view/day_chart.rs:535-541
- [ ] not seen · Day chart hint on hover: "Scroll to zoom · drag to move" ·
  log_view/day_chart.rs:291-300
- [ ] not seen · Header button says "Copied" for 2 s · log_view/mod.rs:301-313, :358-361
- [ ] not seen · Service out of reach: no state of its own · log_view/mod.rs:79

### Traffic

- [ ] not seen · Before the first counters: "Looking…" · traffic_view/mod.rs:446
- [ ] not seen · No connections: "No connections open." · traffic_view/mod.rs:447
- [ ] not seen · Connections can't be listed: "Crake couldn't list the connections. The ways out
  above are live." · traffic_view/mod.rs:443
- [ ] not seen · Graph over its limit: "and <n> more" · traffic_view/graph.rs:111-200
- [ ] not seen · Table over its limit: "and <n> more" · traffic_view/tables.rs:250
- [ ] not seen · "Where things go" reason: learned, through <tunnel> · traffic_view/tables.rs:87-150
- [ ] not seen · Reason: "Pinned" · traffic_view/tables.rs:87-150
- [ ] not seen · Reason: "You sent it direct with Undo" · traffic_view/tables.rs:87-150
- [ ] not seen · Reason: tunnel off or not connected ("direct for now") ·
  traffic_view/tables.rs:87-150
- [ ] not seen · Reason: "Never through a tunnel (Settings → Learning)" ·
  traffic_view/tables.rs:87-150
- [ ] not seen · Reason: "Everything else" (a full tunnel, or direct) · traffic_view/tables.rs:87-150
- [ ] not seen · Polls the service every second while open · traffic_view/mod.rs:84-104
- [ ] not seen · Motion off: dots stand still, thickness says the rate ·
  traffic_view/mod.rs:138, traffic_view/graph.rs:412-446
- [ ] not seen · Service out of reach: no state of its own · traffic_view/mod.rs:469

### Profile editor

- [ ] not seen · Loading: "Reading it from OpenVPN 3…" · profile_editor/mod.rs:493
- [ ] not seen · Locked: "<X>'s profile is locked in OpenVPN 3: its text can't be shown or changed
  here." · profile_editor/mod.rs:494-497
- [ ] not seen · Unreadable: "OpenVPN 3 has no profile called <x> any more." ·
  profile_editor/mod.rs:201-205
- [ ] not seen · Unreadable: "Couldn't read it from OpenVPN 3: <why>" · profile_editor/mod.rs:201-205
- [ ] not seen · Editing, unchanged: Save at lowered opacity, inert · profile_editor/mod.rs:413, :37
- [ ] not seen · Editing, changed: changed lines tinted, folded blocks muted, footer "Saving gives
  OpenVPN 3 the new text…" · profile_editor/mod.rs:211-262, :382
- [ ] not seen · Saving: "Giving OpenVPN 3 the new text…" · profile_editor/mod.rs:379
- [ ] not seen · Refused: "OpenVPN 3 refused it: “…”. Nothing was changed." (red) ·
  profile_editor/mod.rs:378, :402, ovpn3/profile.rs:143
- [ ] not seen · Refused: the old profile wasn't removed · ovpn3/profile.rs:165
- [ ] not seen · A folded line was edited: "The folded <tag> line was changed, so it can't be put
  back…" · profile_editor/text.rs:86
- [ ] not seen · Leaving with changes: "Leave without saving?" · profile_editor/mod.rs:333-374

### What's new

- [ ] not seen · Since a version (from the panel's notice): the header says so ·
  whats_new/view.rs:103-106, whats_new/mod.rs:103
- [ ] not seen · This version only (from About) · whats_new/view.rs:103-106
- [ ] not seen · Earlier versions folded · whats_new/view.rs:192-216
- [ ] not seen · Earlier versions shown · whats_new/view.rs:192-216
- [ ] not seen · Nothing to show: no branch for it; the body would be empty ·
  whats_new/view.rs:187-196

### Per platform

- [ ] not seen · Linux: through XWayland unless `CRAKE_WAYLAND`, so the panel opens at the icon ·
  main.rs:316-323
- [ ] not seen · Linux: the tray's `Activate(x, y)` gives the click; tray host online / offline ·
  sys/linux/tray.rs:57-78
- [ ] not seen · Linux: notifications with buttons · notify.rs:44, :92
- [ ] not seen · Linux: motion follows GNOME's `enable-animations` · motion.rs:15
- [ ] not seen · Windows: opaque windows, the system rounds and frames them (no card hairline, no
  rounding of Crake's) · chrome.rs:59-83, panel/mod.rs:180, settings_view/mod.rs:549
- [ ] not seen · Windows: tray in the notification area, drawn for a light or a dark bar ·
  sys/menu_tray.rs:49-72
- [ ] not seen · Windows: "the Start menu" in the bird row; the report command with its full path ·
  settings_view/tray_row.rs:25, settings_view/diagnostics.rs:81
- [ ] not seen · Windows: Download fetches this computer's installer · versions.rs:33-48
- [ ] not seen · macOS: the system's window buttons, no Crake ×; headers leave room for them ·
  chrome.rs:17-51
- [ ] not seen · macOS: menu bar icon as a template image; the panel placed at the bar's edge ·
  windows.rs:139, sys/menu_tray.rs:49-70
- [ ] not seen · macOS: Dock icon only while a window is open · windows/dock.rs:22-61
- [ ] not seen · macOS: Cmd+W closes a window, Cmd+Q quits · focus.rs:69-72, :77-89
- [ ] not seen · macOS: plain notifications without buttons; the short-name one shows its title
  only · notify.rs:270-331
- [ ] not seen · macOS: "Show the bird in the menu bar", "Applications" ·
  settings_view/tray_row.rs:19, :27

### Demo and development switches (each is a way to reach a state)

- [ ] not seen · `CRAKE_DEMO=1`: the design's sample, no service, nothing remembered, keyring
  untouched · main.rs:337-402, model/demo.rs:9
- [ ] not seen · `CRAKE_DEMO=empty` (first run) · main.rs:360
- [ ] not seen · `CRAKE_DEMO=log` · main.rs:356, model/demo.rs:406
- [ ] not seen · `CRAKE_DEMO=expired` · main.rs:375, model/demo.rs:198
- [ ] not seen · `CRAKE_DEMO=openvpn3` · main.rs:376, model/demo.rs:267
- [ ] not seen · `CRAKE_DEMO=leftover` · main.rs:377, model/demo.rs:243
- [ ] not seen · `CRAKE_DEMO=staying` · main.rs:379, model/demo.rs:255
- [ ] not seen · `CRAKE_DEMO=long` (names on two lines) · main.rs:381, model/demo.rs:213
- [ ] not seen · `CRAKE_DEMO=first|back|ask|short` (the panel's moments) · main.rs:383,
  model/moments.rs:79
- [ ] not seen · `CRAKE_DEMO=anyconnect` (the acts-as notice) · main.rs:338
- [ ] not seen · `CRAKE_DEMO=arrive` (a decision every 8 s) · main.rs:454
- [ ] not seen · `CRAKE_DEMO=busy` (a merged tick) · model/demo.rs:140
- [ ] not seen · `CRAKE_DEMO=whatsnew` · whats_new/mod.rs:136
- [ ] not seen · `CRAKE_DEMO=notray:…` · no_tray.rs:136
- [ ] not seen · Demo differences: switching on just connects; settings change locally; Undo writes
  its own entry; Dismiss just says Off; Traffic moves a sample · requests.rs:203-212, :289-295,
  :478-506, :527-535, :250-276
- [ ] not seen · `CRAKE_SHOW=panel`, `hover:<minutes>` · main.rs:449, panel/mod.rs:486
- [ ] not seen · `CRAKE_SHOW=detail:<tunnel>` · main.rs:413
- [ ] not seen · `CRAKE_SHOW=profile:<tunnel>` · main.rs:420
- [ ] not seen · `CRAKE_SHOW=settings[:section[:saved|:saving]]` · windows.rs:242-250,
  settings_view/diagnostics.rs:35
- [ ] not seen · `CRAKE_SHOW=log`, `log@a-b` · main.rs:432, :484
- [ ] not seen · `CRAKE_SHOW=whatsnew` · main.rs:436
- [ ] not seen · `CRAKE_SHOW=traffic` · main.rs:440
- [ ] not seen · `CRAKE_SHOW=newtunnel[:step[:protocol[:…]]]` · new_tunnel/mod.rs:376
- [ ] not seen · `CRAKE_SHOW=signin:<tunnel>` · main.rs:488
- [ ] not seen · `CRAKE_HINT=<setting>` (that help card open) · settings_view/mod.rs:430
- [ ] not seen · `CRAKE_APPEARANCE=light|dark` (demo only) · main.rs:346
- [ ] not seen · `CRAKE_MOTION=off|slow` · motion.rs:12, :28
- [ ] not seen · `CRAKE_TRAY=bird` (whole-bird icons) · tray/mod.rs:163
- [ ] not seen · `CRAKE_WAYLAND=1`, `CRAKE_DEBUG` · main.rs:316, :627

### Entities: none, one, many, at once

- [ ] not seen · Tunnels, none: first-run panel; Settings "No tunnels yet…"; no tunnel items in the
  tray menu; "No tunnel is on" · panel/first_run.rs:108, settings_view/tunnels.rs:34,
  sys/menu_tray.rs:83, model/mod.rs:551
- [ ] not seen · Tunnels, one: a single row, no drag hint · settings_view/tunnels.rs:102
- [ ] not seen · Tunnels, many: order and numbers; the panel scrolls; the tooltip names up to 3,
  then "<n> tunnels" · panel/mod.rs:195-198, model/mod.rs:47, :554
- [ ] not seen · Tunnels, several on at once: the tray shows connecting over connected, trouble
  over warning; a clash is refused; "Other tunnels are unaffected" · model/mod.rs:506-527,
  requests.rs:127-139, signin_view/failed.rs:104
- [ ] not seen · Hosts, none: "Nothing learned through <tunnel> yet." · tunnel_detail/hosts.rs:107
- [ ] not seen · Hosts, one: "Connected · 1 host", "1 host" · model/mod.rs:331,
  tunnel_detail/chart.rs:20
- [ ] not seen · Hosts, many: "Connected · <n> hosts"; rows built only in view; "Forget the <n>
  hosts…?" · model/mod.rs:332, tunnel_detail/hosts.rs:406, :560
- [ ] not seen · Hosts, several in one decision: "Sent <n> hosts through <tunnel>." / "Left <n>
  hosts alone." · model/mod.rs:257-267
- [ ] not seen · Log entries, none: the panel's "Nothing yet…", the log's "Nothing to tell today…" ·
  panel/decisions.rs:31, log_view/mod.rs:106
- [ ] not seen · Log entries, one · panel/decisions.rs:143
- [ ] not seen · Log entries, many: 3 in the panel, 500 kept in memory, by day in the log, earlier
  days folded · panel/mod.rs:69, model/mod.rs:39
- [ ] not seen · Log entries close together: chart marks within a minute and a half merge, with a
  count · model/mod.rs:207, :638-647, log_view/day_chart.rs:554
- [ ] not seen · Notices, none · panel/mod.rs:208-211
- [ ] not seen · Notices, one of a kind: one version notice at a time (Restart, then "<X> is out",
  then What's new) · panel/version.rs:18-39
- [ ] not seen · Notices, many: a question per waiting name, a leftover notice per process, a
  staying notice per tunnel · panel/question.rs:20-25, panel/leftover.rs:20-30,
  panel/staying.rs:17-27
- [ ] not seen · Notices at once: the tray and the header's bird show one mark, red over amber ·
  tray/mod.rs:72-96
- [ ] not seen · Protocols: 10 entries ("Detect automatically" + 9) · new_tunnel/mod.rs:62-104
- [ ] not seen · Protocols, none installed: tags and missing-engine sentences · engines.rs:105-139
- [ ] not seen · Short-name questions, one tunnel knows it · craked/src/manager/short_names.rs:159-171
- [ ] not seen · Short-name questions, several know it ("<A> and <B> know <short>."); room for 8
  choices a row · craked/src/manager/short_names.rs:159-171, panel/question.rs:11
- [ ] not seen · Browsers with their own DNS: one ("looks names up itself") or several ("look names
  up themselves") · panel/tunnels.rs:283-287
- [ ] not seen · Leftover processes: one or more, each its own notice and command ·
  panel/leftover.rs:40
- [ ] not seen · Releases in What's new: none hides the notice; one; many with earlier ones folded ·
  whats_new/mod.rs:58-78

## 3. Controls and their states

No control defines a pressed look (see "Found on the way"). Focus is one shared ring.

### Shared pieces, and every use

- [ ] not seen · Focus ring (`Keys::keys()`): 2 px accent ring, 2 px gap in the panel's colour,
  keyboard focus only; 104 call sites · focus.rs:44-51
- [ ] not seen · Tab / Shift+Tab move the focus · focus.rs:56-65
- [ ] not seen · Switch: off (track colour), on (accent), slides on a spring, pill-shaped focus
  ring; no hover, no pressed, no disabled look of its own · panel/mod.rs:363-399
- [ ] not seen · Switch use: a tunnel in the panel · panel/tunnels.rs:112
- [ ] not seen · Switch use: a tunnel in Settings · settings_view/tunnels.rs:240
- [ ] not seen · Switch use: Start when you log in · settings_view/general.rs:59
- [ ] not seen · Switch use: Switch on the tunnels you left on (dimmed and inert when the row is) ·
  settings_view/general.rs:79
- [ ] not seen · Switch use: Tell me when there's a new version · settings_view/general.rs:103
- [ ] not seen · Switch use: Keep internal names private · settings_view/names.rs:55
- [ ] not seen · Switch use: Ask every tunnel when a name is unknown · settings_view/names.rs:67
- [ ] not seen · Switch use: Hide names and addresses when sharing · settings_view/privacy.rs:141
- [ ] not seen · Switch use: Show the bird · settings_view/tray_row.rs:133
- [ ] not seen · Segmented control: chosen has accent fill, frame and ink; hand cursor; no hover,
  pressed or disabled · controls.rs:18-48
- [ ] not seen · Segmented use: the log's Show filter (4) · log_view/mod.rs:387
- [ ] not seen · Segmented use: Wait for a direct answer (1 / 2 / 3 / 5 s) ·
  settings_view/learning.rs:22
- [ ] not seen · Small action (`controls::action`): ink, 13 px semibold, underlined, hand; no
  hover or pressed; focus only where the caller adds it · controls.rs:53-67
- [ ] not seen · Action use: "Show in <tunnel>" · log_view/rows.rs:77
- [ ] not seen · Action use: "Use <tunnel>'s" in the log · log_view/rows.rs:161
- [ ] not seen · Action use: "Use <tunnel>'s" in the panel · panel/decisions.rs:272
- [ ] not seen · Action use: "Use <tunnel>" · panel/moments.rs:116
- [ ] not seen · Action use: "Edit…" · tunnel_detail/facts.rs:157
- [ ] not seen · Action use: "Change…" · new_tunnel/ipsec.rs:293
- [ ] not seen · Action use: "Show" / "Hide" · new_tunnel/ipsec.rs:300
- [ ] not seen · Action use: "Keep the old key" · new_tunnel/ipsec.rs:339
- [ ] not seen · Action use: "Import .ovpn…" beside the profile list · new_tunnel/server.rs:55
- [ ] not seen · Action use: "Import .ovpn…" alone · new_tunnel/server.rs:493
- [ ] not seen · Action use: the sign-in window's link wrapper · signin_view/widgets.rs:175
- [ ] not seen · "Copied" acknowledgement (muted check + word) · controls.rs:70-86
- [ ] not seen · Scroll area with its scrollbar: hidden at rest, in as the pointer comes over the
  area, held, then out · controls.rs:123-204
- [ ] not seen · Scroll area use: panel middle · panel/mod.rs:200
- [ ] not seen · Scroll area use: Settings' main column · settings_view/mod.rs:242
- [ ] not seen · Scroll area use: tunnel detail · tunnel_detail/mod.rs:223
- [ ] not seen · Scroll area use: New tunnel · new_tunnel/view.rs:202
- [ ] not seen · Scroll area use: sign-in card · signin_view/mod.rs:213
- [ ] not seen · Scroll area use: log · log_view/mod.rs:177
- [ ] not seen · Scroll area use: Traffic · traffic_view/mod.rs:529
- [ ] not seen · Scroll area use: What's new · whats_new/view.rs:243
- [ ] not seen · Scrollbar always shown, in an open dropdown (`over_shown`) · controls.rs:231-239,
  used new_tunnel/widgets.rs:85
- [ ] not seen · Window close button: surface circle with ink ×; hover red with on-danger ×; not
  drawn on macOS · chrome.rs:88-112
- [ ] not seen · Close use: Settings · settings_view/mod.rs:269
- [ ] not seen · Close use: tunnel detail · tunnel_detail/mod.rs:197
- [ ] not seen · Close use: New tunnel · new_tunnel/view.rs:94
- [ ] not seen · Close use: log · log_view/mod.rs:373
- [ ] not seen · Close use: Traffic · traffic_view/mod.rs:555
- [ ] not seen · Close use: profile editor · profile_editor/mod.rs:485
- [ ] not seen · Close use: What's new · whats_new/view.rs:112
- [ ] not seen · Dashed head "under way" (16 px, dashes going round) · head.rs:88-107
- [ ] not seen · Dashed head use: asking the gateway · new_tunnel/server.rs:198
- [ ] not seen · Dashed head use: a tunnel reconnecting · panel/tunnels.rs:251
- [ ] not seen · Dashed head use: saving the report · settings_view/diagnostics.rs:169
- [ ] not seen · Host name: mono, cut with "…"; hover underline; after a moment a card; click
  copies; "Copied" 3 s; keyboard reachable · full_name.rs:38-100
- [ ] not seen · Host name use: gateway or profile fact · tunnel_detail/facts.rs:171
- [ ] not seen · Host name use: host rows · tunnel_detail/hosts.rs:465
- [ ] not seen · Host name use: Traffic graph destinations · traffic_view/mod.rs:263
- [ ] not seen · Host name use: Traffic tables · traffic_view/tables.rs:217
- [ ] not seen · Amber warning icon use: leftover notice · panel/leftover.rs:59
- [ ] not seen · Amber warning icon use: question notice · panel/question.rs:99
- [ ] not seen · Amber warning icon use: staying notice · panel/staying.rs:51

### Panel

- [ ] not seen · Panel button: bordered at rest, hover surface fill, focus ring; no pressed,
  disabled or busy · panel/mod.rs:339-359
- [ ] not seen · Panel button use: Undo / Redo / "Send through <tunnel>" on a decision ·
  panel/decisions.rs:204
- [ ] not seen · Panel button use: Traffic · panel/mod.rs:614
- [ ] not seen · Panel button use: Full log · panel/mod.rs:622
- [ ] not seen · Panel button use: Copy for helpdesk (label becomes "Copied") · panel/mod.rs:630
- [ ] not seen · Primary button "Open Login Items" (accent fill, no hover) · panel/mod.rs:303
- [ ] not seen · Primary button "Add a tunnel…" · panel/first_run.rs:122
- [ ] not seen · Header settings button (hover surface fill) · panel/mod.rs:574
- [ ] not seen · Notice button with a border (hover border muted): version actions ·
  panel/version.rs:53-79
- [ ] not seen · Notice button "Got it" (first route) · panel/moments.rs:65-83
- [ ] not seen · Notice buttons "Got it" (acts-as, browser DNS) · panel/tunnels.rs:146, :187
- [ ] not seen · "Dismiss" on a tunnel in trouble · panel/tunnels.rs:217-237
- [ ] not seen · "Keep <tunnel>" under a route · panel/moments.rs:131
- [ ] not seen · Question notice's "Use <tunnel>'s" and "Not now" · panel/question.rs:48, :72
- [ ] not seen · Staying notice's "Import .ovpn…" and "Not now" · panel/staying.rs:70, :89
- [ ] not seen · No-tray note's "Not now" · panel/no_tray.rs:25
- [ ] not seen · Command chip with Copy (leftover): Copy → "Copied" 3 s · panel/leftover.rs:115-182
- [ ] not seen · "?" help mark in the panel · panel/decisions.rs:73
- [ ] not seen · Chart: pointer readout and hairline · timeline.rs:120-163, :240-287

### Settings

- [ ] not seen · Nav row: rest; hover; selected (panel colour, bold) · settings_view/mod.rs:576-606
- [ ] not seen · Row, row with content below, group, sub-line · settings_view/widgets.rs:6-72
- [ ] not seen · "?" help mark: muted at rest, accent while its card is open; opens on hover, a
  click keeps it · settings_view/mod.rs:454-529
- [ ] not seen · Select: 38 px with chevron; open with options below; option hover; chosen option
  accent-soft; focus ring on button and options · settings_view/widgets.rs:165-247
- [ ] not seen · Select use: When Crake routes a host · settings_view/general.rs:19
- [ ] not seen · Select use: Appearance · settings_view/general.rs:34
- [ ] not seen · Select use: When two tunnels answer · settings_view/learning.rs:36
- [ ] not seen · Select use: Forget hosts unused for · settings_view/learning.rs:50
- [ ] not seen · Select use: Keep the log for · settings_view/privacy.rs:14
- [ ] not seen · Outline button (38 px, optional icon, "armed" colour on hover for a destructive
  one) · settings_view/widgets.rs:80-115
- [ ] not seen · Outline use: "Save a report…" (muted, default cursor while saving) ·
  settings_view/diagnostics.rs:98
- [ ] not seen · Outline use: "Keep it" (clear log) · settings_view/privacy.rs:34
- [ ] not seen · Outline use: "Clear log" (confirm) · settings_view/privacy.rs:43
- [ ] not seen · Outline use: "Clear log…" · settings_view/privacy.rs:64
- [ ] not seen · Small button (32 px, optional icon, armed hover) · settings_view/widgets.rs:120-161
- [ ] not seen · Small button use: Edit · settings_view/tunnels.rs:309
- [ ] not seen · Small button use: Remove… · settings_view/tunnels.rs:329
- [ ] not seen · Small button use: Keep it · settings_view/tunnels.rs:395
- [ ] not seen · Small button use: Remove · settings_view/tunnels.rs:404
- [ ] not seen · Tunnel row: grab cursor when ordered; the name opens detail; chevron button ·
  settings_view/tunnels.rs:139-290
- [ ] not seen · Primary "+ New tunnel" (accent, focus ring, no hover) ·
  settings_view/tunnels.rs:82-97
- [ ] not seen · Never-list chip with × (hover surface) and "+ Add" · settings_view/learning.rs:92-145
- [ ] not seen · Never-list input · settings_view/learning.rs:127
- [ ] not seen · Link use: "Source on GitHub" · settings_view/about.rs:87
- [ ] not seen · Link use: "dorhawk.software" · settings_view/about.rs:89
- [ ] not seen · Link use: report "Open" · settings_view/diagnostics.rs:226
- [ ] not seen · Link use: report "Show in folder" · settings_view/diagnostics.rs:232
- [ ] not seen · Coffee button (outline, hover border muted) · settings_view/about.rs:173-189
- [ ] not seen · No-tray remedy button use: "Switch on AppIndicator" · no_tray_view.rs:37
- [ ] not seen · No-tray remedy button use: "Open Extensions" · no_tray_view.rs:47
- [ ] not seen · No-tray command chip: Copy → check + "Copied" 3 s · no_tray_view.rs:90-153

### New tunnel (none of these takes the keyboard's focus ring; see "Found on the way")

- [ ] not seen · Step tab: current (bold, underlined), done (dot with ✓), ahead (muted) ·
  new_tunnel/view.rs:14-70
- [ ] not seen · Header "Don't add it" (hover surface) · new_tunnel/view.rs:79-92
- [ ] not seen · Footer Back · new_tunnel/view.rs:242-250
- [ ] not seen · Footer Continue / Add and connect / Save · new_tunnel/view.rs:270-287
- [ ] not seen · Text field (42 px, optional mono; the focus look is the toolkit's) ·
  new_tunnel/widgets.rs:121-134
- [ ] not seen · Field use: Name · new_tunnel/server.rs:154
- [ ] not seen · Field use: Gateway (two layouts) · new_tunnel/server.rs:170, :174
- [ ] not seen · Field use: Username · new_tunnel/sign_in.rs:139
- [ ] not seen · Field use: Gateway's key · new_tunnel/ipsec.rs:324
- [ ] not seen · Field use: Company domains · new_tunnel/ipsec.rs:399
- [ ] not seen · Field use: IPsec username · new_tunnel/ipsec.rs:464
- [ ] not seen · Field use: Gateway's identity · new_tunnel/ipsec.rs:531
- [ ] not seen · List input ("Send these through") · new_tunnel/routing.rs:82
- [ ] not seen · Dropdown (42 px; the open list scrolls past 240 px; option hover) ·
  new_tunnel/widgets.rs:8-100
- [ ] not seen · Dropdown use: OpenVPN 3 profile · new_tunnel/server.rs:61
- [ ] not seen · Dropdown use: Group · new_tunnel/server.rs:553
- [ ] not seen · Dropdown use: Certificate from · new_tunnel/sign_in.rs:153
- [ ] not seen · Dropdown use: Forget hosts unused for · new_tunnel/routing.rs:181
- [ ] not seen · Radio: off (strong line), on (accent with dot) · new_tunnel/widgets.rs:136-149
- [ ] not seen · Radio use: protocol card · new_tunnel/server.rs:425
- [ ] not seen · Radio use: choice card · new_tunnel/widgets.rs:177
- [ ] not seen · Choice card: off 1 px edge, on 2 px accent edge, optional tag ·
  new_tunnel/widgets.rs:152-203
- [ ] not seen · Card use: sign-in methods · new_tunnel/sign_in.rs:37
- [ ] not seen · Card use: routing modes · new_tunnel/routing.rs:15
- [ ] not seen · Card use: "A key" · new_tunnel/ipsec_proof.rs:86
- [ ] not seen · Card use: "Its certificate" · new_tunnel/ipsec_proof.rs:103
- [ ] not seen · Card use: IPsec sign-in methods · new_tunnel/ipsec.rs:435
- [ ] not seen · Protocol card: chosen or not, with tags · new_tunnel/server.rs:400-484
- [ ] not seen · Checkbox (New tunnel's own): off outlined 1.5 px, on accent with a 12 px check ·
  new_tunnel/widgets.rs:206-241
- [ ] not seen · Checkbox use: "Report a different client version" · new_tunnel/sign_in.rs:97
- [ ] not seen · Checkbox use: "Allow weak ciphers" · new_tunnel/sign_in.rs:110
- [ ] not seen · Checkbox use: "Stay signed in for 12 hours, if the gateway allows it" ·
  new_tunnel/sign_in.rs:215
- [ ] not seen · Checkbox use: "Send these through from the start, without waiting" ·
  new_tunnel/routing.rs:151
- [ ] not seen · "Choose a file…" / "Choose another…" buttons · new_tunnel/sign_in.rs:191,
  new_tunnel/ipsec_proof.rs:150-152, :296-298
- [ ] not seen · strongSwan command chip with Copy → "Copied" · new_tunnel/ipsec.rs:228-288

### Sign-in

- [ ] not seen · Button, Primary look (accent, no hover) · signin_view/widgets.rs:122-166
- [ ] not seen · Button, Outline look (hover surface) · signin_view/widgets.rs:122-166
- [ ] not seen · Button, Text look (hover surface) · signin_view/widgets.rs:122-166
- [ ] not seen · Button use: "Open the page again" · signin_view/mod.rs:169
- [ ] not seen · Button use: "Don't connect" (browser) · signin_view/mod.rs:176
- [ ] not seen · Button use: "Don't connect" (progress) · signin_view/mod.rs:188
- [ ] not seen · Button use: a choice · signin_view/question.rs:50
- [ ] not seen · Button use: "Don't connect" (choice) · signin_view/question.rs:60
- [ ] not seen · Button use: "Don't connect" (answer) · signin_view/question.rs:194
- [ ] not seen · Button use: "Continue" · signin_view/question.rs:200
- [ ] not seen · Button use: "Don't connect" (trust; the keyboard starts here) ·
  signin_view/question.rs:255
- [ ] not seen · Button use: "Trust it" · signin_view/question.rs:262
- [ ] not seen · Button use: "Close" · signin_view/failed.rs:60
- [ ] not seen · Button use: "Change the key…" · signin_view/failed.rs:69
- [ ] not seen · Button use: "Try again" · signin_view/failed.rs:74
- [ ] not seen · Link use: "Copy for helpdesk" · signin_view/failed.rs:47
- [ ] not seen · Checkbox (sign-in's own): off 1 px outline, on accent with a 14 px check ·
  signin_view/widgets.rs:183-216
- [ ] not seen · Checkbox use: "Remember the password in your keyring" ·
  signin_view/question.rs:145
- [ ] not seen · Checkbox use: "Keep a copy of the certificate in your keyring" ·
  signin_view/question.rs:169
- [ ] not seen · Answer field: 1 px line at rest, 2 px accent while focused; eye toggle for
  secrets · signin_view/question.rs:126-135
- [ ] not seen · Step row mark: done (check) · signin_view/widgets.rs:82-109
- [ ] not seen · Step row mark: under way (dots, bold) · signin_view/widgets.rs:82-109
- [ ] not seen · Step row mark: ahead (circle, muted) · signin_view/widgets.rs:82-109
- [ ] not seen · Step row mark: failed (cross, red, bold) · signin_view/widgets.rs:82-109

### Tunnel detail, log, Traffic, profile editor, What's new

- [ ] not seen · Detail header button · tunnel_detail/mod.rs:258-272
- [ ] not seen · Header button use: "Tunnels" (back) · tunnel_detail/mod.rs:160
- [ ] not seen · Header button use: Switch on / Switch off · tunnel_detail/mod.rs:180
- [ ] not seen · Detail tab: selected (ink, bold, accent underline), others muted, with a count ·
  tunnel_detail/hosts.rs:56-93
- [ ] not seen · Detail row button: quiet at rest, hover surface · tunnel_detail/hosts.rs:624-644
- [ ] not seen · Row button use: Pin / Unpin · tunnel_detail/hosts.rs:367
- [ ] not seen · Row button use: Forget (turns red on hover) · tunnel_detail/hosts.rs:381
- [ ] not seen · Row button use: "Forget all…" · tunnel_detail/hosts.rs:515
- [ ] not seen · Row button use: Forget (a short name) · tunnel_detail/short_names.rs:41
- [ ] not seen · "Add a host" field and "Send through <tunnel>" button ·
  tunnel_detail/hosts.rs:180-200
- [ ] not seen · Log outline button (hover surface) · log_view/mod.rs:204-228
- [ ] not seen · Log outline use: "Whole day" · log_view/day_chart.rs:81
- [ ] not seen · Log outline use: "Show earlier days" · log_view/mod.rs:133
- [ ] not seen · Log outline use: "Copy for helpdesk" · log_view/mod.rs:356
- [ ] not seen · Log tunnel picker and its options (option hover, chosen accent-soft) ·
  log_view/mod.rs:408-471
- [ ] not seen · Log search field ("Host, address or tunnel") · log_view/mod.rs:498-505
- [ ] not seen · Log routed row: rest; hover or focus tinted · log_view/rows.rs:97-116
- [ ] not seen · Day chart: zoom by scroll, drag to move, double-click, pointer readout ·
  log_view/day_chart.rs:205-270
- [ ] not seen · Traffic tab: selected (ink, accent underline) or not, with a count ·
  traffic_view/mod.rs:381-404
- [ ] not seen · Profile editor button: primary or plain; primary unavailable at lowered opacity
  with the default cursor · profile_editor/mod.rs:421-446, :413
- [ ] not seen · Profile editor button use: "Keep editing" · profile_editor/mod.rs:360
- [ ] not seen · Profile editor button use: "Discard changes" · profile_editor/mod.rs:368
- [ ] not seen · Profile editor button use: "Cancel" · profile_editor/mod.rs:406
- [ ] not seen · Profile editor button use: "Save and reconnect" · profile_editor/mod.rs:412
- [ ] not seen · Profile editor's code editor with line numbers (the toolkit's) ·
  profile_editor/mod.rs:129-133, :508
- [ ] not seen · What's new: the arrow keys, Page Up and Down, Home and End scroll ·
  whats_new/view.rs:54

### States a control can be in, across the app

- [ ] not seen · Busy: "Asking the gateway…" · new_tunnel/server.rs:617
- [ ] not seen · Busy: "Saving the report…" · settings_view/diagnostics.rs:312
- [ ] not seen · Busy: "Giving OpenVPN 3 the new text…" · profile_editor/mod.rs:379
- [ ] not seen · Busy: a sign-in step's dots · signin_view/widgets.rs:96
- [ ] not seen · Disabled: the reconnect switch, dimmed 0.6 · settings_view/general.rs:75
- [ ] not seen · Disabled: "Save a report…" muted while saving · settings_view/diagnostics.rs:102-106
- [ ] not seen · Disabled: "Save and reconnect" at lowered opacity · profile_editor/mod.rs:413
- [ ] not seen · Pressed: no control has one · (grep in section 8)

## 4. Transient surfaces

### Questions asked in place (no modal dialog exists): 4 confirmations and 1 trust question

- [ ] not seen · "Remove <tunnel>?" + "What Crake learned for it and its saved sign-in go too." →
  "Keep it" / "Remove" · settings_view/tunnels.rs:363-425
- [ ] not seen · "Clear log…" → "Keep it" / "Clear log" · settings_view/privacy.rs:28-62
- [ ] not seen · "Forget the <n> hosts Crake learned for <tunnel>?" → "Cancel" / "Forget all" ·
  tunnel_detail/hosts.rs:537-617
- [ ] not seen · "Leave without saving?" → "Keep editing" / "Discard changes" ·
  profile_editor/mod.rs:333-374
- [ ] not seen · "Crake can't vouch for <tunnel>'s gateway" → "Don't connect" / "Trust it" (asked
  by the app during sign-in, not a confirmation of the user's own act) ·
  signin_view/question.rs:211-268

### Menus: 10 in windows (3 implementations), 1 tray menu in 2 backends

- [ ] not seen · "When Crake routes a host": Say nothing / Quiet notification / Notification and a
  “crex crex” · settings_view/general.rs:19
- [ ] not seen · "Appearance": Follow the system / Light / Dark · settings_view/general.rs:34
- [ ] not seen · "When two tunnels answer": Ask me / Use the faster one / Use the first in order ·
  settings_view/learning.rs:36
- [ ] not seen · "Forget hosts unused for": 1 day / 7 days / 30 days / Never ·
  settings_view/learning.rs:50
- [ ] not seen · "Keep the log for": 1 day / 7 days / 30 days / Don't keep a log ·
  settings_view/privacy.rs:14
- [ ] not seen · New tunnel: OpenVPN 3 profile · new_tunnel/server.rs:61
- [ ] not seen · New tunnel: Group · new_tunnel/server.rs:553
- [ ] not seen · New tunnel: Certificate from (Smart card (PKCS#11) / System keyring / A file…) ·
  new_tunnel/sign_in.rs:153
- [ ] not seen · New tunnel: Forget hosts unused for · new_tunnel/routing.rs:181
- [ ] not seen · Log tunnel picker ("All tunnels" + the tunnels) · log_view/mod.rs:408-471
- [ ] not seen · Tray menu, Linux: summary (disabled), a check item per tunnel, Open Crake, New
  tunnel…, Settings…, Quit · sys/linux/tray.rs:77-146
- [ ] not seen · Tray menu, Windows and macOS: the same items · sys/menu_tray.rs:83-106

### Notices in the panel: 11 (9 with a button that puts them away)

- [ ] not seen · No tray: "Crake has no icon in the tray." + reason + remedy + "Not now" ·
  panel/no_tray.rs:10-73, no_tray.rs:45, :86-97
- [ ] not seen · Short-name question (amber): message, reason, "Use <tunnel>'s" per tunnel, "Not
  now" · panel/question.rs:29-117
- [ ] not seen · "Crake was updated to <v>. Restart it to finish." [Restart] (no way to put it
  away) · panel/version.rs:28-31
- [ ] not seen · "Crake <v> is out. You have <this>." [Download] [Not now] · panel/version.rs:32-38
- [ ] not seen · "Crake is now <v>: <n> change(s) since <s>." / "Crake is now <v>." [What's new]
  [Not now] · panel/version.rs:21-27, whats_new/mod.rs:58-78
- [ ] not seen · Leftover OpenVPN 3 process (amber): "OpenVPN 3 left a connection running." +
  sentence + command + "Got it" + "This note goes away by itself once the process ends." ·
  panel/leftover.rs:33-110, crake-proto/src/lib.rs:238
- [ ] not seen · Stays on OpenVPN 3 (amber), locked: title and why, "Import .ovpn…", "Not now" ·
  panel/staying.rs:31-103, openvpn/mod.rs:332-333, :453-456
- [ ] not seen · Stays on OpenVPN 3 (amber), refused: "<tunnel> stays on OpenVPN 3" + "<said>
  Import a .ovpn file Crake can run to move it." · panel/staying.rs:31-103, openvpn/mod.rs:457-460
- [ ] not seen · "Crake's first route." + sentence + "Got it" · panel/moments.rs:13-87
- [ ] not seen · "<X> answers for it too." with "Use <X>" / "Keep <Y>" (answered, not put away) ·
  panel/moments.rs:91-143
- [ ] not seen · "Crake introduces itself to <gateway> as Cisco AnyConnect, because <gateway>'s
  gateway only lets AnyConnect in." + "Got it" · panel/tunnels.rs:122-163
- [ ] not seen · Browser DNS over HTTPS + "Got it" · panel/tunnels.rs:167-208, text :267-293

### Notice lines elsewhere (the wider count of 27 adds these 16 to the 11 above)

- [ ] not seen · Panel: service-not-there block (its 5 general and 5 Windows variants are in
  section 2) · panel/mod.rs:254, service.rs:114-141
- [ ] not seen · Panel: failure sentence, 8 s · panel/mod.rs:224-235
- [ ] not seen · Settings: failure sentence, 8 s · settings_view/mod.rs:206-210, :321
- [ ] not seen · Tunnel detail: failure sentence, 8 s · tunnel_detail/mod.rs:112-116,
  tunnel_detail/hosts.rs:203
- [ ] not seen · Settings, Privacy: statement card "Everything Crake decides stays on this
  machine." · settings_view/privacy.rs:107
- [ ] not seen · Settings, Names: "Not kept private right now. <why>" · settings_view/names.rs:81-89
- [ ] not seen · Settings, General: the no-tray reason and remedy under the bird row ·
  settings_view/tray_row.rs:143
- [ ] not seen · Settings, Diagnostics: the report's status box · settings_view/diagnostics.rs:165
- [ ] not seen · Settings, Managed: footer "You can switch managed tunnels on and off, but not
  change them…" · settings_view/managed.rs:98
- [ ] not seen · Settings, Tunnels: the clash line on a row · settings_view/tunnels.rs:233
- [ ] not seen · New tunnel: problem line · new_tunnel/view.rs:215
- [ ] not seen · New tunnel: detection line and certificate sentence · new_tunnel/server.rs:113-135
- [ ] not seen · New tunnel: Beta note · new_tunnel/server.rs:237-244
- [ ] not seen · New tunnel: import line under the profile · new_tunnel/server.rs:77
- [ ] not seen · Tunnel detail: the why sentence · tunnel_detail/facts.rs:330
- [ ] not seen · Traffic: the note where a table would be · traffic_view/mod.rs:461

### "Copied" acknowledgements: 7 (there is no toast component)

- [ ] not seen · Panel footer button, 2 s · panel/mod.rs:632-636
- [ ] not seen · Log header button, 2 s · log_view/mod.rs:358-361
- [ ] not seen · Sign-in failed, beside Copy for helpdesk, 3 s · signin_view/failed.rs:52-53
- [ ] not seen · Host name after a click, 3 s · full_name.rs:98
- [ ] not seen · Leftover command chip, 3 s · panel/leftover.rs:167
- [ ] not seen · No-tray command chip, 3 s · no_tray_view.rs:138
- [ ] not seen · strongSwan install command · new_tunnel/ipsec.rs:273

### Tooltips and hover surfaces: 1 tooltip at 4 sites, 2 tray tooltips, 3 chart readouts, 1 hint

- [ ] not seen · Full-name card ("Click to copy", the whole name when cut): gateway or profile
  fact · tunnel_detail/facts.rs:171 (card full_name.rs:71, :125-160)
- [ ] not seen · Full-name card: tunnel detail host rows · tunnel_detail/hosts.rs:465
- [ ] not seen · Full-name card: Traffic graph destinations · traffic_view/mod.rs:263
- [ ] not seen · Full-name card: Traffic tables · traffic_view/tables.rs:217
- [ ] not seen · Tray tooltip, Linux: title "Crake", description the summary ·
  sys/linux/tray.rs:50-56
- [ ] not seen · Tray tooltip, Windows and macOS: the summary · sys/menu_tray.rs:72, :143
- [ ] not seen · Chart readout: the panel's "Last hour" · timeline.rs:120-163, :240-287
- [ ] not seen · Chart readout: tunnel detail's "Today through <tunnel>" · tunnel_detail/chart.rs:9
- [ ] not seen · Chart readout: the log's day chart · log_view/day_chart.rs:500-541
- [ ] not seen · Hover hint (a line, not a tooltip): "Scroll to zoom · drag to move" ·
  log_view/day_chart.rs:291-300

### Help cards: 11 shown (2 implementations), 1 text with no place to show it

- [ ] not seen · Panel "What Crake did" · panel/decisions.rs:71-141, text model/kept.rs:18-69
- [ ] not seen · "Show the bird in the tray" / "…in the menu bar", with the legend of 6 looks ·
  settings_view/tray_row.rs:33-52, used :145, :152
- [ ] not seen · "When Crake routes a host" · settings_view/mod.rs:347, used
  settings_view/general.rs:93
- [ ] not seen · "Wait for a direct answer" · settings_view/mod.rs:350, used
  settings_view/learning.rs:162
- [ ] not seen · "When two tunnels answer" · settings_view/mod.rs:356, used
  settings_view/learning.rs:169
- [ ] not seen · "Forget hosts unused for" · settings_view/mod.rs:359, used
  settings_view/learning.rs:170
- [ ] not seen · "Ask a tunnel's DNS for these names" · settings_view/mod.rs:362, used
  settings_view/names.rs:48
- [ ] not seen · "Keep internal names private" · settings_view/mod.rs:365, used
  settings_view/names.rs:53
- [ ] not seen · "Ask every tunnel when a name is unknown" · settings_view/mod.rs:368, used
  settings_view/names.rs:65
- [ ] not seen · "Keep the log for" · settings_view/mod.rs:371, used settings_view/privacy.rs:128
- [ ] not seen · "Hide names and addresses when sharing" · settings_view/mod.rs:374, used
  settings_view/privacy.rs:139
- [ ] not seen · Titles drawn without a card (6): Start when you log in, Switch on the tunnels you
  left on, Tell me when there's a new version, Appearance, Never send through a tunnel, Clear the
  log · settings_view/general.rs:57, :77, :101, :115, settings_view/learning.rs:177,
  settings_view/privacy.rs:147

### System notifications: 8 kinds, 10 places that send one, the call played at 2

- [ ] not seen · Routed: the log's message and reason, [Undo] [Open Crake], 8 s (GNOME keeps its
  own 4 s) · notify.rs:97-119, sent updates.rs:68
- [ ] not seen · Tunnel stopped by itself: [Switch on] [Open Crake], stays · notify.rs:208-226,
  sent updates.rs:75
- [ ] not seen · Sign-in ran out, not just switched on: the same notification · sent updates.rs:82
- [ ] not seen · OpenVPN 3 left a process running: no buttons, stays · notify.rs:231-236, sent
  updates.rs:88
- [ ] not seen · Short name: [Use <tunnel>'s]… [Not now], 20 s · notify.rs:176-203, sent
  updates.rs:101
- [ ] not seen · Ask me first: [Send through <tunnel>] [Leave it], 20 s · notify.rs:155-170, sent
  updates.rs:104
- [ ] not seen · Another tunnel answered too: [Use <X>]… [Keep it], 15 s · notify.rs:124-151, sent
  updates.rs:123
- [ ] not seen · No tray at login: "Crake is running without an icon" + line, [Switch on
  AppIndicator] when possible, [Open Crake] · notify.rs:241-268, sent tray_host.rs:139
- [ ] not seen · Report saved after Settings was closed: "Saved …" · notify.rs:52-61, sent
  settings_view/diagnostics.rs:290
- [ ] not seen · Report failed after Settings was closed: "Couldn't save the report." + reason ·
  sent settings_view/diagnostics.rs:292
- [ ] not seen · The call "crex crex" (a sound) with a routed notification · notify.rs:336-362,
  played updates.rs:70
- [ ] not seen · The call with a question · played updates.rs:111
- [ ] not seen · macOS: every one plain, no buttons · notify.rs:270-331
- [ ] not seen · "Say nothing": most are held back (two aren't; see "Found on the way") ·
  updates.rs:66-88

### Tray icon states: 10 shapes

- [ ] not seen · Off (asleep) · tray/mod.rs:26
- [ ] not seen · Connecting (dashed, looping) · tray/mod.rs:27, loop tray/motion.rs:140
- [ ] not seen · Listening (awake) · tray/mod.rs:28
- [ ] not seen · Routed (two teal marks; the shape for 4 s, the tray refreshed after 5 s) ·
  tray/mod.rs:29, model/mod.rs:36, updates.rs:11
- [ ] not seen · Trouble (red badge, awake) · tray/mod.rs:31
- [ ] not seen · Off with trouble · tray/mod.rs:33
- [ ] not seen · Connecting with trouble · tray/mod.rs:35
- [ ] not seen · Warning (amber triangle, awake) · tray/mod.rs:38
- [ ] not seen · Off with a warning · tray/mod.rs:40
- [ ] not seen · Connecting with a warning · tray/mod.rs:42
- [ ] not seen · When each applies · model/mod.rs:492-524
- [ ] not seen · Drawn white for a dark bar, dark for a light bar (Windows, macOS light), template
  image on macOS; 9 sizes from 16 to 48, and 64 · sys/menu_tray.rs:49-70, tray/mod.rs:153-157
- [ ] not seen · `CRAKE_TRAY=bird`: five whole-bird drawings · tray/mod.rs:163-196
- [ ] not seen · The same marks on the panel header's bird · panel/mod.rs:147
- [ ] not seen · The legend of six looks in Settings · settings_view/tray_row.rs:42-52

### App icon

- [ ] not seen · Drawn from the bird on a tile; the asset files written from it ·
  tray/family.rs:69-79, embedded assets.rs:7
- [ ] not seen · Windows executable icon · crake/build.rs:5, assets/icons/app/crake.ico
- [ ] not seen · Notification icon and Linux desktop entry id `software.dorhawk.crake` ·
  notify.rs:21
- [ ] not seen · macOS bundle id `software.dorhawk.Crake` · notify.rs:25
- [ ] not seen · macOS Dock icon, only while a window is open · windows/dock.rs:22-61

## 5. System-drawn surfaces

- [ ] not seen · Launch: no splash; the tray icon appears, the panel opens from it · main.rs:411
- [ ] not seen · Launch by hand with no tray: the panel opens itself with its note ·
  tray_host.rs:116-126
- [ ] not seen · Launch at login: Linux autostart entry · sys/linux/autostart.rs:13-37
- [ ] not seen · Launch at login: Windows Run key · sys/windows/autostart.rs:10-27
- [ ] not seen · Launch at login: macOS login agent · sys/macos/autostart.rs:20-49
- [ ] not seen · Launch failure: nothing of the app's own (see "Found on the way") ·
  main.rs:403-405
- [ ] not seen · Window frame, Linux: Crake draws its card and × · chrome.rs:59-112
- [ ] not seen · Window frame, Windows: the system rounds and outlines · chrome.rs:59-83,
  sys/windows/host.rs:81
- [ ] not seen · Window frame, macOS: the system's three buttons · chrome.rs:17-51
- [ ] not seen · Authorisation, Linux: the service asks polkit without interaction, so no prompt
  is drawn; a refusal comes back as "not authorized to use Crake's service" in a failure
  sentence; the app says nothing before it · craked/src/dbus/guard.rs:107-113,
  packaging/software.dorhawk.crake.policy:8
- [ ] not seen · Authorisation, macOS: System Settings → Login Items & Extensions, after "Crake's
  service needs your OK." and "Open Login Items" · panel/mod.rs:255-265, :318,
  sys/macos/service.rs:98
- [ ] not seen · Authorisation, Windows and macOS refusals: "only the signed-in user can use
  Crake's service", "only the user at the console can use Crake's service" ·
  craked/src/dbus/guard.rs:126, :141
- [ ] not seen · File picker, open: import .ovpn ("Import") · new_tunnel/import.rs:33-38
- [ ] not seen · File picker, open: certificate file ("Choose") · new_tunnel/steps.rs:205-210
- [ ] not seen · File picker, open: IT's certificate authority ("Choose") ·
  new_tunnel/ipsec_proof.rs:219-224
- [ ] not seen · File picker, save: the report, in Documents by default, named with the date ·
  settings_view/diagnostics.rs:249-252, report.rs:150-162
- [ ] not seen · Open the report with the system · settings_view/diagnostics.rs:228
- [ ] not seen · Show the report in its folder · settings_view/diagnostics.rs:234
- [ ] not seen · Browser sign-in handoff, openconnect's external browser · signin/mod.rs:188-215,
  engines.rs:79
- [ ] not seen · Browser sign-in handoff, OpenVPN: the page opened once, "Open the page again" ·
  openvpn/mod.rs:253-257, signin_view/mod.rs:169-171
- [ ] not seen · Browser sign-in handoff, OpenVPN 3 · ovpn3/mod.rs:570-574
- [ ] not seen · The browser's name in "Waiting for you in <browser>" ("your browser" as the
  fallback) · sys/linux/host.rs:235, sys/windows/host.rs:54, sys/macos/host.rs:78
- [ ] not seen · Link opened in the browser: the download page · requests.rs:382,
  panel/version.rs:35
- [ ] not seen · Links opened in the browser: source, dorhawk.software · settings_view/about.rs:34,
  :87-89
- [ ] not seen · Link opened in the browser: coffee · settings_view/about.rs:189
- [ ] not seen · Link opened in the browser: "Crex crex" · settings_view/mod.rs:308
- [ ] not seen · Keyring, Linux (Secret Service) · sys/linux/keyring.rs:13-42
- [ ] not seen · Keyring, macOS (Keychain, service "Crake") · sys/macos/keyring.rs:11-28
- [ ] not seen · Keyring, Windows (Credential Manager) · sys/windows/keyring.rs:9-36
- [ ] not seen · Keyring item labels: "Crake: password / username / certificate / certificate
  password for <tunnel>" · keyring.rs:29-34
- [ ] not seen · The app's words before a keyring prompt: the two sign-in checkboxes and New
  tunnel's sentences · signin_view/question.rs:148, :169, new_tunnel/sign_in.rs:145, :209
- [ ] not seen · System notifications as the desktop draws them (section 4) · notify.rs:30-47
- [ ] not seen · Clipboard: 7 writes (section 4, "Copied") · full_name.rs:78
- [ ] not seen · GNOME: the AppIndicator extension switched on over D-Bus ·
  sys/linux/gnome_tray.rs:36-50
- [ ] not seen · GNOME: the Extensions app opened · sys/linux/gnome_tray.rs:50-58
- [ ] not seen · Updates: asked once a day; "Download" opens the installer or the release page; no
  installer inside the app · versions.rs:15, :33-48, :101, updates.rs:226-277
- [ ] not seen · Update by package: the newer service's version brings "Restart" · versions.rs:136,
  requests.rs:406
- [ ] not seen · Sound: the call through pw-play / paplay / aplay (Linux in effect) ·
  notify.rs:336-362
- [ ] not seen · Terminal (`crake report`): "Saved <path>", "Couldn't save the report: <why>",
  usage · report.rs:316-340

## 6. Processes end to end (each attempt, then how it fails)

### First run

- [ ] not seen · Start → tray icon → panel "No tunnels yet" → "Add a tunnel…" → New tunnel ·
  main.rs:337-403, panel/first_run.rs:122-136
- [ ] not seen · Failed: no tray → note, notification or Settings row, with one of 9 remedies ·
  tray_host.rs:119-141, no_tray.rs:54-83
- [ ] not seen · Failed: the service isn't there → the panel's service body · panel/mod.rs:254
- [ ] not seen · macOS: approval needed → "Open Login Items" → System Settings → the panel fills
  in · updates.rs:142-180, panel/mod.rs:260

### New tunnel, per protocol

- [ ] not seen · Detect / AnyConnect: name + gateway → Crake asks the gateway → method → routing →
  "Add and connect" → the service names it → switched on 400 ms later → sign-in window ·
  new_tunnel/steps.rs:162-201, updates.rs:10, :38-52
- [ ] not seen · Failed: no answer, or not AnyConnect · new_tunnel/server.rs:129-134
- [ ] not seen · Failed, step 1: "Give it a name." · new_tunnel/steps.rs:62
- [ ] not seen · Failed, step 1: the missing-engine sentence · new_tunnel/steps.rs:63
- [ ] not seen · Failed, step 1: "Which profile? Import its .ovpn file." · new_tunnel/steps.rs:69
- [ ] not seen · Failed, step 1: "Which gateway? Its name or address, like vpn.example.com." ·
  new_tunnel/steps.rs:72
- [ ] not seen · Failed, step 1: "Still asking the gateway. A moment." · new_tunnel/steps.rs:75
- [ ] not seen · Failed, step 1: "Pick the protocol: the gateway didn't say." ·
  new_tunnel/steps.rs:76
- [ ] not seen · Failed, step 2: "Your username, as your IT team gave it." · new_tunnel/steps.rs:82
- [ ] not seen · Failed, step 2: "Choose the certificate file." (two cases) ·
  new_tunnel/steps.rs:89, :96
- [ ] not seen · Failed: the service refuses the fields (name, gateway, too many tunnels), said in
  the 8 s failure sentence · craked/src/own.rs:143-204, craked/src/manager/app_tunnels.rs:52
- [ ] not seen · Failed: the service isn't running → stays open with its sentence ·
  new_tunnel/steps.rs:13, :172-180
- [ ] not seen · Other openconnect protocols (gp, pulse, nc, fortinet, f5, array): chosen by hand,
  Beta note · new_tunnel/mod.rs:70-90, new_tunnel/server.rs:237-244
- [ ] not seen · Failed: "Not installed" · engines.rs:126-131
- [ ] not seen · OpenVPN: import .ovpn → name taken from it → "As the profile says" → routing ·
  new_tunnel/import.rs:33-128, new_tunnel/sign_in.rs:12-30
- [ ] not seen · Failed: the file's sentences · ovpn_file.rs:42-50,
  crake-proto/src/openvpn_profile.rs:37-59, new_tunnel/import.rs:64
- [ ] not seen · Failed: openvpn missing · engines.rs:112-117
- [ ] not seen · OpenVPN 3 profile taken from the list · new_tunnel/server.rs:53-66
- [ ] not seen · Failed: locked, missing or unreadable in OpenVPN 3 · new_tunnel/import.rs:140-145,
  new_tunnel/server.rs:40
- [ ] not seen · IPsec (Linux): gateway proof (key or certificate) → domains → method (password /
  certificate / both) → identity → routing · new_tunnel/ipsec_proof.rs:69,
  new_tunnel/ipsec.rs:45-54
- [ ] not seen · Failed: "Add the gateway's key, or choose Its certificate if IT gave none." ·
  new_tunnel/ipsec.rs:126
- [ ] not seen · Failed: "“<d>” isn't a domain. Just the domain, like nordbank.internal; several
  are separated by spaces." · new_tunnel/ipsec.rs:134
- [ ] not seen · Failed: the authority file isn't a certificate, or can't be read ·
  new_tunnel/ipsec_proof.rs:258-262
- [ ] not seen · Failed: strongSwan missing, with the install command · new_tunnel/ipsec.rs:228
- [ ] not seen · Failed: another system ("Crake runs IPsec on Linux only for now.") ·
  engines.rs:139
- [ ] not seen · Edit: the same window, "Save" → the service changes it ·
  new_tunnel/steps.rs:182-195
- [ ] not seen · Failed: "<x> isn't one you added here…", "Switch <x> off first: it runs on OpenVPN
  3 now." · craked/src/manager/app_tunnels.rs:82, :104
- [ ] not seen · Remove: "Remove…" → "Remove" → keyring items forgotten → "Removed <x>." ·
  requests.rs:358-378, craked/src/manager/app_tunnels.rs:217
- [ ] not seen · Failed: "<x> didn't switch off. Try again in a moment." ·
  craked/src/manager/app_tunnels.rs:196

### Sign-in, per method

- [ ] not seen · Browser: window → browser → "Waiting for you in <browser>" → tunnel up → closes ·
  signin_view/mod.rs:153-181, :465-480, signin/mod.rs:188
- [ ] not seen · Failed: this openconnect is too old or built without it · signin/mod.rs:666, :674
- [ ] not seen · Username and password (+ code): questions in turn → Continue → remembered in the
  keyring · signin_view/question.rs:90-158
- [ ] not seen · Failed: refused → asked again with "didn't accept that username or password" ·
  signin_view/question.rs:98-100
- [ ] not seen · Group choice: a button per option · signin_view/question.rs:34-66
- [ ] not seen · Certificate: "Unlock your certificate" → keep a copy · signin_view/question.rs:67-190
- [ ] not seen · Failed: wrong password asked again; "Certificate didn't open" ·
  signin_view/question.rs:72-86, signin_view/explain.rs:109-117
- [ ] not seen · Gateway not trusted: the trust question → "Trust it", remembered per gateway ·
  signin_view/question.rs:211-268, signin_view/mod.rs:520-540, signin/trust.rs:13-37
- [ ] not seen · Refused: "Don't connect" ends the sign-in · signin_view/question.rs:255
- [ ] not seen · AnyConnect-only gateway: tried again as AnyConnect, remembered, said in the
  panel · signin/mod.rs:353, signin_view/mod.rs:469-476, panel/tunnels.rs:142
- [ ] not seen · OpenVPN and IPsec (the service asks): kept answers given silently, the others
  shown (username, password, code, key password, browser page, trust) · openvpn/mod.rs:92-155,
  :221-316
- [ ] not seen · Failed: "<x> isn't asking that any more." · craked/src/manager/openvpn/mod.rs:29
- [ ] not seen · OpenVPN 3 session: started, followed, taken over when started elsewhere ·
  ovpn3/mod.rs:118-320, updates.rs:283-390
- [ ] not seen · Failed: its sentences · ovpn3/mod.rs:157-299, ovpn3/connect.rs:58,
  ovpn3/attached.rs:28
- [ ] not seen · IPsec key mismatch → "Change the key…" → Edit opens · signin_view/failed.rs:69,
  signin_view/mod.rs:625
- [ ] not seen · Any failure: title, sentence, the failed step, the program's own words, Copy for
  helpdesk, Try again · signin_view/failed.rs:9, signin_view/explain.rs:17-127
- [ ] not seen · Failed, gateway unreachable: the checks run and say why ·
  signin_view/checks.rs:82-134, :181
- [ ] not seen · Cancelled: "Don't connect" / Esc / Cmd+W → the tunnel is no longer connecting ·
  signin_view/mod.rs:601-622

### Connect and disconnect

- [ ] not seen · Switch on (panel, Settings, detail, tray menu, notification, login) → clash check
  → sign-in window, or connect with the kept sign-in · requests.rs:122-166
- [ ] not seen · The service says: "Switched <x> on.", "<x> is up. Routing nothing yet." / "…
  Sending <n> known destinations through it." / "… Everything goes through it (full tunnel).",
  "<x> is back." · craked/src/manager/tunnels.rs:57, :266-275
- [ ] not seen · Failed: a clash, "Turn <other> off to use <x>." · requests.rs:134,
  craked/src/manager/tunnels.rs:198
- [ ] not seen · Failed: "<x> is already on", "<x> is still switching off" ·
  craked/src/manager/tunnels.rs:26-34
- [ ] not seen · Failed: the kept sign-in ran out → the sign-in opens if it was switched on within
  30 s, else a notification · requests.rs:172-191, updates.rs:79-84
- [ ] not seen · Failed: "<x> stopped unexpectedly." + trouble words (14 short labels) → red
  state, Dismiss, notification · crake-proto/src/lib.rs:223, crake-proto/src/trouble.rs:43-66
- [ ] not seen · Switch off → the service (OpenVPN 3: detach, then end the session) → "Switched
  <x> off." · requests.rs:217-241, craked/src/manager/tunnels.rs:491
- [ ] not seen · Dismiss trouble → Off, the reason kept in tunnel detail · requests.rs:526-541
- [ ] not seen · Reconnecting: "<x> lost the gateway. Trying again." → "Lost the gateway · trying
  again…" · craked/src/manager/hook.rs:17, model/mod.rs:333
- [ ] not seen · UDP blocked: "UDP is blocked here. <x> runs over TCP on this network." ·
  craked/src/manager/tunnels.rs:392-417
- [ ] not seen · A tunnel's DNS stops answering: "<x>'s DNS (…) doesn't answer." … "answers
  again." · craked/src/manager/outage.rs:67-86
- [ ] not seen · Network or device gone, and back · craked/src/manager/gone.rs:11-39

### Learning a host

- [ ] not seen · By DNS: "Sent <host> through <x>." + "Only <x>'s DNS knows it (…). Routed before
  the first packet." (4 reasons) · craked/src/manager/naming.rs:383-398
- [ ] not seen · By probing: "Sent <host> through <x>." + how it failed directly + "Through <x> it
  answered in <n> ms." · craked/src/manager/probing.rs:313-353, craked/src/probe.rs:34-36
- [ ] not seen · Shown as: tray shape and frames, notification with Undo, the row sliding in, the
  tick growing · updates.rs:65-131, panel/decisions.rs:53, panel/chart.rs:37
- [ ] not seen · First route ever: the notice until "Got it" · panel/moments.rs:13
- [ ] not seen · Not learned: "Left <host> alone." + "No answer just then, directly or through
  <x>. Nothing was changed." and its other reasons · craked/src/manager/probing.rs:285-302,
  :412-429
- [ ] not seen · Ask me first: "<host> needs <x>." + "You asked to decide these yourself." →
  notification or row, "Send through <x>" / "Leave it" · craked/src/manager/probing.rs:36-37,
  notify.rs:155, panel/decisions.rs:191
- [ ] not seen · Rechecked: "<host> goes direct again." + "It answers directly now…" ·
  craked/src/manager/recheck.rs:155-157
- [ ] not seen · Added by hand: field + "Send through <x>" → "Sent <host> through <x>." + "You
  added it." (a network is pinned) · tunnel_detail/mod.rs:400-411,
  craked/src/manager/host_ops.rs:30-131
- [ ] not seen · Failed: its refusals (no DNS, DNS doesn't know it, already through another, covers
  the gateway, IPv6 here, only single hosts) · craked/src/manager/host_ops.rs:30-131, :563-593

### Undo and Redo

- [ ] not seen · Undo (panel row or notification) → "<host> goes direct again." + "You undid the
  route. Crake won't learn this host again on its own." → the row offers Redo ·
  requests.rs:470-476, craked/src/manager/host_ops.rs:206-211
- [ ] not seen · Redo → "Sent <host> through <x> again." + "You redid it." ·
  craked/src/manager/host_ops.rs:298-303
- [ ] not seen · Failed: "<a> is pinned to <x>. Unpin it first.", "<x> isn't sent through <y>.",
  "<x> wasn't sent direct by you.", "Only single hosts can be undone…" ·
  craked/src/manager/host_ops.rs:183-191, :290, :563

### Pin and forget

- [ ] not seen · Pin → "Pinned <t> to <x>." + "It stays until you unpin it." ·
  tunnel_detail/hosts.rs:367, craked/src/manager/host_ops.rs:335-340
- [ ] not seen · Unpin → "Unpinned <t> from <x>." + "It goes direct again." ·
  craked/src/manager/host_ops.rs:386-390
- [ ] not seen · Forget → "Stopped sending <t> through <x>." + "You forgot it. Next time it's
  needed, Crake finds out again." · tunnel_detail/hosts.rs:381,
  craked/src/manager/host_ops.rs:441-446
- [ ] not seen · Forget all → question → "Forgot the <n> hosts Crake learned for <x>." ·
  craked/src/manager/host_ops.rs:497-503
- [ ] not seen · Failed: already pinned, not pinned, not sent through it, too wide or a bad
  address, "Crake hasn't learned anything for <x>." · craked/src/manager/host_ops.rs:320, :368,
  :420, :478, craked/src/hosts.rs:87-168

### When two tunnels answer, and short names

- [ ] not seen · Ask me: routed through the first, "<Y> answers for <host> too." → notification
  ("Use <Y>", "Keep it") and panel line ("Use <Y>", "Keep <X>") ·
  craked/src/manager/probing.rs:379-381, notify.rs:124, panel/moments.rs:91
- [ ] not seen · Move → "Sent <host> through <y> instead of <x>." + "You chose <y>." ·
  craked/src/manager/host_ops.rs:259-264
- [ ] not seen · Use the faster one: the reason gains " The fastest of the tunnels that
  answered." · craked/src/manager/probing.rs:348
- [ ] not seen · Failed: pinned to another; "<host> can't go through <x>. Its address … already
  goes through …" · craked/src/manager/host_ops.rs:236, craked/src/manager/clash.rs:114-116
- [ ] not seen · Short name: question (notice, notification, log row) → "Use <A>'s" → "<short> is
  <full> now." · panel/question.rs:29, craked/src/manager/short_names.rs:159-268
- [ ] not seen · Short name: "Not now" puts it off for the day · panel/question.rs:72
- [ ] not seen · Short name forgotten → "Forgot <short>." · tunnel_detail/short_names.rs:41,
  craked/src/manager/short_names.rs:268

### Settings changes

- [ ] not seen · A setting of the service's (wait, both answer, forget after, never list, private,
  ask-unknown, log days, order) → sent → changed → read again · settings_view/mod.rs:439-443,
  client.rs:476
- [ ] not seen · Failed: "Wait 1, 2, 3 or 5 seconds.", "<v> isn't a number of days", "<item> is
  neither a host, a network nor a name.", "no setting called …" → the 8 s failure sentence ·
  craked/src/settings.rs:106-211
- [ ] not seen · A preference of the app's own (notify, appearance, bird, reconnect, version check,
  hide hosts), kept locally · settings_view/mod.rs:446-450, prefs.rs:117-128
- [ ] not seen · Start when you log in → the system's entry written or removed ·
  settings_view/general.rs:61
- [ ] not seen · Show the bird off / on → the tray stopped / started · requests.rs:106
- [ ] not seen · Tunnel order: a row dragged onto another · settings_view/tunnels.rs:43-56
- [ ] not seen · Clear log → question → cleared · settings_view/privacy.rs:53-54, client.rs:577-583

### Reports and copies for the helpdesk

- [ ] not seen · Panel "Copy for helpdesk" → text built, hosts hidden when chosen → "Copied" ·
  panel/mod.rs:642-645, model/helpdesk.rs:17-87, model/scrub.rs:91
- [ ] not seen · Log "Copy for helpdesk" → "Crake log, <now>" + the rows shown ·
  log_view/mod.rs:278-312
- [ ] not seen · Sign-in failed "Copy for helpdesk" → what failed and what was checked ·
  signin_view/mod.rs:577-598
- [ ] not seen · Save a report… → picker → Saving → Saved (Open / Show in folder) ·
  settings_view/diagnostics.rs:244-299, text report.rs:170-255
- [ ] not seen · Picker cancelled: nothing said · settings_view/diagnostics.rs:256
- [ ] not seen · Failed: "Crake couldn't ask its service." (6 reasons behind it) ·
  settings_view/diagnostics.rs:272, report.rs:55-101
- [ ] not seen · Failed: a write error, "Couldn't save the report." + reason ·
  settings_view/diagnostics.rs:180
- [ ] not seen · Settings closed meanwhile: a notification says how it ended ·
  settings_view/diagnostics.rs:290-292
- [ ] not seen · `crake report` in a terminal: saved, failed, usage, not-read sentences ·
  report.rs:58-134, :316-340

### Update, restart, What's new

- [ ] not seen · A package updated the service → "Crake was updated to <v>. Restart it to finish."
  → Restart starts the new binary and quits; tunnels stay · panel/version.rs:28,
  requests.rs:406-416, craked/src/manager/handover.rs:191
- [ ] not seen · Failed: a Restart whose new binary doesn't start does nothing, and says nothing ·
  requests.rs:406-416
- [ ] not seen · A newer version exists → "Crake <v> is out." → Download (browser) or Not now (for
  that version) · panel/version.rs:32-37
- [ ] not seen · Failed: offline → asked again in an hour, nothing shown · updates.rs:253
- [ ] not seen · After an update: "Crake is now <v>…" → the What's new window, or Not now; the
  version marked seen · whats_new/mod.rs:58-78, requests.rs:383-390
- [ ] not seen · What's new from About's link · settings_view/about.rs:39-47

### OpenVPN 3

- [ ] not seen · Moving a tunnel from OpenVPN 3 to Crake's own OpenVPN, at start → "<x> now runs
  on Crake's own OpenVPN." · openvpn/mod.rs:387-474, craked/src/manager/app_tunnels.rs:140-142
- [ ] not seen · Failed: locked or refused → the staying notice in the panel (no notification) ·
  openvpn/mod.rs:453-473
- [ ] not seen · Failed: the profile gone or openvpn3 not answering → tried again at the next
  start, nothing said · openvpn/mod.rs:462
- [ ] not seen · A session started elsewhere is taken over (looked for every 3 s) ·
  updates.rs:12, :283-390
- [ ] not seen · A leftover process → log entry, notification, panel notice with `sudo kill -9
  <pid>`, amber badge; goes when the process ends · updates.rs:86-88, :313-361,
  panel/leftover.rs:40
- [ ] not seen · Edit a profile: "Edit…" → editor → "Save and reconnect" → off and on again ·
  profile_editor/mod.rs:266-306, requests.rs:546-580
- [ ] not seen · Failed: refused by OpenVPN 3, or a folded line was changed ·
  ovpn3/profile.rs:143, :165, profile_editor/text.rs:86

### Quit and closing

- [ ] not seen · Tray menu "Quit" → the app quits; the service and its tunnels stay; no question ·
  sys/linux/tray.rs:137-139, sys/menu_tray.rs:104, requests.rs:380
- [ ] not seen · macOS: Cmd+Q from any window · focus.rs:74-89
- [ ] not seen · Closing windows (×, Esc, Cmd+W; the panel on focus loss) never quits ·
  panel/mod.rs:462-473, main.rs:330

## 7. Motion

Crake's own motion is off when `motion::enabled()` is false: GNOME's `enable-animations` is false,
or `CRAKE_MOTION=off` (motion.rs:11-24). The toolkit's reduce-motion is set from it (main.rs:332).
`CRAKE_MOTION=slow` makes everything ten times slower (motion.rs:27-33).

- [ ] not seen · Panel opens: rises 8 px (drops, from a top bar) and fades in over 180 ms,
  ease-out · panel/mod.rs:59-60, :236-239
- [ ] not seen · Panel opens, reduced: appears at once · panel/mod.rs:112-113
- [ ] not seen · Panel refit keeps the same motion, so nothing replays · panel/mod.rs:71-79,
  windows.rs:57
- [ ] not seen · Panel's upward refit repeats after 150 ms and 500 ms · windows.rs:14
- [ ] not seen · New decision row slides in 6 px and fades, 220 ms · panel/decisions.rs:8, :53-61
- [ ] not seen · New decision row, reduced: at once · panel/decisions.rs:53-61
- [ ] not seen · New chart tick grows in two strokes, 260 ms each, the second 90 ms later ·
  panel/chart.rs:10-11, :33-41
- [ ] not seen · New chart tick, reduced: drawn whole · panel/chart.rs:33-41
- [ ] not seen · Switch knob and track: a critically damped spring, about 150 ms ·
  panel/mod.rs:61-62, :363-399
- [ ] not seen · Switch, reduced: lands at once (the toolkit's reduce-motion) · main.rs:331-332
- [ ] not seen · First-run bird peeks out of the grass: 2.8 s, first after 0.6 s, then every 3 to
  8 s · panel/first_run.rs:19-24, :50-57, :143-175
- [ ] not seen · First-run bird, reduced: still · panel/first_run.rs:144-146
- [ ] not seen · Dashed head: dashes go round, a dash and gap per 0.7 s · head.rs:14, :88-107
- [ ] not seen · Dashed head, reduced: still; the seconds still count · head.rs:93-99, :80-82
- [ ] not seen · Scrollbar: fades in 150 ms, holds 300 ms, fades out 300 ms ·
  controls.rs:102-104, :152-170
- [ ] not seen · Scrollbar, reduced: no gate of Crake's own; the toolkit draws each animation's
  end state, so it appears at once and goes at once, without the hold · controls.rs:152-170
- [ ] not seen · Traffic: dots move along the paths at a speed set by the rate; a newly routed
  destination's path draws in · traffic_view/mod.rs:138-148, traffic_view/graph.rs:39-60, :317-411
- [ ] not seen · Traffic, reduced: dots stand still · traffic_view/mod.rs:138
- [ ] not seen · Traffic numbers and strip change on the poll, with no easing ·
  traffic_view/mod.rs:86-102
- [ ] not seen · Tray: connecting → listening (outline fills, eye opens), frames 70 ms apart ·
  tray/motion.rs:11-131, updates.rs:18, :425-473
- [ ] not seen · Tray: → routed (the "crex, crex" marks) · tray/motion.rs:11-131
- [ ] not seen · Tray: routed → listening (marks fade) · tray/motion.rs:11-131
- [ ] not seen · Tray: → off (eye closes, head drops) · tray/motion.rs:11-131
- [ ] not seen · Tray: off → listening (wakes) · tray/motion.rs:11-131
- [ ] not seen · Tray: a badge appears in 4 steps · tray/motion.rs:11-131
- [ ] not seen · Tray connecting loop: dashes round the outline, 10 steps · tray/motion.rs:140-150
- [ ] not seen · Tray, reduced: the resting icon only · updates.rs:429-436
- [ ] not seen · Tray "routed" shape held, then back · updates.rs:11, :125-130
- [ ] not seen · Tray told about the icon again 300 ms after a change (not visible motion) ·
  updates.rs:22, :418
- [ ] not seen · Windows fit their content on the next frame, a jump with no easing (panel,
  sign-in, New tunnel, tunnel detail) · panel/mod.rs:404, signin_view/mod.rs:285,
  new_tunnel/view.rs:148, tunnel_detail/mod.rs:121
- [ ] not seen · Day chart: zoom and pan follow the wheel and the drag directly; the hint shows by
  hover opacity, with no timing · log_view/day_chart.rs:213-300
- [ ] not seen · Help cards, menus, questions in place, notices, hover and selection: no
  animation, they change at once · settings_view/mod.rs:500-526
- [ ] not seen · Timed, not animated: "Copied" 2 s (panel, log) or 3 s (host name, leftover,
  no-tray, sign-in) · panel/mod.rs:600, log_view/mod.rs:307, full_name.rs:25,
  panel/leftover.rs:10, no_tray_view.rs:19, signin_view/mod.rs:8
- [ ] not seen · Timed, not animated: the failure sentence goes after 8 s, with no fade ·
  model/mod.rs:50
- [ ] not seen · Timed, not animated: the sign-in window closes 0.9 s after "Tunnel up" ·
  signin_view/mod.rs:32, :380
- [ ] not seen · Sound: the "crex crex" call with a notification, when chosen · notify.rs:336

## 8. Proof

### Who made it

Two independent agents each made an inventory from the code (one by file sweep, one by entry
points and strings); a third reconciled them against the code. Nobody has looked at the running
app for this list.

### Files swept

136 non-test `.rs` files under `crates/crake/src` (177 in all; the 41 left out are tests:
`tests.rs`, `tests_ipsec.rs`, `e2e_*`, `journey_*`, `*_journey.rs`, `test_app.rs`,
`wire_tests.rs`, `windows_tests.rs`, `macos_packaging_tests.rs`, `scenarios/`, `gateway_tests/`).
Counted with:

    find . -name '*.rs' | grep -Ev '(^|/)(tests|tests_ipsec|test_app|wire_tests|windows_tests|
      macos_packaging_tests|e2e_[a-z]+|journey_[a-z]+|macos_journey|windows_journey)\.rs$|
      /scenarios/|/gateway_tests/' | wc -l          (one line; 136)

By module:

- root (32): assets, autostart, browser_dns, chart, chrome, client, controls, detect, engines,
  focus, full_name, gateway_cert, head, keyring, main, motion, no_tray, no_tray_view, notify,
  ovpn_file, place, prefs, report, requests, service, theme, timeline, tray_host, units, updates,
  versions, windows
- windows (1): dock
- log_view (3): day_chart, mod, rows
- model (10): days, demo, helpdesk, kept, live, mod, moments, scrub, standing, waiting
- new_tunnel (10): import, ipsec, ipsec_proof, mod, routing, server, sign_in, steps, view, widgets
- openvpn (2): kept, mod
- ovpn3 (6): attached, bus, connect, leftover, mod, profile
- panel (11): chart, decisions, first_run, leftover, mod, moments, no_tray, question, staying,
  tunnels, version
- profile_editor (2): mod, text
- settings_view (11): about, diagnostics, general, learning, managed, mod, names, privacy,
  tray_row, tunnels, widgets
- signin (2): mod, trust
- signin_view (6): checks, explain, failed, mod, question, widgets
- sys (23): mod, unix, menu_tray; linux/{autostart, gnome_tray, host, keyring, mod, root_cursor,
  single, tray}; macos/{autostart, host, keyring, mod, service, single, trust};
  windows/{autostart, host, keyring, mod, single}
- traffic_view (4): graph, mod, strip, tables
- tray (4): art, family, mod, motion
- tunnel_detail (5): chart, facts, hosts, mod, short_names
- whats_new (4): changelog, mod, open, view

Lines in `crates/craked` and `crates/crake-proto` (the sentences the service writes into the log,
notifications and failure sentences) are taken from the two inventories; of those, only
`crake-proto/src/trouble.rs`, `craked/src/dbus/guard.rs` and the notification call sites were
opened again here.

### Count per shared piece, verified against the code

All greps are `grep -nE '<pattern>'` over the 136 files, the definition's own line taken out.

- Windows: 9 · `open_window\(` (8 in windows.rs, 1 in whats_new/open.rs)
- Focus ring: 104 · `\.keys\(\)` gives 105, less requests.rs:268 (a map's keys). Per module: panel
  24, settings 31, sign-in 15, tunnel detail 12, log 9, profile editor 5, traffic 3, What's new
  2, no-tray view 2, full name 1, New tunnel 0
- Pressed looks: 0 · `\.active\(`
- Hover and focus style sites: 33 · `\.hover\(|group_hover\(|on_hover\(|focus_visible`
- Switch: 9 (2 direct, 7 through Settings' `toggle`) · `[^_a-z]switch\(|[^_a-z.]toggle\(`
- Segmented control: 2 · `controls::segment\(`
- Small action: 11 · `controls::action\(`
- `controls::copied`: 2 (the other 5 "Copied" words are drawn by their own code) ·
  `controls::copied\(`
- "Copied" acknowledgements: 7, one per clipboard write · `write_to_clipboard`
- Scroll areas: 8, and 1 always-shown bar · `controls::over(_shown)?\(`
- Close button: 7 · `chrome::close_button\(`
- Card edge: 8; window background: 9; header beside the system's buttons: 4 ·
  `card_edge\(|on_buttons_line|window_background\(`
- Dashed head: 3 · `head::under_way\(`
- Host name with its card: 4 uses; tooltips: 1 · `host_name\(`, `\.tooltip\(`
- Amber warning icon: 3 · `warning_icon\(`
- Settings select: 5; New tunnel dropdown: 4; log picker: 1 (10 menus in windows, 3
  implementations) · `[^_a-z.]select\(`, `\.dropdown\(`
- Settings outline button: 4; small button: 4 · `[^_a-z]outline\(|small_button\(`
- Settings title rows: 17 call sites, 16 labels · `self\.title\(`
- Help texts: 12 (11 arms of `help()` and the panel's); shown: 11 · `fn help` at
  settings_view/mod.rs:344, `"Try tunnels in this order"`
- Checkbox: 2 implementations; 4 uses in New tunnel, 2 in sign-in · `[^_a-z]checkbox\(`
- New tunnel card: 5; radio: 2; field: 8 · `[^_a-z.]card\(`, `[^_a-z]radio\(`, `[^_a-z.]field\(`
- Sign-in button: 12; sign-in link: 1; Settings link: 4 · `[^_a-z.]button\(`, `[^_a-z.]link\(`
- Panel button: 4; profile editor button: 4; no-tray button: 2 · `[^_a-z.]button\(`
- Log outline button: 3; detail row button: 4; detail header button: 2 ·
  `outline_button\(|row_button\(|header_button\(`
- Text inputs: 6 places a toolkit input is drawn, 1 code editor · `Input::new\(|Editor::new\(`
- Deferred overlays: 5 · `deferred\(`
- Questions in place: 5 (4 confirmations, 1 trust question); no dialog component
- Panel notices: 11; with a button that puts them away: 9 · strings in panel/version.rs,
  moments.rs, no_tray.rs, question.rs, leftover.rs, staying.rs, tunnels.rs
- Failure sentences: 3 (panel, Settings, tunnel detail)
- System notifications: 8 kinds, 10 places that send one, the call played at 2 ·
  `notify::[a-z_]+\(` gives 13 lines: 10 sends, 2 `crex`, 1 `worth_telling`
- File pickers: 4 (3 open, 1 save) · `prompt_for_paths\(|prompt_for_new_path\(`
- `open_url`: 6; open with the system: 1; show in folder: 1 ·
  `open_url\(|open_with_system\(|reveal_path\(`
- Tray shapes: 10 (+ 5 whole-bird drawings)
- Protocols in New tunnel: 10 · the array's length at new_tunnel/mod.rs:62
- Motion sites: `with_animation|with_spring|with_transformation|request_animation_frame|motion::`
  in 9 files (controls, head, main, panel/first_run, panel/mod, traffic_view/mod, tray/mod,
  tray/motion, updates); panel/chart.rs and panel/decisions.rs animate through `phase(`

### Differences resolved

A is the file sweep, B the entry points and strings.

- Windows, 10 (A) or 9 (B): 9. A's tenth was `crake report`, which opens no window; it is listed
  under "Surfaces that are not windows" · windows.rs:167-622, whats_new/open.rs:29
- Confirmations, 4 (A) or 5 (B): both, counted apart. 4 confirm the user's own act; the trust
  question has the same two-answer shape but is asked by the app during sign-in ·
  signin_view/question.rs:211-268
- Panel notices, 11 (A) or 10 dismissable / 27 lines (B): 11 notices. 9 have a button that puts
  them away; Restart has none (panel/version.rs:30); "answers for it too" is answered, not put
  away (panel/moments.rs:116-135). B's 10 left that one out. B's 27 counts every notice-like line
  in every window; both lists are kept, apart
- Notification call sites, 13 (A) or 10 (B): 10 sends and 2 plays of the call. A counted the two
  `crex` lines and openvpn/mod.rs:457, which builds the staying notice's title and sends nothing ·
  updates.rs:68-123, tray_host.rs:139, settings_view/diagnostics.rs:290-292
- "<x> stays on OpenVPN 3" as a notification (A, twice): wrong. It is the panel's staying notice
  only; `notify::said` is called at the report's two places and nowhere else ·
  openvpn/mod.rs:457-473
- Help cards, 12 (A) or 11 (B): 12 texts, 11 shown. "Try tunnels in this order" has text and no
  title that asks for it · settings_view/mod.rs:353
- Tooltips, "1 implementation, 4 uses + the tray's" (A) or "4 + 2 + 4" (B): the same things. One
  `.tooltip(` (full_name.rs:71) used at 4 places, 2 tray tooltips, and B's last 4 are 3 chart
  readouts and a hint, which are hover surfaces, not tooltips
- Menus, "11 uses" (A's heading) or 10 (B): 10. A's own list has 10
- Tray menus, 2 (A) or 1 in 2 backends (B): one menu, drawn by two backends
- Protocols, 11 (A) or 10 (B): 10 · new_tunnel/mod.rs:62
- Sign-in failures explained, "8 ways" (A, listing 9) or 15 sets (B): 9 · signin_view/explain.rs:21,
  :34, :42, :51, :70, :93, :101, :114, :121
- Trouble labels, 14 (A) or 11 (B): 14, 11 matched by how the reason starts and 3 after the
  tunnel's name · crake-proto/src/trouble.rs:43-66
- Validation sentences, 10 (A) or 9 (B): 8 cases with 7 fixed sentences and the missing-engine
  sentence in new_tunnel/steps.rs:62-96, and 3 more for IPsec (key, authority file, domain) in
  new_tunnel/ipsec.rs:119-136
- Headers beside the system's buttons (`on_buttons_line`), 5 (A): 4 · log_view/mod.rs:332,
  profile_editor/mod.rs:462, traffic_view/mod.rs:492, tunnel_detail/mod.rs:152
- Scroll areas, "7 … = 8" (B) or 8 (A): 8
- Sentences after the reachability checks, 6 (A): 7 · signin_view/checks.rs:117, :120, :124, :127,
  :129, :132, :134
- Short-name notification's time, none (A) or 20 s (B): 20 s · notify.rs:77, :184
- Tray "routed" shape, 5 s (A) or about 4–5 s (B): the shape lasts 4 s (model/mod.rs:36), the
  tray is refreshed after 5 s (updates.rs:11)
- View line numbers, Settings 191 / 190, New tunnel 124 / 123, sign-in 139 / 134: `impl Render` is
  at 190, 123 and 138; A gave `fn render`, B's 134 was off
- Scrollbar under reduced motion, "left to the toolkit" (A) or "not determined" (B): settled.
  controls.rs has no gate of its own, and the toolkit's `with_animation` draws the end state when
  reduce-motion is set (gpui-pre 0.3.12, src/elements/animation.rs:78, :407), which main.rs:332
  sets
- What's new with nothing to show, "none hides the notice" (A) or "not determined" (B): both
  hold. The notice is hidden (whats_new/mod.rs:66-67); the window itself has no branch for it
  (whats_new/view.rs:187-196)
- Only in B, kept: the service restarting, the 10 s wait after Restart, the bird hidden,
  appearance, the per-platform lines, each switch as its own line, primary buttons, the busy and
  disabled states, window fits and the tray's re-announce under motion, the service's terminal
  console, the Windows executable icon · lines in sections 1 to 7
- Only in A, kept: the drag ghost, the macOS line per window, the 5 s refresh of tunnel detail,
  "accepted the sign-in, but the tunnel didn't stay up", questions / browsers / leftovers /
  releases as entities, the app icon's drawing, DNS outage and network gone as processes · lines
  in sections 1 to 6

### Still not determined

- The hover, focus, pressed and disabled looks of the toolkit's own widgets (text input, code
  editor, scrollbar thumb, the password eye): drawn outside this crate.
- What the keyring, the file pickers and the notifications look like and say: the system's.
- Whether What's new can open with nothing to show.
- Which of the service's DNS sentences reach "Not kept private right now. <why>" and which the
  helpdesk copy.
- How the app icon reaches the Linux and macOS packages (packaging was not read).
- Whether the toolkit follows a reduce-motion setting on Windows and macOS by itself.
- Line references into `crates/craked` beyond the three files named above were not opened again.

## Found on the way

Verified in the code. These are bugs or gaps in the app, not design grades.

- No pressed look anywhere: `.active(` occurs 0 times in the UI crate; a press changes nothing
  until the click lands · grep above; e.g. panel/mod.rs:339-359, controls.rs:53-67
- New tunnel can't be worked from the keyboard beyond its text fields: `new_tunnel/` has 0
  `.keys()`; Tab moves between the step's text fields only, so the step tabs, protocol and choice
  cards, checkboxes, dropdowns, "Import .ovpn…", Back, Continue and the close button take no
  focus · new_tunnel/steps.rs:16-34, new_tunnel/view.rs:186-188, new_tunnel/view.rs:94
- Two notifications ignore "Say nothing": "Ask me first" (`notify::asked`) is sent whatever the
  choice, and so is "another tunnel answered too" (`notify::also_answered`); every other one
  checks `how != "none"` · updates.rs:104, updates.rs:115-123 (the checks: :67, :74, :82, :87, :95)
- Two checkbox implementations that differ: New tunnel's has a 1.5 px edge on and off and a 12 px
  check; sign-in's has a 1 px edge only when off and a 14 px check ·
  new_tunnel/widgets.rs:206-241, signin_view/widgets.rs:183-216
- "Try tunnels in this order" has help text and nowhere to show it: no title is drawn with that
  label (the Tunnels section has no such row), so the card can't open · settings_view/mod.rs:353;
  the 17 `self.title(` calls are in general.rs, learning.rs, names.rs, privacy.rs, tray_row.rs
- Nothing says the service is out of reach in Settings, the log, tunnel detail or Traffic:
  among the views, `reachable` is read only by the panel and New tunnel's last step (besides the
  status word, the tray's shape, autostart and the service watch). The tunnels are cleared when
  it goes, so an open tunnel detail goes blank; Settings and the log keep what they last had ·
  panel/mod.rs:169, new_tunnel/steps.rs:175, model/mod.rs:402-407, tunnel_detail/mod.rs:89-96
- No launch-failure surface: a second start exits with 0 and no word; a window that fails to open
  is dropped silently (only sign-in writes a debug line); a Restart whose new binary doesn't
  start does nothing · main.rs:403-405, windows.rs:193, :286, :345, :409, :456-460, :531, :581,
  :660, requests.rs:406-416
- Scrollbar fade and reduced motion: true as flagged that nothing in controls.rs asks
  `motion::enabled()`; not a live fault, since the toolkit draws the end state of each animation
  once reduce-motion is set from it. The side effect is that the bar then goes the moment the
  pointer leaves, without its 300 ms hold · controls.rs:152-170, main.rs:332
- Found beside it: reduced motion is read from GNOME only. On Windows and macOS the `gsettings`
  call fails, motion counts as on, and the app sets the toolkit's reduce-motion to off ·
  motion.rs:11-20, main.rs:332
- Found beside it: the Restart notice is the one panel notice with no way to put it away ·
  panel/version.rs:28-31
