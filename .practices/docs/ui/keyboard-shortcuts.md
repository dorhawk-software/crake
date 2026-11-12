# Keyboard shortcuts the way each system writes them

A shortcut is bound and shown the way the system it runs on does it. On a
Mac that means ⌘ where Linux and Windows have Ctrl, ⌥ where they have Alt,
and the Mac's symbols and order. A Mac user who sees "Ctrl K" knows at
once the app wasn't made for their computer. If ⌘K then does nothing, the
app is broken for them.

## Bind the system's main modifier, not Ctrl

**What.** An app's own shortcuts use the system's main modifier: Command
on macOS and Ctrl on Linux and Windows. Ctrl is bound by name only when a
shortcut really means the Control key on every system, which is rare.

**Why.** tern (a GPUI mail client) bound `ctrl-k` for search and `ctrl-b`
for bold. Its first run on a Mac (2026-11-11) showed "Ctrl K" in the
search field, and ⌘K did nothing. On a Mac, ⌃K in a text field already has
a meaning: it deletes to the end of the line. The owner's verdict: showing
⌘ instead of Ctrl, and ⌥ instead of Alt, is "absolutely vital and
absolutely necessary for a product to be well done from a UX point of
view". Crake learned the same thing earlier from its first real Mac run:
⌘W closes a window and ⌘Q quits
([ea6b144](https://github.com/dorhawk-software/crake/commit/ea6b144)).

**Apply.**
- Bind through the toolkit's "main modifier" if it has one (GPUI:
  `secondary-k`; Electron: `CmdOrCtrl+K`; the web: check `metaKey` on a
  Mac and `ctrlKey` elsewhere), or choose the modifier per system in one
  place.
- Some shortcuts differ by more than the modifier. Use each system's own:

  | Action | macOS | Linux and Windows |
  | --- | --- | --- |
  | Settings | ⌘, | Ctrl+, (no fixed rule; many apps use it) |
  | Quit | ⌘Q | Ctrl+Q |
  | Close the window | ⌘W | Ctrl+W |
  | Redo | ⇧⌘Z | Ctrl+Shift+Z (Windows also Ctrl+Y) |
  | Find the next one | ⌘G | F3 or Ctrl+G |
  | Delete the selected item | ⌘⌫ | Delete |

- Leave the system's own shortcuts alone. On a Mac those are ⌘H, ⌘M, ⌘Q,
  ⌘W, ⌘\`, ⌘Tab and ⌘Space. Elsewhere they are Alt+Tab, Alt+F4 and the
  Super key.
- Single-key shortcuts (E for done, J and K to move) are the same
  everywhere. Only the modifiers change.

## Show them the way the system writes them

**What.** Every place a shortcut appears follows the system's way of
writing it: tooltips, menus, hints in fields, the shortcut list, tips.

**macOS.**
- Modifiers are symbols, in the order ⌃ ⌥ ⇧ ⌘, then the key, with no
  plus signs: ⇧⌘K, ⌥⌘N.
- Keys are written ↩ (Return), ⌫ (Delete), ⌦ (Forward Delete), ⎋ (Esc),
  ⇥ (Tab), ␣ or Space, and the arrows ← → ↑ ↓. Letters are capitals.
- In running text, use the words: Command, Option, Control, Shift.
  Write "hold Option", never "hold Alt".

**Linux and Windows.**
- Words joined by plus signs, in the order Ctrl, Alt, Shift, then the
  key: Ctrl+Shift+K. Letters are capitals.
- Key names are Enter, Backspace, Delete, Esc, Tab and Space. Don't show
  Super or the Windows key to people.
- An app that draws keys as separate caps (`Ctrl` `K`) may drop the plus
  signs, but it must do so the same way everywhere.

**Why.** tern's search hint said "Ctrl K" on a Mac, and so did its
tooltips ("Bold (Ctrl B)"). Its translation files had the key names
typed into the text, so the same words showed on every system.

**Apply.**
- Write a shortcut once, as its binding, and make its label from that
  binding with one function. The function takes the system as an
  argument, so both forms can be tested on any machine.
- Translations hold words, never keys. A tooltip's text is
  `Bold ({ $keys })`, and the app fills in `$keys` from the binding.
  Translators can't get a key wrong, and changing a binding changes every
  place it's shown.
- The toolkit's own way of showing a keystroke is rarely what people
  should see. Check it before using it, because some print `ctrl-K`.

## Check

- A test that no translation file contains a modifier's name or symbol
  (Ctrl, Alt, Cmd, Option, ⌘, ⌥, ⌃, ⇧). This catches the next "Ctrl" typed
  into the text.
- A test of the label function with both systems for every binding the
  app shows. The Mac form must have no "Ctrl" or "+", and the other form
  no ⌘ or ⌥.
- A test, on a Mac build, that the main shortcuts are bound to ⌘ (press
  ⌘K in a headless window, and the search opens).
- Look at each touched screen on a Mac and on Linux. Check the hints in
  fields, the tooltips and the shortcut list.

For GPUI: [rust/gpui.md](../rust/gpui.md#keyboard-shortcuts-per-system).
