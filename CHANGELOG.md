# Changelog

What's new in each Crake release, for the people using it. The newest is at the top.

<!-- How this file works (scripts/release-notes.sh, .github/workflows/release.yml):
     changes worth telling users go under "Unreleased" as they're made. To
     release, that heading becomes "## <version> (<YYYY-MM-DD>)", the version
     goes into Cargo.toml, packaging/nix/package.nix and packaging/arch/PKGBUILD,
     and the tag v<version> is pushed: the release workflow refuses a tag whose
     section is missing, and that section is the GitHub release's text. -->

## Unreleased

## 0.15.2 (2026-10-26)

### Fixed

- **Websites open without a pause on a Mac or Windows PC.** While a tunnel was up, every new
  website name could take three seconds when the network's first DNS server doesn't answer and
  its second does: Crake asked the silent one first every time and waited it out. Now it asks
  the one that answered last first, and the next one too after a moment's quiet.

## 0.15.1 (2026-10-25)

### Better

- **On a Mac, the bird in the menu bar is the size of its neighbours.** It was the largest icon
  there, a little soft and sitting low: drawn for Linux's small bars and stretched. Now it's drawn
  for the Mac's own bar, as tall as the icons beside it and on their line, with a rounder eye, and
  asleep it's dimmed less, so it doesn't look disabled.

### Fixed

- **The call marks read on a light bar.** When Crake routes a host, the two marks in front of the
  bird's bill were the dark theme's teal, faint on Windows' light taskbar or a light Mac menu bar.
  They're the light theme's teal there now.

## 0.15.0 (2026-10-25)

### New

- **What's new, after an update.** The first time you open the panel after Crake is updated, it
  says so, with how many changes there are since the version you had, and What's new shows them
  in these words. Settings → About has it too, for any time.

### Better

- **What scrolls says so.** The log, Settings, tunnel detail, Traffic, New tunnel and its lists,
  the sign-in window, What's new and the panel show a scrollbar while you point at them, when
  there's more than fits. It fades away when you move on.
- **Switched off and on, an OpenVPN tunnel doesn't send you to the browser again.** After you sign
  in in the browser, Crake keeps the gateway's sign-in, as it does AnyConnect's, until it runs out
  or the computer restarts. It has done so since 0.11; this is the first time it's said.
- **"Crake X is out" downloads the installer itself on Windows and Mac.** Its Download button
  opened the release's page, with a file for every system; it now fetches the one for your computer
  (the ARM installer on Windows on ARM).

### Fixed

- **Your log is yours alone.** Crake's log (which hosts you reached, and when), the hosts it learned
  and its traffic counts could be read by every user of the computer, from its files and from
  Crake's service, and its decisions were announced to all of them. Now the files are readable by
  Crake's service alone, it answers only the person at the computer, and tells only them. Settings
  said the log was in your home folder; it says where it really is now.
- **An OpenVPN tunnel switched on again, or kept through an update, asks for your sign-in less
  often.** The gateway can turn the kept sign-in away by mistake when the new connection happens
  to come from the same port as one that just ended. Crake tries once more before asking you.
- **A company site with a public and a private address stays inside, over IPv6 too.** When your
  company's DNS gives a public name a private address, Crake sends it through the tunnel; the
  name's public IPv6 address was still handed out, and an app that tries IPv6 first went to the
  public site, which turns outsiders away. Now only the private address is given.
- **The first connection to a host Crake just learned is no longer cut off.** If the app tried
  again at the very moment Crake routed the host, that try could leave the tunnel with your
  network's own address; where the gateway could answer that address (on the office network), the
  page started loading and was then reset. Nothing leaves a tunnel with another address now, so
  the app's next try goes through.
- **Installing with Homebrew no longer prints a warning.** "Calling string comparison format for
  `depends_on macos:` is deprecated" came from Crake's cask, not from your Mac.
- **Opening an IPsec certificate file is safer.** ctutils, which Crake uses when it opens a
  `.p12` certificate with its password, is updated to 0.4.5: before, its comparisons could take
  slightly different times depending on the secret (RUSTSEC-2026-0385). It all happens on your
  own computer, so the risk was small.

## 0.14.0 (2026-10-21)

### New

- **IPsec gateways that prove themselves with a certificate.** New tunnel asks how the gateway
  proves itself: with a key from your IT team, or with its certificate, as a website does. Crake
  checks that the certificate is for the gateway, signed by an authority this computer knows or by
  the one your IT team gave. When no authority here vouches for it, Crake shows you whose it is and
  who signed it, and asks before trusting it, as it does for AnyConnect gateways. If it changes
  later, Crake asks again and says so. A certificate made out to another name is never offered to
  be trusted.
- **Sign in to an IPsec gateway with your own certificate.** New tunnel's step 2 offers "Certificate"
  and "Certificate and password" beside "Username and password": choose the .p12, .pfx or .pem file
  your IT team gave you, and Crake keeps a copy in your keyring. Its password is asked as "Unlock your
  certificate", and remembered when you tick it. A certificate that has run out is said before
  anything is sent. A certificate the gateway turns down is said as the certificate, not the password.

### Fixed

- **"Remember the password in your keyring" for a certificate's file is kept.** Ticked when OpenVPN
  asked for a profile key's password, it was shown but forgotten.
