# Linux and Windows conventions: a reference study for Crake

Researched October 2026 for Crake's design pass: what the platform's own network UI, tray and notifications already do, so that the bird in the bar, the panel that drops from it, the tunnel rows, the sign-in window and the notifications sit beside them without surprising anyone. The surfaces in question are the panel (`tray-panel.html`: header, Last hour, What Crake did, Tunnels), tunnel detail, sign-in, Settings → General, and the native fallback menu.

**How to read this.** Each section studies one platform surface or convention, says which version was read, and lists only what bears on Crake. A point marked *(observed)* comes from my own knowledge of the shipping system, not from a cited page; check it on a real machine before quoting it as fact. Everything else links to its source. Many GNOME facts come from reading the source code itself (gnome-shell, gnome-control-center and the AppIndicator extension on GitHub), which is more reliable than the release notes. Each section ends with **For Crake:** (the pattern and the screen it changes) and **GPUI:** (whether gpui-kit 0.7 can draw it in Crake's own windows, whether it needs the OS, or whether it can't be done). Wherever DESIGN.md's rejected directions disagree with a reference, the rejection wins, and the pattern goes into "Not for Crake" at the end.

**Selection.**

| # | Surface | Version read | Why it's here |
|---|---|---|---|
| 1 | GNOME Settings → Network → VPN | gnome-control-center `main` (GNOME 50/51) | How Linux lists a VPN, its state words, its detail page |
| 2 | GNOME Shell Quick Settings: the VPN toggle | gnome-shell `main`; GNOME 43 → 50 | The system's own tunnel switch, beside Crake's bird |
| 3 | GNOME Shell notifications | gnome-shell `main`; GNOME 48–49 changes | Where "Sent … through ACME." with Undo actually appears |
| 4 | AppIndicator and KStatusNotifierItem Support | v66 (GNOME Shell 45–51) | What a click on the bird does |
| 5 | Dash to Panel | `master` (GNOME Shell 46–51) | The user's own bar: bottom, 48 px |
| 6 | GNOME HIG: toasts, banners, placeholders, capitalisation | developer.gnome.org/hig, October 2026 | The words and shapes GNOME apps use for events and states |
| 7 | Circle and neighbours: Warp, Fragments, Impression, Resources, Mission Center | 1.0.0, 3.0.1, 3.8.0, 51.0, 1.2.0 | Status pages, self-healing, honest copy, quiet graphs |
| 8 | The Windows notification area (tray icon) | Win32 UX guide (Windows 7, still current) and Windows 11 settings | Clicks, menus, overlays and the overflow |
| 9 | Windows 11 Quick Settings and the network flyout | Windows 11 23H2/24H2 | The VPN switch and the state words beside Crake |
| 10 | Fluent flyouts and geometry | Microsoft Learn, 2025–2026 | How Crake's panel behaves as a flyout |
| 11 | Windows app notifications (toasts) | Microsoft Learn, July 2026 | Undo, buttons, attribution, duration |

---

## 1. GNOME Settings: Network → VPN

The Settings app's Network page, as built from `main` (Settings 50/51). Ubuntu 26.04 ships GNOME 50.

