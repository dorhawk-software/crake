# Window buttons where each system puts them

A desktop app that draws its own window header still puts the window
buttons where the system does: macOS's red, yellow and green on the left,
drawn by macOS itself; the app's own on the right on Windows and Linux.
The header leaves them their room and lines up with them.

Links point at commits in [Crake](https://github.com/dorhawk-software/crake).

## macOS draws its own buttons; the header makes room

**What.** On macOS the window keeps the system's three buttons at the top
left. The app's header doesn't draw its own close, minimise or maximise
there, starts its content after the buttons, and is laid out so the
buttons sit centred on its line. On Windows and Linux the app draws its
buttons, on the right.

**Why.** Crake draws its own headers (Cancel, the title, ×), as it does on
Linux. A colleague's first run on a Mac showed macOS drawing its buttons
on top of that header, over Cancel
([407cf71](https://github.com/dorhawk-software/crake/commit/407cf71)). A
proposal showed two fixes in place (`design/proposals/macos-window-buttons.html`);
option A was chosen: the system draws the buttons, top left and centred on
the header, the header leaves them room and drops its own close, and New
tunnel's Cancel moves to the right there. Two more lessons followed:

- The room has to be measured, and then looked at. 76 px left none:
  Traffic's title touched the green button. 88 px fitted, but tern's
  owner saw the bird sit closer to the buttons than the buttons sit to
  the window's edge: macOS 26 draws three 14 px buttons 9 px apart, 60 px
  across, which left only 10 px after them. 96 px (18 px to the first
  button, 60 across, 18 clear after them, as much as before them) looks
  right.
- Being after the buttons isn't enough; the header has to be on their
  line. Traffic's and the profile editor's titles stood 4 px above the
  buttons ([c75a781](https://github.com/dorhawk-software/crake/commit/c75a781)).
  The owner's rule: "the buttons should be the topmost thing, or level
  with it".

**Apply.**
- **macOS:** keep the system's buttons, place them (the toolkit usually
  lets the app set their position) centred on a header of one fixed
  height, and reserve their width at the header's left. Don't draw a close
  button of your own; anything that sat at the top left (Cancel) moves to
  the right.
- **Windows:** the app's own buttons at the top right, in the system's
  order: minimise, maximise, close. That's the platform's convention, not
  something Crake had to learn.
- **Linux:** the app's own buttons on the right is the safe default.
  GNOME's own default is a close button alone on the right; the user can
  change it (`gsettings get org.gnome.desktop.wm.preferences button-layout`,
  left and right of the colon), and KDE lets the user arrange them too. An
  app that wants to follow the user's layout reads that setting. Crake
  draws a single × on Linux and Windows; tern draws minimise, maximise and
  close.
- **Every system:** the header is the drag zone (drag to move,
  double-click to maximise or restore, right-click for the window menu),
  except over its buttons and fields, which take their own clicks.
- A new or changed header is a UI change, so it starts as a proposal with
  options on each system ([design before build](design-before-build.md)).

**Check.**
- A headless test per window, with the system's buttons on: no close
  button of the app's own, the first thing in the header starts at or
  after the reserved width, and the title's middle is on the buttons'
  middle (within a pixel). Crake has one for each of its windows (the log,
  the tunnel detail, Traffic, Settings, New tunnel).
- Look at each window on each system you ship for, at the top left
  especially, before calling header work done.
- On a Mac, capture a window and measure the gap after the green button
  against the gap before the red one; they should match. Look at an
  inactive window too: macOS draws its buttons grey there (on a light
  header they read as faint rings), which is the system, not a fault.

For GPUI: [rust/gpui.md](../rust/gpui.md#window-buttons-on-macos).