- **No AnyConnect groups for an IPsec tunnel.** Choosing IPsec at a gateway that also answered as
  AnyConnect showed AnyConnect's sign-in groups, which IPsec doesn't use.
- **A failed sign-in names who said why.** When an IPsec or OpenVPN tunnel couldn't sign in, the
  sign-in window put strongSwan's or OpenVPN's words after "openconnect said:". It now names them.

## 0.13.1 (2026-10-21)

### Fixed

- **Every network an IPsec gateway offers shows in tunnel detail.** A gateway offering two networks
  showed only the first under "From the gateway", and only that one was raced on the first
  connection. Both are now.

## 0.13.0 (2026-10-20)

### New

- **IPsec on NixOS.** `services.crake.ipsec = true;` brings strongSwan and runs Crake's own charon
  for IPsec tunnels, as the other distributions' packages do. New tunnel says that line on NixOS
  instead of an apt command.

- **Crake for Windows on ARM (preview).** Windows on ARM can't load the drivers made for Intel and
  AMD processors, so it gets its own installer, `crake-setup-<version>-arm64.exe`, with openconnect,
  OpenVPN and their drivers built for ARM. Each installer tells you if you picked the other one.

- **Your IT team can set up IPsec tunnels.** A file in Crake's managed folder can now describe an
  IPsec (IKEv2) tunnel: the gateway, its identity, the pre-shared key in a file of its own (kept
  root's alone; a key anyone on the computer can read is refused), the company's domains and what
  may go through. You switch it on and sign in with your password, as with your own. craked.toml
  takes the same.

### Better

- **"Change the key…" when the gateway turns the key down.** An IPsec sign-in refused for its key
  now ends with a button that opens the tunnel's Edit, instead of Try again, which couldn't help.

### Fixed

- **A certificate check fixed in a library Crake carries.** rustls-webpki, part of the window
  toolkit's web client, could accept a certificate chain through intermediates that are no longer
  trusted (RUSTSEC-2026-0382); updated to 0.103.16. Crake itself doesn't fetch anything through it
  (its update check uses your system's curl), and your tunnels' TLS is openconnect's and OpenVPN's.

- **Signing in through the browser works for AnyConnect and other openconnect gateways.** Crake
  handed openconnect the browser's name without its folder, and openconnect doesn't look for it, so
  the browser never opened on Linux or macOS and the sign-in waited forever. It's found where your
  system keeps it now. (Found by a new test that plays a Cisco gateway's browser sign-in.)

- **IPsec tunnels come back after an update.** Updating Crake switched IPsec tunnels off. Now they
  switch on again by themselves, and if you keep the password in your keyring they sign in without
  asking.

- **Names kept private once another VPN client lets them go.** When another VPN program had been
  serving a company's names and stopped, Crake took them back to keep them private, but if the
  system's resolver wasn't ready that moment, it never tried again, and those names could go to
  the internet's DNS. It tries again two seconds later now.

## 0.12.0 (2026-10-20)

### New

- **IPsec (IKEv2) tunnels, on Linux (Beta).** New tunnel has an IPsec card for FortiGate's "IPsec
  VPN" and other IKEv2 gateways: the gateway, the key your IT team gave you and, if they named them,
  the company's domains; then your username. Crake runs strongSwan's charon itself and asks your
  password when you switch the tunnel on, keeping it in your keyring if you tick Remember. As
  always, only the hosts that need the tunnel go through it. Needs strongSwan (New tunnel says the
  command); Crake's runs beside a strongSwan of your own and leaves it alone. A wrong key is said as
  such: "The key doesn't match". On Fedora, a small SELinux module of Crake's lets strongSwan
  read Crake's settings.

- **You choose whether a password is kept.** Every password the sign-in window asks for has
  "Remember the password in your keyring", ticked to start with; before, a gateway's password was
  kept without a word, and only a certificate's had the box. One-time codes are never kept.

### Better

- **Updating Crake on Windows keeps your tunnels signed in, from this version on.** Installing a
  newer Crake stopped its service the plain way, which signed every tunnel out; you had to sign in
  again. From now on the old service hands the tunnels to the new one, as the Linux and macOS
  packages do: they come back on their own, with what they had learned. The update to this
  version still signs you out once, since it's the older service that stops.

- **Crake's DNS rules come back on Windows and macOS.** If another program removed the rule that
  sends a company's names to its tunnel (Windows' name resolution policy, or macOS' /etc/resolver
  file), those names went to the network's DNS until the tunnel reconnected. Crake now notices
  within moments and puts its rule back, as it already did on Linux.

- **Browser sign-in on Windows says why it isn't there.** The openconnect Crake brings on Windows
  can't sign in through the browser. New tunnel said it "needs openconnect 9 or newer; this computer
  has 9.12", and a tunnel set to it failed with openconnect's own words. Both now say it isn't built
  into the openconnect Crake uses there, and to use a username and password or a certificate.

- **Crake's notifications carry the bird.** They name Crake's icon and its desktop entry, so the
  desktop shows them with Crake's icon and files them under Crake, as other apps' are.

- **The seconds count where you'd expect.** "Asking the gateway…" and "Saving the report…" stay
  quiet for two seconds, then count from 3 s, the second you're in, not a 2 you've just sat through.
  The bird beside "Asking the gateway…" is drawn in the text's colour now: teal is kept for what
  Crake decided.

### Changed

- **Teal means one thing again.** Small actions and links (Import .ovpn…, Edit…, Copy for helpdesk,
  About's links) are ink and underlined, as Diagnostics' Open already was; the update notice's
  Restart and Download are buttons, as other notices' are. Teal is left to what Crake decided, what's
  chosen or on, and the one main button. Copy for helpdesk now says "Copied".

### Fixed

- **Your IT team's tunnels on Windows and macOS.** Tunnels your IT team puts in Crake's managed
  folder (ProgramData\Crake\managed.d on Windows) were read only when a craked.toml was there too,
  which Windows' and macOS' installers don't make, so they never showed up. They do now.

- **Names only your company knows work on Windows with IPv6 about.** When a company's DNS
  answered a name's IPv4 address but refused its IPv6 one, Crake passed on the internet's "no such
  name" for the IPv6 question, and Windows then forgot the IPv4 answer too, so the name didn't
  resolve. Crake now says the name exists without an IPv6 address.

- **Moving a tunnel to TCP is tried twice.** On a network that blocks UDP, Crake moves the tunnel
  over to TCP on the same sign-in. If the gateway wasn't done with the old connection yet, the move
  failed and the tunnel said it stopped unexpectedly. Crake now tries it once more, a moment later.

- **A host another tunnel learned can go through this one.** Once a tunnel had learned a host, no
  other tunnel could learn it, even with that tunnel switched off (or gone, when your IT team removed
  theirs), so the host went nowhere. Now only a tunnel that's up keeps a host to itself, and switching
  one off lets the others reach what it carried straight away.

- **A tunnel whose DNS stops answering hands over cleanly.** When a company's DNS goes quiet, Crake
  answers the names it has seen with their last address. For a moment at the switch, a name asked
  could go to the internet's DNS instead (against "Keep internal names private"), or wait on an
  answerer that wasn't listening yet. Crake's answerer is now listening before it takes the names
  over. And if Crake's service ended during such an outage (a crash, a reboot), the company's names
  kept failing once it was back, until the next outage; now they work again as soon as it starts.

- **On a network that blocks UDP, a tunnel isn't called connected too soon.** When AnyConnect's UDP
  channel can't get through, Crake moves the tunnel over to TCP on the same sign-in. For about three
  seconds of that, the panel said Connected while the tunnel was still being moved, and internal
  names looked up then failed. It now says Connecting until the tunnel over TCP is up, and the log
  says the tunnel is up rather than "back".

- **Everything stays on a small screen.** On a 1366×768 laptop, Settings ran off the bottom of the
  screen, the panel's footer went missing once there were four tunnels, and a gateway's long list
  of groups pushed Cancel out of the sign-in window. Each now fits its screen and scrolls what
  doesn't fit, with its buttons kept in view; New tunnel's dropdowns scroll too.

### Underneath

- **Windows drawn with the newest GPUI.** The toolkit Crake draws its windows with is updated
  (0.7.3), with Crake's own fix for placing the panel at the tray carried over. It needs about
  thirty fewer libraries than before.

## 0.11.1 (2026-10-19)

### Better

- **OpenVPN is no longer Beta.** Crake's own OpenVPN has signed in to a real company gateway
  through the browser and reached its internal network, so New tunnel shows it beside AnyConnect,
  without the Beta tag.

- **"Saved …" goes when you move on.** The line under "Save a report…" stays while you're in
  Diagnostics, to open the file or show it in its folder, and goes when you choose another section;
  a save still under way waits for you to see how it went. It also says when names and addresses
  were left out. Close Settings while a report is saving and a notification says how it went.

### Fixed

- **"Hide names and addresses when sharing" hides them all.** A report could still name a host
  whose company domain ends in a digit, or one written with its port, after "host=", in a list, in a
  range of addresses, as a Mac shortens a network, or as one word where your DNS settings list
  domains; and OpenVPN's gateway address in Crake's service log. Now they're "a host" too, and your
  tunnels' names stay, so a report still says whose each line is. Whatever the setting, e-mail
  addresses, more sign-in cookies and file paths wherever they stand (a Windows path with a space
  in it gave away a surname) are left out.

### Underneath

- **The libraries Crake is built on are up to date.** tokio 1.53.3 marked the way Crake's service
  watched its own kernel connections as unsafe; it now uses tokio's safe replacement. Also
  serde_json 1.0.154, base64 0.23, resvg 0.48 and, on a Mac, system-configuration 0.8, with which a
  busy Mac refusing Crake a look at its network settings is asked again rather than caught as a
  crash.

## 0.11.0 (2026-10-19)

### New

- **Crake runs OpenVPN profiles itself.** Import the .ovpn file your IT team gave you (New
  tunnel, beside Gateway): Crake picks OpenVPN, takes the gateway and a name from the file, and
  keeps its own copy, which only its service can read. Signing in through your browser, with a
  password, a code or a certificate's password works as the profile says. A profile that would run
  something on your computer is refused, saying why. Beta until it has met a real gateway. The
  packages now recommend openvpn.

- **IT can set up OpenVPN tunnels too.** A managed profile in `/etc/crake/managed.d` can name a
  `.ovpn` file beside it (`protocol = "openvpn"`, `profile = "initech.ovpn"`), with the same
  learning policy as AnyConnect's; Crake checks the file as it does one you import, and says why
  if it can't run it.

- **OpenVPN on Windows and macOS too.** On Windows it comes with Crake, with its drivers; on a Mac
  it comes with Homebrew's cask (`brew install openvpn` otherwise). On Windows, too, a company's
  internal names go to its DNS and are routed before the first packet.

- **A report for whoever is helping you.** When something isn't working, Settings → Diagnostics →
  "Save a report…" saves one text file with what they need to see: how this computer connects to
  networks and what Crake added, whether the internet and each tunnel's names answer right then,
  the last day of Crake's decisions and its service's own log. Passwords, sign-ins, keys, profiles
  and your devices' hardware addresses are never in it; with "Hide names and addresses when
  sharing" on (Settings → Privacy & log; it was "Hide host names in helpdesk copies"), the places
  you connected to are left out too. Crake sends it nowhere: Open it, read it, then send it
  yourself. From a terminal, `crake report` saves the same file. The log's Export, which saved its
  rows as a file, is gone: Copy for helpdesk still copies them.

### Better

- **The tray's badge reads now.** When a tunnel stops or Crake needs you, the badge sits at the
  bird's bottom right, larger, with a "!" drawn on whole pixels, so it reads at a bar's small size.
  On a light taskbar or menu bar it's a deeper red or amber, so it stands out there too.

- **OpenVPN remembers your sign-in.** After a username and password that worked, Crake keeps them
  in your keyring, as it does for AnyConnect, and doesn't ask again. If the gateway refuses them,
  Crake forgets them and asks.

- **Public sites a tunnel took by mistake go direct again on Windows and macOS too.** Once per
  version, Crake tries each public address a tunnel learned and forgets those that answer
  directly, as it already did on Linux.

### Changed

- **Your OpenVPN 3 tunnels move to it.** Each one is copied over once, while it's off, and the log
  says so; OpenVPN 3 keeps its own profile. One whose profile OpenVPN 3 keeps locked stays where it
  is, and the panel asks for its .ovpn file.

### Fixed

- **On Windows, what Crake keeps is yours alone.** Its log and the hosts it learned for each tunnel
  were readable by every user of the PC; now only Windows itself and administrators can read them.

- **Another VPN client's names keep working on Windows.** With "Keep internal names private" on,
  Crake no longer holds back names that another VPN client on the PC serves (AnyConnect,
  GlobalProtect and the like), as it already didn't on Linux and macOS.

- **An internal site right after connecting works on networks that block UDP.** When a tunnel
  restarted over TCP in its first seconds, a site opened in that moment could be left direct for ten
  minutes ("No answer just then"); Crake now tries it again on the next attempt. The same when a
  tunnel comes up after a site found no answer: it's tried through it at once.

- **On a Mac, switching a tunnel off undoes all of it.** Crake turns on the Mac's packet filter
  while a tunnel is up, for connections that started before a host was routed. Switching the
  tunnel off, or quitting Crake, could leave the filter on, with Crake's rule for that tunnel
  still in it. Now both go. If Crake's service ever stops without tidying up, it does so the next
  time it starts.

## 0.10.4 (2026-10-17)

### Fixed

- **No more large pointer for a moment.** Coming onto a Crake window, the pointer could blink
  large for an instant: GNOME had given XWayland's background a pointer at an older scale's size,
  which shows before the window's own. Crake now gives it your theme's pointer at today's size,
  and keeps it so when the scale or the pointer size changes.

## 0.10.3 (2026-10-17)

### Fixed

- **The panel stays above the bar.** With long names in "What Crake did" (an Azure cluster's
  private-link name, say), the panel opened from a bottom bar could grow down under it, hiding the
  last tunnel and the buttons. It grows upward again, and long names break after a dot instead of
  in the middle of a word, in the panel and the log.

- **The pointer is its right size from the start.** Over Crake's windows the arrow could be too
  big until the pointer had been over a button once. Each window now has its own arrow from the
  moment it opens.

## 0.10.2 (2026-10-17)

### Fixed

- **"Keep internal names private" says when it can't.** If another DNS server on your computer
  holds the port Crake answers on, or Crake can't add its private DNS link, Settings → Names now
  says so under the setting, and Crake keeps trying. Before, it failed quietly, and with such a
  server running the names could reach it and go out to your provider's DNS.

## 0.10.1 (2026-10-17)

### Changed

- **The app icon is the bird in one ink.** The corncrake's colours are gone from the icon: it's
  the same light bird as in the panel and the bar, calling in teal, on the dark tile.

### Fixed

- **The pointer keeps its size.** After the display's scale changed while Crake ran (a laptop
  docked to a large monitor, or undocked), pointing at a button or a field in Crake's windows
  showed a pointer twice (or half) its size. It follows the scale now, and the cursor size set in
  Accessibility too.

- **Traffic's tick stays on a host you pinned.** A host Crake sent through a tunnel lost its
  two-stroke tick in Traffic once you pinned it; it keeps it now. Hosts you added yourself have none.

- **Traffic's ticks have room.** The line to a destination Crake learned stopped under its
  two-stroke tick, and packets ran beneath it; it now ends before the tick.

- **The bird in the bar is smooth again at its smallest.** 0.10.0 drew it pixel by pixel at 16 px,
  and next to the bar's other icons it looked pixelated; it's drawn with soft edges, as at every
  other size, with the cut of its wing back.

## 0.10.0 (2026-10-17)

### New

- **Crake says so when your desktop shows no tray icons.** Stock GNOME on Fedora and Debian
  shows none without its AppIndicator extension, and Crake used to start with nothing on screen.
  Now its panel opens with a note when you start it, and at login it sends one notification. Where
  it can, Crake switches the extension on in one click, or gives you the command to install it.
  Settings' bird row says the same.

### Better

- **Sign-in asks in words.** "ACME asks for your group", "…for a code", "…for your username",
  with the gateway's own words quoted under it, instead of "ACME asks: GROUP". A field Crake
  doesn't know keeps the gateway's word.

- **Small things in the panel and New tunnel.** A notice's "Got it" is the same bordered button
  everywhere; the dot next to "Trouble" is red, as the bird's badge is; "Found a Cisco AnyConnect
  gateway" wears a plain check (the two-stroke tick is for hosts Crake routed); the note under
  "Stay signed in" about your browser is gone (the sign-in card says it); a number and its unit
  ("38 ms") no longer part at a line's end; the Traffic window's margins match the other windows'.
  The amber of "needs you" is a touch deeper in the light theme, so it reads against the panel.

- **Tunnel detail reads as its screen.** Pin and Forget are quiet bordered buttons again (they had
  become bare bold words), "last used" lines up on the right, a pinned host shows nothing there
  (its line says it's kept), and the title row says "Switch off", the words used everywhere else.

- **The log's empty day says it once.** "Nothing to tell today. No host needed a decision." under
  the day's chart, instead of a summary of zeros and "Nothing matches today"; a host left alone has
  no badge any more; the "Scroll to zoom · drag to move" hint shows only while you point at the
  chart.

- **Settings is quieter.** "Remove…" and "Clear log…" are plain buttons until you point at them
  (red is for something that stopped, or for the question "Remove ACME?" itself); the "?" marks
  stay only where their card says more than the line under the setting; the row that only means
  something when Crake starts at login is dimmed less, so it can still be read; and Privacy's
  statement sits on a plain surface.

### Changed

- **Traffic shows its decisions, not its tunnels, in teal.** The packets moving through the graph
  are grey on every way now; the two-stroke tick marks each destination Crake learned, in the
  graph and in the table's "Way" column (it was "Goes"), the same tick as under the chart. Long
  names aren't cut short any more.

- **A new bird.** The one in the bar was a plain head; now it's a crake with its head raised,
  calling: a short bill parted at the tip, a square eye, the folded wing one cut across the
  breast. It opens its bill when it calls (a host sent through a tunnel), and drops its head to
  sleep. The same bird is in the panel, in Settings and on the app icon, where it wears the
  corncrake's own colours.

### Fixed

- **Settings no longer says the bird is in the "top bar".** It's "the tray" on Linux and Windows
  and "the menu bar" on a Mac, with the bird itself next to the words, drawn the way that bar shows
  it. Its "?" now says what each look of the bird means: asleep, dashed, awake, two teal marks, an
  amber triangle, a red badge.

## 0.9.0 (2026-10-16)

- **A host that didn't answer once is no longer called "probably down".** The log now says what
  happened: "No answer just then, directly or through ACME. Nothing was changed."

- **Crake can be used from the keyboard.** Tab and Shift+Tab move through the panel, tunnel detail,
  Settings, the log, Traffic and the editor; Enter or Space presses what's in focus, and a ring
  shows where you are.

- **Public sites an older Crake sent through a company by mistake go direct again.** Before 0.8.0,
  one lost connection on a shaky network could send a public site through a tunnel for good (it's
  kept while it's used). Once after an update, Crake tries each such site directly and forgets the
  ones that answer, and says so in the log. Linux for now.

- **Switching an OpenVPN 3 tunnel off no longer says it "stopped unexpectedly".** Crake ended the
  session before telling its service, which sometimes noticed the tunnel's device gone first and
  called it ended by OpenVPN 3. The service is told first now, and a device missing for a moment
  isn't taken for a stop.

- **An edited OpenVPN 3 profile is never thrown away by one key.** With changes not saved, Escape,
  the × or Cancel ask first ("Leave without saving?"), and Keep editing is the default.
- **New tunnel gets out of the way once the gateway has answered.** The protocol list folds into
  "Choose the protocol yourself", so the next field is in view; opened, the protocols sit under
  "Used with real gateways" and "Beta" instead of a badge on each.
- **Traffic's numbers agree with their order.** The busiest places and connections show the rate
  they're ordered by (the last twenty seconds or so), so a row never shows more than the one above.
- **Traffic keeps teal for Crake's decisions.** The last five minutes are drawn in greys, with a
  teal tick where Crake sent a host through a tunnel; every way out has the same grey outline.
- **Quieter hosts in tunnel detail.** Pin and Forget no longer look like a wall of buttons; they fill
  in when pointed at, and a pinned host counts once, under Pinned.
- **The panel's bird says what the bird in the bar says**, with its amber triangle or red badge, and
  the leftover OpenVPN 3 notice has a Copy button inside the command, a Got it button, and says it
  goes away by itself.

- **A process OpenVPN 3 left running is shown in the panel, with the command that stops it.** The
  notice is tinted amber and has Copy and Got it beside the command; it goes by itself once the
  process has ended. The bird in the bar wears a small amber triangle meanwhile (a red badge, for a
  tunnel that stopped, takes its place). Crake doesn't end another program's process itself.

## 0.8.3 (2026-10-15)

- **Traffic names things better and stays still when nothing flows.** Many names can share one
  address (a company's DNS often gives every name under a domain the same one); a connection now
  keeps the name it was first seen with instead of the latest one looked up. A tunnel carrying only
  its keepalives no longer shows packets moving while it says 0 KB/s.

## 0.8.2 (2026-10-15)

- **"Crake was updated" now appears after a package update.** The app kept running against the new
  service without noticing it had changed, so the panel never said to restart. Now it notices within
  a couple of seconds and offers Restart.
- **Firefox no longer hangs when another OpenVPN 3 session reconnects.** OpenVPN 3 sends every name
  to a tunnel's company for a moment whenever any of its sessions reconnects; Crake used to put
  things back within two seconds, long enough for Firefox's proxy auto-detect to ask in that moment
  and hang. Now it's put back at once.
- **Crake tells you when OpenVPN 3 leaves a connection running.** OpenVPN 3 can lose track of a
  session's process: the session ends, the process doesn't, and it keeps reconnecting on its own
  for days, changing your other tunnels' DNS each time. When Crake sees that happen to one of your
  sessions, it says so in the log and a notification, with the command that stops it.
- **Crake no longer leaves an OpenVPN 3 session it can't follow.** A session Crake started is ended
  if anything goes wrong before it's connected, and Crake doesn't tell a session to connect when
  something else already has (two at once were how OpenVPN 3 lost those processes).
- **Traffic is calmer.** The rates follow the traffic over a few seconds instead of jumping every
  second, the busiest places and the connections keep their places unless the traffic really
  shifts, and packets are drawn above every path instead of disappearing under the one crossing it.

## 0.8.1 (2026-10-15)

- **OpenVPN 3 tunnels work again.** 0.8.0 lost an OpenVPN 3 tunnel's routing a second after it came
  up and left it "connecting": a sign-in through the browser looked stuck. Fixed: while the
  session's device is there, the tunnel stays up, and a session that reconnects is handed over
  again.
- **An update keeps an OpenVPN 3 tunnel's names.** After an update, an OpenVPN 3 tunnel could come
  back with none of its domains, and OpenVPN 3 could then send every name on the computer to the
  company's DNS (Firefox hung looking for a proxy). The tunnel's DNS and domains now go along with
  the handover.

## 0.8.0 (2026-10-15)

- **Traffic, live.** A new window, from the panel's Traffic button, shows what goes where right
  now: this computer, each tunnel that's up and direct, the busiest places each one reaches, and
  packets ticking along every path as they go. Under it, the last five minutes, the connections
  open right now and every rule Crake has. On Windows only TCP connections are listed (Windows
  doesn't say where UDP goes); a connection's bytes count from when the window first looked. It's
  only asked for while the window is open, and stays on this computer like everything Crake sees.
- **Edit an OpenVPN 3 profile in Crake.** An OpenVPN 3 tunnel's details have "Edit…" next to its
  profile: its text opens in a window, certificates and keys folded away and kept as they are.
  "Save and reconnect" gives OpenVPN 3 the new text and reconnects the tunnel; if OpenVPN 3
  refuses it, it says why and nothing changes. For people who tune their own profiles.
- **"Is it safe?"** Settings → About answers it in four lines: nothing leaves your computer, what
  happens if Crake stops, what your company sees, and who can change what.
- **Hover.** A window's × turns red while you point at it, and Settings' sections light up softly
  under the pointer.
- **Whole names.** A long host name cut short with "…" in a tunnel's details or the Traffic window
  shows whole when you point at it, and a click copies it.
- **Dismiss a stopped tunnel.** A tunnel that stopped on its own has a Dismiss button next to why in
  the panel: it goes back to plain "Off" and the red badge leaves the bird. Why it stopped is still
  in its details and the log.
- **The bird says what's up.** With a tunnel stopped and nothing else connected, the bird sleeps,
  with the red badge; it no longer looks awake as if something were connected. While another tunnel
  connects, it's drawn dashed with the badge.
- **No company detours for public sites.** One connection to a public site that got no answer (a
  lost packet on Wi-Fi), or a minute when the network itself had trouble, could send that site
  through a company's tunnel for a week, and break it when that tunnel's session ended. Crake now
  tries a public site directly once more before sending it through a tunnel, and leaves public
  sites alone while several places stop answering at once.
- **OpenVPN 3 profiles found the first time.** Switching on an OpenVPN 3 tunnel sometimes said its
  profile wasn't there, and "Try again" worked: openvpn3 had only just started. Crake now gives it
  a moment.
- **OpenVPN 3 tunnels survive a reconnect.** When openvpn3 sets a session up again (the network
  dropped for a moment, the computer woke up), Crake says the tunnel is reconnecting and routes
  through it again once it's back. It used to call the tunnel stopped while the session carried on.

## 0.7.0 (2026-10-14)

- **Crake says when it was updated and needs a restart:** after a package update the panel says
  "Crake was updated to X. Restart it to finish.", and Restart does it with your tunnels kept up.
- **Crake tells you when a new version is out,** where nothing else updates it (a Mac without
  Homebrew, Windows, a .deb or .rpm installed from a file): once a day it asks dorhawk.software for
  the newest version's number and the panel links its download page. Settings → General switches
  it off.
- **The panel no longer blinks and jumps when it changes size,** e.g. after Undo or Redo with a
  bar at the bottom of the screen: it grows and shrinks in place instead of opening again.

- **The sign-in window goes behind other windows** when you click elsewhere, like any window. It
  stayed above everything, even over the browser page of a web sign-in.

- **Forget all in a tunnel's detail.** One button empties what Crake learned for a tunnel, and
  what you sent direct with Undo, so it learns again from scratch as you use things. It asks first;
  pinned hosts stay.

- **On a Mac, System Settings lists Crake by its name.** Its service and "Start when you log in"
  are now registered by Crake itself, so Login Items & Extensions shows them as Crake, not under
  the name on its developer certificate. macOS asks you to switch the service on once: Crake opens
  after installing, and its panel says so, with a button to the right page. Nothing is routed
  until then.

## 0.6.1 (2026-10-14)

- **Crake no longer gets in the way of another VPN client signed in to the same company.** With
  "Keep internal names private" on, Crake refused a company's names while its own tunnel was off,
  even when AnyConnect or openconnect was connected there: every internal name failed (seen on a
  Mac). Names another VPN client serves are now left to it, and kept private again once it
  disconnects. On a Mac, Crake's service also no longer stops when macOS briefly refuses it its
  network settings.

- **Crake keeps a tunnel's DNS to its own domains, even when another program changes it.**
  openvpn3, when one of its sessions reconnects, set another OpenVPN 3 tunnel's DNS back to its
  own: every lookup, public ones included, went to that company's DNS, and short names like
  "wpad" turned into the company's (a browser auto-detecting its proxy then hung). Crake now
  notices within seconds and puts its own settings back.
- **OpenVPN 3 profiles whose sessions don't name their device can be used.** For some profiles
  (seen with an Azure VPN gateway's) openvpn3 never says which device a session uses; Crake now
  finds it from the process running the session, where it said "didn't say which device the
  session uses". If Crake still can't take a session over, it ends it rather than leave it up
  unrouted.

## 0.6.0 (2026-10-14)

- **Crake keeps its size when a monitor with another scale is plugged in.** On Linux, a laptop
  at 100% that got a HiDPI monitor showed Crake's windows at half size until Crake was started
  again, and the panel opened away from the tray icon. Crake now follows the desktop's scale as
  it changes, and the panel opens at the icon at any scale.
- **An OpenVPN 3 tunnel whose session ended behind Crake's back shows as down.** When openvpn3
  ended a session while Crake's window wasn't running (it does when the app that started it
  quits), the tunnel stayed "Connected · its DNS doesn't answer" for good. Crake now notices
  within seconds that the session's device is gone and says the tunnel stopped.
- **Signing in to OpenVPN 3 again no longer ends in "isn't a VPN device".** openvpn3 can say a
  session is connected a moment before it names its device; Crake now waits for the name instead
  of giving up and ending the session. If Crake can't use a session, the log says why.
- **A tunnel that stopped says why**, when Crake knows: "Session expired", "Ended by the gateway",
  "Sign-in refused" or "Ended by OpenVPN 3" instead of "Couldn't stay connected" in the panel,
  and the whole sentence in the tunnel's detail.
- **The tray shows that Crake is connecting while you sign in again** to a tunnel that stopped,
  instead of keeping its red "!" until the sign-in is done.

## 0.5.0 (2026-10-13)

- **The panel says when a browser looks names up itself.** Firefox with DNS over HTTPS on, or
  Chrome, Edge and Brave with Secure DNS set to a provider of their own, ask their own DNS, so
  Crake can't send a company's names through the tunnel before they connect: the first visit
  there waits a few seconds. A notice under the tunnel says so, and what fixes it, until "Got it".
- **"Copy for helpdesk" says what Crake sees on this computer:** each tunnel's device, address and
  DNS, who answers the computer's DNS, and which browsers look names up themselves (DNS over
  HTTPS, where Crake can't route a company's names before the first packet). A helpdesk no
  longer has to ask you to look it up (with host names hidden, the addresses are left out too).
- **"Start when you log in" set up by an older Crake is brought up to date** when Crake starts, so
  on macOS it too shows as Crake, with its icon, in System Settings' background items.
- **macOS and Windows: when the network's DNS stops answering, the internet keeps working.** While
  a tunnel is up, Crake answers the computer's DNS and asks the network's servers in its stead. If
  they stop answering (a hotel's sign-in page, a network that changed), Crake now gives the
  computer its own DNS back, says so, and tries again two minutes later.
- **Undo and Redo change the chart's tick, and only that.** Undo takes one off its count, or turns
  it grey when nothing's left; Redo puts it back. Undoing and redoing used to add a decision each
  time, as if Crake had routed the host again.

## 0.4.0 (2026-10-13)

- **macOS: a tunnel can no longer take the whole internet down.** While a tunnel is up, Crake
  answers the Mac's DNS so it can find names only a company knows. On a Mac whose DNS servers it
  couldn't read, every name failed; it now leaves such a Mac's DNS alone.
- **macOS: Crake's windows open in front.** Settings, New tunnel, the log and a tunnel's details
  opened behind the app you were in.
- **macOS: the Mac's own window buttons.** Close, minimise and zoom sit top left as in every Mac
  app, instead of Crake's header being drawn under them; Crake's own close button is gone there.
- **macOS: Crake shows as Crake in System Settings' background items,** with its icon, not under
  the developer's name.
- **macOS on Intel Macs too.** The installer and the Homebrew cask now carry Crake for both Apple
  Silicon and Intel.
- **Works with encrypted DNS (DNS over TLS).** A tunnel's own DNS is asked plainly, as it must be,
  even when the computer encrypts the rest; the log says once that it's on.
- **Settings → About has a "Buy me a coffee".** Crake is free and stays free; a coffee is a
  thank-you and unlocks nothing.

## 0.3.0 (2026-10-13)

- **OpenVPN 3: a second session for the same tunnel no longer stops both.** When a new session turns
  up (openvpn3's indicator reconnecting, say) it takes over from the old one, and the old one
  ending can't take the new one down.
- **"Start when you log in" works on macOS and Windows** (it did nothing there): a LaunchAgent on
  macOS, the user's Run key on Windows, as each system expects.
- **A tunnel that stops by itself says so, and why.** A notification tells you, with **Switch on**
  to sign in again. When an OpenVPN 3 server ends the session (many do after an hour or so), the
  log and the notification say the session expired and after how long, instead of only "the
  session is gone".

## 0.2.0 (2026-10-13)

- **Your first tunnel is easy to find.** With no tunnel yet, the panel says so and offers
  **Add a tunnel…**, while the bird peeks out of the grass now and then, looking for one. The
  tray's menu has **New tunnel…** too, on every system.
- **New tunnel shows it's still asking the gateway:** the dashed head turns, and after a moment
  the line says for how long. A slow gateway no longer looks like nothing is happening.
- **Tab** goes from New tunnel's name to its gateway, and Shift+Tab back.
- **packages.dorhawk.software** has a start page with the commands to add it. On a system with
  32-bit packages switched on, apt noted that the repository has none; fetch the sources file
  again to quiet it:
  `curl -fsSL https://packages.dorhawk.software/apt/dorhawk.sources | sudo tee /etc/apt/sources.list.d/dorhawk.sources >/dev/null`

## 0.1.0 (2026-10-13)

The first release. Crake connects to the VPNs you already have, and sends nothing through them
until a host needs it.

### What it does

- **Learns from DNS.** A name only the company's DNS knows is routed before the first packet.
  Names public DNS has never heard of are asked of your tunnels too.
- **Learns from silence.** A host that doesn't answer directly is tried through each tunnel. The
  one that answers gets it, and the log says so in a sentence. On Linux, apps that talk UDP are
  covered too.
- **Undo, always.** Every decision can be undone where it's shown. Sent direct, a host stays direct
  until you say otherwise.
- **Several tunnels at once,** in the order you set. When two answer for the same host, Crake asks
  you, takes the faster, or keeps to the order. Tunnels that can't be up together are refused
  with the reason, before they half-work.
- **Rules from IT.** A managed profile says what a tunnel may learn, or asks for a full tunnel.
  Crake shows it in plain words, and nobody can widen it.
- **Public name, private address.** When a company serves its own staff a different address for a
  public name, Crake asks the company's DNS and routes the private one.
- **Internal names stay internal.** With its tunnel off, a company's names fail at once instead of
  going to public DNS.
- **Networks that block UDP:** noticed, the tunnel runs over TCP on the same session, and the
  network is remembered for next time.
- **A tunnel's DNS that stops answering:** Crake says so, and names it answered before keep
  working.
- **Gateways that only let Cisco's client in:** Crake introduces itself as AnyConnect, and tells
  you it did.
- **IPv6** alongside IPv4: learned, routed, and refused rather than sent around a full tunnel
  that has none.
- **OpenVPN 3 profiles,** including sessions started elsewhere, and importing a `.ovpn` file.
- **New tunnel** finds out what the gateway speaks and tells you when its certificate runs out.
- **Passwords and certificates** live in your system's keyring, never in Crake's files.
- **Copy for helpdesk:** the day in plain text, with host names left out if you like.

### Protocols

Cisco AnyConnect (and ocserv) and OpenVPN 3 have been used against real gateways. GlobalProtect,
Ivanti Connect Secure (Pulse), Juniper, Fortinet, F5 and Array are spoken through openconnect and
marked Beta in the app until each has been.

### Where it runs

- **Ubuntu** 26.04, 24.04 and 22.04 and **Debian** 13 and 12: one `.deb`, from Crake's own apt
  repository at packages.dorhawk.software (see the README). On Ubuntu 22.04, sign in with a
  password or a certificate: its openconnect is too old for browser sign-in.
- **Fedora** 45, 44 and 43: an RPM, from the same repository with dnf.
- **Arch Linux:** a PKGBUILD for the AUR.
- **NixOS:** a flake with the package and a module (`services.crake.enable`).
- **Windows 11, preview:** an installer with openconnect included. It isn't code-signed yet, so
  Windows warns before installing it. The first connection to a host Crake has just learned fails
  once; the app's retry goes through.
- **macOS on Apple Silicon, preview:** a signed and notarized installer, also as a Homebrew cask
  (`brew install --cask dorhawk-software/tap/crake`, which brings openconnect along). It hasn't
  been tried on a real Mac yet.