**The list**
- A "VPN" group with a flat "+" ("Add VPN") in its header; each VPN is one `Adw.ActionRow` titled with the connection's name, with a switch and a flat cog ("VPN Options") as suffixes ([network-vpn.blp](https://github.com/GNOME/gnome-control-center/blob/main/panels/network/network-vpn.blp); [cc-network-panel.blp](https://github.com/GNOME/gnome-control-center/blob/main/panels/network/cc-network-panel.blp)).
- The row says nothing about state: no subtitle, no "Connected". The switch is the whole status ([network-vpn.blp](https://github.com/GNOME/gnome-control-center/blob/main/panels/network/network-vpn.blp)).
- With nothing set up, the list's placeholder is the dimmed line "Not set up" ([cc-network-panel.blp](https://github.com/GNOME/gnome-control-center/blob/main/panels/network/cc-network-panel.blp)).

**The detail**
- The cog opens a connection editor dialog titled with the connection's name, with Cancel / Apply ([net-connection-editor.c](https://github.com/GNOME/gnome-control-center/blob/main/panels/network/connection-editor/net-connection-editor.c)). Its pages are Details, Identity, IPv4 and IPv6 *(observed)*.
- Details lists facts as label/value pairs: "IPv4 Address", "IPv6 Address", "Default Route", "DNS", "Last Used" (Today, Yesterday, Never), then "Connect automatically" and "Make available to other users" ([ce-page-details.blp](https://github.com/GNOME/gnome-control-center/blob/main/panels/network/connection-editor/ce-page-details.blp); [net-connection-editor.c](https://github.com/GNOME/gnome-control-center/blob/main/panels/network/connection-editor/net-connection-editor.c)).
- Removal is "Remove VPN…", confirmed with the consequence: "Saved details for “%s” will be permanently lost. This includes passwords and any network changes." ([net-connection-editor.c](https://github.com/GNOME/gnome-control-center/blob/main/panels/network/connection-editor/net-connection-editor.c)).
- GNOME 50's Settings changes touched Date & Time, Sound and the modem dialogs, not the VPN page ([GNOME 50 release notes](https://release.gnome.org/50/)).

**For Crake:** two things carry over. First, the switch is the state, and the row's name is the tunnel's name. The panel's Tunnels section already works this way and adds what GNOME leaves out: a second line in Crake's words (`model::tunnel_state`: "Connected · 5 hosts", "Connecting…", "Off", the trouble sentence). Keep that line; it is where Crake is better than the platform, not different from it. Second, tunnel detail's facts block ("Connected for", "Your address", "Protocol", "Gateway") is GNOME's Details page in sentence case. A user who has seen both reads it at once. Remove… in Settings → Tunnels can borrow GNOME's consequence sentence in Crake's voice: "ACME's sign-in and the hosts it learned will be forgotten. The log keeps what happened." That copy is new; check it against what Remove actually deletes first.

**GPUI:** all of it is Crake's own rows, switches and text, which GPUI already draws.

## 2. GNOME Shell Quick Settings: the VPN toggle

From gnome-shell `main` (`js/ui/status/network.js`). Quick Settings arrived in GNOME 43. The toggle's subtitle property has existed since 44 ([gjs.guide, GNOME 44](https://gjs.guide/extensions/upgrading/gnome-shell-44.html)).

**The toggle**
- A `QuickMenuToggle` titled "VPN". Its subtitle is the active connection's name, or "%d connected" when more than one is up. Its icon follows the state: `network-vpn-acquiring-symbolic` while connecting, `network-vpn-symbolic` when up, `network-vpn-disabled-symbolic` otherwise ([network.js](https://github.com/GNOME/gnome-shell/blob/main/js/ui/status/network.js)).
- Clicking the toggle's body switches the most recent connection. The arrow opens a menu headed "VPN" with one row per connection (name and a switch, at most 8 shown, most recent first) and "VPN Settings" at the foot ([network.js](https://github.com/GNOME/gnome-shell/blob/main/js/ui/status/network.js)).
- While a VPN is up, a VPN icon sits in the top bar's status area, bound to the toggle's checked state and icon ([network.js](https://github.com/GNOME/gnome-shell/blob/main/js/ui/status/network.js)).
- When a connection fails, the Shell sends one system notification: "Connection failed" / "Activation of network connection failed" ([network.js](https://github.com/GNOME/gnome-shell/blob/main/js/ui/status/network.js)).
- It lists only NetworkManager's VPN and WireGuard connections ([network.js](https://github.com/GNOME/gnome-shell/blob/main/js/ui/status/network.js)). Crake's tunnels are run by craked, not NetworkManager, so they never appear there *(inference from the code)*.

**Who can add to it**
- Quick Settings items come from the Shell and from extensions (`QuickToggle`, `QuickMenuToggle`, `SystemIndicator`); the guide documents it as the way "for extensions to add indicators, toggles and entry points" ([gjs.guide: Quick Settings](https://gjs.guide/extensions/topics/quick-settings.html)). No API lets an app add a toggle without an extension.
- The "Background Apps" section in Quick Settings (GNOME 44) lists apps through the background portal, which in practice means Flatpak apps ([Phoronix](https://www.phoronix.com/news/GNOME-Monitor-Background-Apps); [Ubuntu Discourse](https://discourse.ubuntu.com/t/flatpak-and-background-apps/71397)). Crake isn't a Flatpak and won't be (CLAUDE.md, Packaging).
- GNOME 49 moved Do Not Disturb into Quick Settings ([GNOME 49 release notes](https://release.gnome.org/49/)).

**For Crake:** three conventions to keep. (1) The two-tunnel phrase. GNOME says "2 connected"; the panel header's status line ("Listening · ACME, Nordbank", `model/mod.rs`) names them, which is better while there are two or three. Past three it should fall back to a count: "Listening · 4 tunnels" (new work, small). (2) The failure sentence. GNOME's "Activation of network connection failed" is what Crake must not sound like. Sign-in's "Couldn't reach Nordbank" with its three checks is the model, and the notification for a stopped tunnel should use the same words. (3) Two VPN switches on one desktop. A user who also has an NM VPN sees GNOME's "VPN" toggle beside Crake's bird, and the two don't know about each other. Crake shouldn't pretend to be that toggle. New tunnel's import path can mention it when the user picks a profile NetworkManager already has (new work, only if it comes up on a real machine).

**GPUI:** the panel's version of all this is GPUI. A Quick Settings tile is not possible for Crake without shipping a GNOME Shell extension, which CLAUDE.md already parks ("A GNOME Shell extension would be the most native option; not now").

## 3. GNOME Shell notifications

From gnome-shell `main` (`messageTray.js`, `notificationDaemon.js`, last changed June 2026) and the release notes for 48 and 49.

**What the code does** (this is the most consequential finding in this study)
- **A low-urgency notification never gets a banner.** `_onNotificationRequestBanner` returns at once for `Urgency.LOW`, so the notification goes silently into the list under the clock ([messageTray.js](https://github.com/GNOME/gnome-shell/blob/main/js/ui/messageTray.js)).
- **The app's timeout is ignored.** The daemon takes `expire_timeout` as an unused parameter (`timeout_`). Every banner except CRITICAL hides after a fixed `NOTIFICATION_TIMEOUT` of 4000 ms; only CRITICAL stays until dismissed and is shown expanded ([notificationDaemon.js](https://github.com/GNOME/gnome-shell/blob/main/js/ui/notificationDaemon.js); [messageTray.js](https://github.com/GNOME/gnome-shell/blob/main/js/ui/messageTray.js)).
- An action with the id `default` becomes the click on the body, not a button. Every other action becomes a button ([notificationDaemon.js](https://github.com/GNOME/gnome-shell/blob/main/js/ui/notificationDaemon.js)).
- GNOME 48 groups notifications from one app into a stack that expands ([GNOME 48 release notes](https://release.gnome.org/48/)). GNOME 49 put Do Not Disturb in Quick Settings ([GNOME 49](https://release.gnome.org/49/)).

**What the HIG says**
- At most three buttons. Use actions only when "often required". They must not duplicate the default action. Title in header capitals, body in sentence case. "Don't rely solely on notifications … they might not always be seen" ([HIG: notifications](https://developer.gnome.org/hig/patterns/feedback/notifications.html)).

**For Crake:** this changes what Settings → General promises. `notify::routed` sends `Urgency::Low` with an 8 s timeout. On GNOME that means **no banner at all**: "Sent git.acme.internal through ACME." waits silently in the list, and its Undo is two clicks away. The 8, 15 and 20 s timeouts, and the "until dismissed" of a stopped tunnel (`STOPPED_SHOWN = 0`, sent at Normal urgency), all come out as GNOME's 4 s. Choose deliberately:
- if "Quiet notification" means a short banner, send Normal (4 s on GNOME);
- if it means silent, keep Low and let the option's line say so ("It waits under the clock; nothing pops up");
- a tunnel that stopped by itself is the one case for CRITICAL, since it may need you and should stay.

Settings already says "Undo stays in the panel either way", and the HIG backs it: What Crake did is the real home of Undo. GNOME 48's stacking also argues for one notification per decision, never a summary that changes ("3 hosts routed"), because the Shell stacks them already. The "default" → Open Crake action is correct on GNOME.

**GPUI:** none of this is GPUI. It's the freedesktop notification API (notify-rust), and only the OS draws it.

## 4. AppIndicator and KStatusNotifierItem Support (v66)

`appindicatorsupport@rgcjonas.gmail.com`, maintained by Ubuntu, for GNOME Shell 45–51 ([metadata.json](https://github.com/ubuntu/gnome-shell-extension-appindicator/blob/master/metadata.json)). The click handling below has been the same since at least v53 (compared tag by tag).

**Clicks**
- **Left click on an item that has a menu and an `Activate` method:** the extension waits for the system's double-click time. If no second click comes, it **opens the menu**. On a double-click it calls `Activate(x, y)` instead ([indicatorStatusIcon.js](https://github.com/ubuntu/gnome-shell-extension-appindicator/blob/master/indicatorStatusIcon.js)). The README says the same: "Reveal indicator menus upon click", "Double clicking an icon will activate the application window" ([README](https://github.com/ubuntu/gnome-shell-extension-appindicator)).
- If `Activate` is missing (found by introspection, or by an UnknownMethod error), a left click opens the menu at once ([appIndicator.js](https://github.com/ubuntu/gnome-shell-extension-appindicator/blob/master/appIndicator.js)). ksni's `MENU_ON_ACTIVATE` uses exactly that error to make "menu on click" work ([ksni dbus_interface.rs, 0.3.7](https://github.com/iovxw/ksni/blob/master/src/dbus_interface.rs)).
- Right click toggles the menu. Middle click sends `SecondaryActivate` (or `XAyatanaSecondaryActivate`). Scrolling sends `Scroll`. A touch opens the menu ([indicatorStatusIcon.js](https://github.com/ubuntu/gnome-shell-extension-appindicator/blob/master/indicatorStatusIcon.js)).
- `x, y` are Clutter's `event.get_coords()`: the Shell stage's coordinates, which span every monitor in logical pixels. The extension's own comment says they "don't seem to have any effect" on most apps ([appIndicator.js](https://github.com/ubuntu/gnome-shell-extension-appindicator/blob/master/appIndicator.js)).

**Drawing**
- Icons are drawn at `Panel.PANEL_ICON_SIZE || 16`. The `icon-size` setting (default 0, meaning auto) applies only to legacy tray icons. Defaults: opacity 240 of 255, saturation, brightness and contrast 0, and compact mode on ([indicatorStatusIcon.js](https://github.com/ubuntu/gnome-shell-extension-appindicator/blob/master/indicatorStatusIcon.js); [gschema](https://github.com/ubuntu/gnome-shell-extension-appindicator/blob/master/schemas/org.gnome.shell.extensions.appindicator.gschema.xml)).
- **Tooltips aren't shown**: the README lists them under missing features ([README](https://github.com/ubuntu/gnome-shell-extension-appindicator)).
- Status `Passive` hides the item, and `NeedsAttention` swaps in `AttentionIcon*` ([appIndicator.js](https://github.com/ubuntu/gnome-shell-extension-appindicator/blob/master/appIndicator.js)).

**For Crake:** three consequences.
1. **Which click opens the panel.** Crake implements `activate()` and has a menu (`sys/linux/tray.rs`). By the extension's code, a single click opens the native menu after a pause, and only a double-click reaches `Activate(x, y)` and opens the panel at the bird. CLAUDE.md describes the panel opening at "the click". Confirm on the user's 26.04 machine which gesture does it, and write it down. If a single click should open the panel, the only route is the ItemIsMenu trick in reverse: no menu, with the fallback reached by right click. That isn't possible, because right click toggles the same menu. So the honest choice is between "single click: menu with Open Crake; double-click: the panel" and "no menu at all". This is a decision for the user, not a fix.
2. **The menu's first line is the tooltip on GNOME.** Since tooltips never show, the disabled first item ("Listening · ACME, Nordbank") is the only place the bird's state is put into words in the bar. Keep it first and keep it current.
3. **Opacity 240.** The bird reaches the bar at about 94 % ink. The dimmed "off" state (45 %) therefore shows at about 42 %, still clearly apart from 94 %. No change needed, but screenshots for the design should be taken through the extension, not from the SVGs.

**GPUI:** the menu is the OS's (DBusMenu, drawn by the Shell, unstyleable). The panel the activation opens is GPUI. Tray icon pixels are drawn by `tray/art.rs`, not GPUI.

## 5. Dash to Panel (the user's bar)

`dash-to-panel@jderose9.github.com`, GNOME Shell 46–51 ([metadata.json](https://github.com/home-sweet-gnome/dash-to-panel/blob/master/metadata.json)).

- It "moves the dash into the gnome main panel so that the application launchers and system tray are combined into a single panel" ([metadata.json](https://github.com/home-sweet-gnome/dash-to-panel/blob/master/metadata.json)).
- Defaults: position **BOTTOM**, size **48** px. Panels can be set per monitor (`panel-positions`, `panel-sizes`), including the left and right edges. Tray item padding, status icon padding and tray font size can be set (`-1`/`0` mean the defaults) ([gschema](https://github.com/home-sweet-gnome/dash-to-panel/blob/master/schemas/org.gnome.shell.extensions.dash-to-panel.gschema.xml)).
- AppIndicator icons stay 16 px in a 48 px bar; the extra height becomes padding *(observed)*.

**For Crake:** `place.rs` already opens the panel upward from a bottom bar. Two cases follow from Dash to Panel's settings and are worth a test each: a **vertical bar** on the left or right edge (the panel should open sideways from it, kept within the monitor), and **one bar per monitor** (the click's monitor decides). If `place.rs` covers neither, it is new work; add it to not-now.md if it isn't done. The 16-px bird in a 48-px bar is why the tray badge (14/32 of the icon) had to be big. Keep judging the bird at 16 px.

**GPUI:** placement is Crake's (XWayland plus the patched GPUI X11 backend). Nothing here needs the OS beyond the click position.

## 6. GNOME HIG: toasts, banners, placeholders, capitalisation

developer.gnome.org/hig as of October 2026.

- **Toasts** are for "individual events, as opposed to ongoing states". They are always transient and dismissible. Add a button only when "directly relevant", Undo being the example. Titles take the informal heading style ([HIG: toasts](https://developer.gnome.org/hig/patterns/feedback/toasts.html)).
- **Banners** "do not automatically hide", are for important ongoing states, and have one short title and an optional button. Examples: "Metered network ‒ automatic updates paused", "Working offline" ([HIG: banners](https://developer.gnome.org/hig/patterns/feedback/banners.html)).
- **Placeholders**: symbolic and neutral for places "usually full, currently empty", with a heading that states the empty state. Illustrated ones are only for an unavoidable first-run state ([HIG: placeholders](https://developer.gnome.org/hig/patterns/feedback/placeholders.html)).
- **Capitalisation**: header caps for headings, header bars, buttons and menu items; sentence caps for check boxes, field labels and the text in dialogs and notifications. An informal heading may be a full sentence in sentence case. Use an ellipsis when the action needs more input ("Delete…"). "Text generally shouldn't end with a period" ([HIG: writing style](https://developer.gnome.org/hig/guidelines/writing-style.html)).

**For Crake:** the shapes match Crake's own rules, and two differences are on purpose.
- *Banners = Crake's notices.* The leftover process, the new version and no tray are ongoing states with one action and "Not now". That is the HIG's banner, in DESIGN.md's notice shape.
- *Toasts = the in-window acknowledgements.* "Copied" after Copy for helpdesk, held 3 s, muted with the check, works in place, inside the row. Keep it in place rather than adding a bottom toast. The panel is too small for a toast to sit "never over a specific region".
- *Placeholders.* Log's "Nothing to tell today. No host needed a decision." and tunnel detail's "Nothing here." are symbolic placeholders in the HIG's sense. The second should state the empty state with the tunnel's name: "Nothing learned through ACME yet."
- *Deliberate differences.* Crake writes buttons and titles in sentence case ("Copy for helpdesk", "Send it direct instead") and ends its sentences with periods. DESIGN.md's copy voice overrides the HIG here. Header caps ("Copy for Helpdesk") would read as a different app. The ellipsis rule matches Crake's already ("Remove…", "Import .ovpn…", "New tunnel…").

**GPUI:** all of it is drawn in Crake's windows. libadwaita's `AdwToast`/`AdwBanner` aren't available, and aren't needed.

## 7. Circle apps and neighbours

### 7.1 Warp 1.0.0 (Circle, March 2026)

- The app page's screenshots form a state machine: "Main Window", "Transmit Code", "Accept File Transfer", "Receiving File", "File Transfer Complete" ([apps.gnome.org/Warp](https://apps.gnome.org/Warp/)).
- Each state is a status page with one job. The other machine is treated as a participant ("waiting for the other side"), and errors name the cause with a way to try again *(observed)*.

**For Crake:** the sign-in window is already Warp-shaped ("Waiting for you in Firefox", "Couldn't reach Nordbank" with Try again). Keep it so: one state per view, each with a defined next state, including the one where the browser tab was closed ("Open the page again").
**GPUI:** yes, Crake's own window.

### 7.2 Fragments 3.0.1 (Circle)

- 3.0: "Torrents are automatically restarted if an incorrect location has been fixed". "Stop downloads when a metered network gets detected". "Clipboard toast notification is no longer displayed multiple times" ([Felix Häcker, Fragments 3.0](https://blogs.gnome.org/haeckerfelix/2024/04/07/fragments-3-0/)). Overview "grouped by status" ([apps.gnome.org/Fragments](https://apps.gnome.org/Fragments/)).

**For Crake:** the lesson is that an app resumes by itself once the cause is gone. A tunnel in "trouble" because the network went away should come back when the network does, and say so in the log ("ACME is back. The network returned at 09:14."). Check whether craked already does this after a network change; if not, it is new work and belongs in not-now.md. The de-duplication lesson matches GNOME 48's stacks: never re-send the same notification for the same host.
**GPUI:** the words are GPUI; the behaviour is craked's.

### 7.3 Impression 3.8.0 (Circle, June 2026)

- Captions describe single-purpose screens, ending in a "Success screen with a big check mark" ([apps.gnome.org/Impression](https://apps.gnome.org/Impression/)). Destructive steps name what is lost before the button *(observed)*.

**For Crake:** New tunnel's three steps already follow it. The one thing to borrow is the consequence placed where the decision is made: under "Full tunnel" in step 3, "Everything goes through Globex while it's on. Crake won't learn here." Check whether step 3 says that today.
**GPUI:** yes.

### 7.4 Resources 51.0 (Incubator, September 2026) and Mission Center 1.2.0

- Resources 1.9.0: "Resources can now halt graphical updates when its window is not in view to save on power" ([releases](https://github.com/nokyan/resources/releases)). Resources is now the GNOME Incubator's system monitor ([apps.gnome.org/Resources](https://apps.gnome.org/Resources/)).
- Mission Center 1.2.0 overhauled its graphs: "smoother animations and rendering, a new loading shimmer while data loads" ([Flathub](https://flathub.org/apps/io.missioncenter.MissionCenter)).

**For Crake:** Resources' power rule applies to the panel's Last hour chart and the Traffic window. Neither should redraw while hidden or closed. CLAUDE.md says performance has never been measured; this is a cheap first check. Mission Center's shimmer is on the "Not for Crake" list.
**GPUI:** GPUI redraws only on notify; the rule is to not notify while hidden.

## 8. The Windows notification area (tray icon)

The Win32 UX guide's "Notification Area" page, written for Windows 7, which Microsoft says "still applies in principle" ([Microsoft Learn](https://learn.microsoft.com/en-us/windows/win32/uxguide/winenv-notification)). Plus Windows 11's taskbar settings.

**Clicks and menu**
- Left single-click: "Display whatever users most likely want to see", usually a flyout "near the associated icon". Prefer "a popup window over a dialog box". "Dismiss the popup window if the user clicks anywhere outside the window." Act on mouse up ([guide](https://learn.microsoft.com/en-us/windows/win32/uxguide/winenv-notification)).
- Double-click: the context menu's default command. Right click: the context menu, its default command in bold. Suggested order: "Open" (bold, first), suspend/resume with a check mark, then "Options" and "Exit". "Use check marks to indicate state" rather than changing labels ([guide](https://learn.microsoft.com/en-us/windows/win32/uxguide/winenv-notification)).
- A flyout in three parts: a summary (the same words as the tooltip), common tasks, and at most one link of each kind ([guide](https://learn.microsoft.com/en-us/windows/win32/uxguide/winenv-notification)).

**The icon**
- Provide 16, 20 and 24 px versions. "Prefer icons with unique outlines over square". Show status with variations or one overlay, "bottom-right for consistency". "Avoid swaths of pure red, yellow, and green" in the base icon. "Don't use long-running animations", "Don't flash the icon", though "Brief, subtle animations are acceptable" for important transitions ([guide](https://learn.microsoft.com/en-us/windows/win32/uxguide/winenv-notification)).
- Tooltip formats: "Feature - Status summary" or a status statement; don't explain how to click ([guide](https://learn.microsoft.com/en-us/windows/win32/uxguide/winenv-notification)).

**The overflow (Windows 11)**
- Settings → Personalization → Taskbar → "Other system tray icons" decides which app icons sit by the clock. The rest wait in the hidden-icons menu (^), and the user drags them between the two ([Microsoft Support: Customize the taskbar](https://support.microsoft.com/en-us/windows/experience/personalization/customize-the-taskbar-in-windows)). An app can't promote its own icon ([guide](https://learn.microsoft.com/en-us/windows/win32/uxguide/winenv-notification)). Users report that the choice can reset when an app updates ([Microsoft Q&A](https://learn.microsoft.com/en-us/answers/questions/5600219/other-system-tray-icons-constantly-having-to-activ)).

**For Crake:** Crake already matches most of this. The left click opens the panel at the icon (`sys/menu_tray.rs`, on mouse up). The tooltip is the summary. The menu uses check marks per tunnel. The badge sits bottom-right. The bird is white or dark with only the state marks in colour. The call animation is brief. Two gaps:
1. **The overflow.** On a fresh Windows install the bird lands under the ^. The first-run panel should say where it is, in Crake's voice: "The bird is under the ^ by the clock. Drag it next to the clock to keep it in sight." It is a neutral note, closed with Got it. That's new work, Windows only.
2. **The menu's order.** Windows puts Open first, in bold. Crake's fallback menu starts with the disabled summary, then the tunnels, then Open Crake. Keep the summary first (Section 4: on GNOME it stands in for the tooltip), but on Windows bold "Open Crake" if tray-icon allows it (it may not), and consider "Exit" for "Quit" on Windows only. Both are optional; ask the user.

**GPUI:** the panel is GPUI. The tooltip, the menu, the overflow and the icon slot belong to the OS (tray-icon crate). The first-run note is GPUI.

## 9. Windows 11 Quick Settings and the network flyout

Windows 11 23H2/24H2.

- **The VPN quick setting.** With one VPN, "you can turn it on and off by toggling the left side of the VPN quick setting". With several, choose "Manage VPN connections", then "Connect" ([Microsoft Support: Connect to a VPN](https://support.microsoft.com/en-us/windows/connect-to-a-vpn-in-windows-3d29aeb1-f497-f6b7-7633-115722c1009c)). The one-switch behaviour arrived in Canary build 26002, November 2023 ([Winaero](https://winaero.com/windows-11-build-26002-canary-adds-new-energy-saver-feature/)). In Settings → VPN the connection shows "Connected" under its name ([Microsoft Support](https://support.microsoft.com/en-us/windows/connect-to-a-vpn-in-windows-3d29aeb1-f497-f6b7-7633-115722c1009c)).
- **The shield.** "a small shield, will be overlayed in your system accent color over the active network connection" when "connected to a recognized VPN profile" (Insider build 25252, November 2022; [Windows Insider blog](https://blogs.windows.com/windows-insider/2022/11/28/announcing-windows-11-insider-preview-build-25252/)). Microsoft's support page now calls it "a blue shield" ([Microsoft Support](https://support.microsoft.com/en-us/windows/connect-to-a-vpn-in-windows-3d29aeb1-f497-f6b7-7633-115722c1009c)).
- **Wi-Fi's state words.** The network under the Wi-Fi arrow reads "Connected, secured", or "No internet, secured" when Windows' connectivity check fails ([How-To Geek](https://www.howtogeek.com/779034/how-to-connect-to-wi-fi-on-windows-11/); [PIA](https://www.privateinternetaccess.com/blog/no-internet-secured-issue/); secondary sources). The state comes first, then a qualifier, with a comma *(observed)*.

**For Crake:** Crake's tunnels are Wintun interfaces driven by openconnect, not Windows VPN profiles, so Windows will most likely show **no shield and no entry** in its VPN quick setting *(inference: "recognized VPN profile")*. Check this in the Windows 11 VM. If true, the bird is the only sign on Windows that a tunnel is up, which is one more reason the listening and asleep shapes must read at 16 px on a light taskbar. The state-word form ("Connected, secured") matches Crake's own row form ("Connected · 5 hosts"): state first, then a qualifier. Keep the middle dot; it is Crake's separator everywhere.

**GPUI:** Crake can't add itself to Windows' Quick Settings (no API; VPN profiles only). Its rows are GPUI.

## 10. Fluent flyouts and geometry

- A flyout is "a light dismiss container". It closes on a tap outside or Escape, and that first outside tap "is typically absorbed and not passed on to the UI underneath" ([Microsoft Learn: Flyouts](https://learn.microsoft.com/en-us/windows/apps/design/controls/dialogs-and-flyouts/flyouts)).
- Windows 11 rounds top-level windows, flyouts and dialogs at 8 px, controls at 4 px. Tooltips also use 4 px. Snapped or maximised windows get 0 ([Microsoft Learn: Geometry](https://learn.microsoft.com/en-us/windows/apps/design/signature-experiences/geometry)).

**For Crake:** the panel already closes on focus loss and Escape. On Windows, its corner is where Crake's 10 px panel radius meets the platform's 8. CLAUDE.md's rule ("Windows draws its own window frames, never Crake's rounding there") covers windows; check whether the panel itself is DWM-rounded (8) or drawn by Crake (10), and make it DWM's. Whether an outside click also reaches what's underneath is the OS's behaviour, not Crake's; don't try to change it.

**GPUI:** the panel's content is GPUI. The corner, shadow and light-dismiss focus behaviour come from the OS/DWM.

## 11. Windows app notifications (toasts)

Microsoft Learn's content reference (July 2026) and design basics (July 2025).

- Up to three text elements: the title takes up to 2 lines, and the other two share up to 4. "You can only have up to 5 buttons (including context menu items)". On Windows 11 the attribution area (the app's name and icon, close, and "…" with turn-off and settings) "is configured by the shell and can't be overridden". Scenarios: Reminder and Alarm stay on screen and need at least one button; Urgent breaks through focus ([App notification content](https://learn.microsoft.com/en-us/windows/apps/develop/notifications/app-notifications/app-notifications-content)).
- "Notifications should not be noisy". Suppress some straight into Notification Center. Respect Focus Sessions. A button that does background work shouldn't open the app. Clear notifications that no longer apply ([Notifications design basics](https://learn.microsoft.com/en-us/windows/apps/develop/notifications/app-notifications/app-notifications-ux-guidance)).
- notify-rust 4.18 on Windows: every action becomes a button, including `default`. Durations are Short, or Long from 25 s up. Critical becomes the Reminder scenario. Without an `app_id` the toast is sent as Windows PowerShell's (`Toast::POWERSHELL_APP_ID`) (notify-rust 4.18.2 `src/windows.rs`, in `~/.cargo/registry`).

**For Crake:** three findings from Crake's own code (`notify.rs`):
1. **Attribution.** `crakes()` sets no `app_id` on Windows, so a routed host's toast would be attributed to "Windows PowerShell". It should carry Crake's AppUserModelID, the one its Start menu shortcut is registered with. That's new work; check it in the VM before release.
2. **"Open Crake" shows as a second button** beside Undo on Windows, though clicking the toast body does the same. GNOME's HIG forbids duplicating the default, and Microsoft's 5-button budget doesn't need it. On Windows send Undo only, and let the body open the panel.
3. **"Until dismissed"** (`STOPPED_SHOWN = 0`) becomes Short on Windows. A tunnel that stopped by itself is the one case for the Reminder scenario (needs a button: "Connect again"). Routed hosts stay default and short.

Copy fits as is: title "Sent git.acme.internal through ACME.", body "It didn't answer directly. Through ACME it answered in 38 ms."

**GPUI:** OS only (WinRT toasts via notify-rust). Nothing to draw.

---

## Not for Crake

- **Pulsing or breathing state.** The Windows Wi-Fi icon's activity animation and Mission Center 1.2's "loading shimmer" ([Flathub](https://flathub.org/apps/io.missioncenter.MissionCenter)) are motion for something under way. DESIGN.md rejects the breathing dot. Under way is said in words, or with the dashed head.
- **A coloured overlay that means "tunnel on".** Windows' accent shield on the network icon makes colour mean "through a VPN". In Crake, teal means "Crake decided", and the bird changes shape, never colour.
- **Per-tunnel colours.** Resources and Mission Center colour each device's graph. DESIGN.md: "Tunnels are told apart by name, not colour."
- **Header capitals.** The HIG's "Copy for Helpdesk", and Mission Center's "Failed To Eject". Crake's voice is sentence case.
- **A bottom toast in the panel.** The HIG's toast suits windows, not a 10-px-radius panel under a tray icon. Acknowledgements stay in place ("Copied").
- **Icon buttons in notifications** (Fluent's 16-px icon buttons). These are marks that need a tooltip, which the user drops. Buttons name the action: "Undo".
- **An illustrated empty state.** The HIG's illustrated placeholder for a first run. Crake's first run already has the bird rising from the grass; it needs no second illustration.
- **A "summary" notification** ("3 hosts routed"). It changes under the user's eyes, and GNOME 48 stacks per-decision notifications already.
- **A dashboard-like flyout** (the Windows guide's summary / tasks / links frame filled with numbers). Crake's panel already has its summary (the header), its tasks (Tunnels) and one link (Full log). Don't add tiles. That is the rejected generic dashboard.

## Platform notes for docs/engineering/

Facts the engineering docs should record (platforms.md, or not-now.md where they mean work):

1. **GNOME banners ignore Low urgency and app timeouts** (gnome-shell `messageTray.js`, `notificationDaemon.js`, `main` June 2026). Low gives no banner, only the list. Normal and High give a 4000 ms banner. Critical stays until dismissed and opens expanded. `expire_timeout` is ignored. Crake's `ROUTED_SHOWN`, `ALSO_ANSWERED_SHOWN`, `ASKED_SHOWN` and `STOPPED_SHOWN` do nothing on GNOME. KDE and others honour them.
2. **The AppIndicator extension (v53–v66): a single left click opens the menu, after the double-click delay; a double-click calls `Activate(x, y)`.** Without `Activate`, a single click opens the menu at once. Right click opens the menu; middle click sends SecondaryActivate. Coordinates are Shell stage coordinates (logical, across all monitors). Tooltips are never shown. Icons are drawn at 16 px; `icon-size` affects only legacy tray icons. Default opacity is 240/255.
3. **Quick Settings can't host a third-party app's toggle** without a GNOME Shell extension (gjs.guide). The VPN toggle lists only NetworkManager VPN and WireGuard connections. GNOME's Background Apps list is fed by the background portal, in practice Flatpak only.
4. **Dash to Panel** defaults to a 48 px bar at the bottom. It can put a bar on every monitor and on the left or right edge (`panel-positions`). Placement needs cases for both.
5. **Windows toasts**: at most 5 buttons including context-menu items; up to 3 text elements (title 2 lines, 4 lines shared by the other two). Duration is Short or Long only (notify-rust: Long from 25 s). Reminder/Alarm stay on screen and need a button. The attribution is the shell's, and its name comes from the AppUserModelID. notify-rust falls back to PowerShell's when `app_id` isn't set. A `default` action becomes a visible button on Windows.
6. **The Windows notification area**: new icons go to the overflow (^). Only the user can promote one (Settings → Personalization → Taskbar → Other system tray icons), and the choice may reset after an app update. Windows' VPN shield and its VPN quick setting cover only Windows VPN profiles. Crake's Wintun tunnels likely get neither (to verify in the VM).
7. **Windows 11 geometry**: flyouts and windows are rounded at 8 px, controls at 4 px; snapped or maximised windows aren't rounded. If DWM draws the panel's corner, it is 8, not DESIGN.md's 10.
