#!/usr/bin/env python3
"""check-views' reading half: SwiftUI views read for four of the craft
rules. Each finding names its rule.

- join (ui/visual-craft.md, "A joined line breaks between its parts"): a
  Text whose own string joins parts with a separator (" · ",
  "\\u{00B7}", " | ") beside an ordinary space, with no .lineLimit(1)
  and no horizontal .fixedSize. Wrapped, the dot ends one line or starts
  the next ("tento týden ·" / "celkem 18"). A sum of Texts is read to its
  end and out of the parentheses around it.
- stack (ui/visual-craft.md, "Alike buttons stacked become one block"):
  two Buttons in the same .buttonStyle, the second straight after the
  first (only closing braces and #if blocks between), in a vertical
  stack. They read as one button twice. One block (rows rounded at its
  ends, a separator view between) or a word between passes.
- menu (ui/pickers.md, "A time turns on a wheel; a menu holds a
  handful"): a Menu whose content has a ForEach, or a Picker with a
  ForEach that isn't .wheel, .inline, .segmented, .palette or
  .navigationLink. Choices built from data (every quarter hour) scroll
  in a popover over the screen.
- stripe (ui/visual-craft.md, "A box has no stripe down one side"): an
  .overlay or .background aligned to one edge (.leading, .trailing,
  .top, .bottom) that draws a shape 2 to 6 points thick along it
  (.frame(width:) for a side, .frame(height:) for the top or bottom). A
  generic callout's accent, not the product's own sign, and a second
  shape whose corners fight the box's. A hairline (under 2) passes.

    view-audit.py FILE...

A deliberate one is marked on its line or the line above, with a reason
of three words or more:
    // practices-allow join: the dot is quoted, not a separator
    // practices-allow stack: an alert's two answers, which iOS spaces
    // practices-allow menu: the flock's members, six at most
    // practices-allow stripe: a progress bar along the bottom

Exit 0 clean, 1 findings, 2 a file it couldn't read.
Why: 25 Nov 2026, one iPhone app. Its Today read "tento týden ·" over
"celkem 18"; its Focus card had "Close all until 04:00" and "Until a
date…" alike, one under the other; and its time came from a menu of 96
quarter hours. The owner asked for a rule and a check for each. The same
day its lock's confirmation drew its warning as a grey box with a 4 pt
stripe down its left edge, whose corners broke the box's ("Seems kinda ai
sloppy and on top of that it contains rounding visual bug").
"""
import importlib.util
import os
import re
import sys

sys.dont_write_bytecode = True
_spec = importlib.util.spec_from_file_location("motion_audit", os.path.join(os.path.dirname(os.path.abspath(__file__)), "motion-audit.py"))
_motion = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_motion)
strip, matching, line_of = _motion.strip, _motion.matching, _motion.line_of

ALLOW = {rule: re.compile(r"practices-allow\s+" + rule + r"\s*:\s*(.*)") for rule in ("join", "stack", "menu", "stripe")}
TEXT = re.compile(r"(?<![\w.])Text\s*\(")
# A separator beside an ordinary space, where a line may break. Tied with no-break spaces
# (U+00A0) on both sides, it can't: that passes, the fix for a dot inside a sentence.
DOT = r"(?:·|\\u\{00B7\}|\\u\{b7\})"
JOIN = re.compile(r"[ \n](?:" + DOT + r"|\|)[ \n]|[ \n]" + DOT + r"|" + DOT + r"[ \n]", re.I)
ONE_LINE = re.compile(r"^lineLimit\(\s*1\s*[,)]|^fixedSize\(\s*\)|^fixedSize\(\s*horizontal\s*:\s*true")
IDENT = re.compile(r"[A-Za-z_]\w*")


def allowed(rule, lines, ln):
    here = ALLOW[rule].search(lines[ln - 1]) if ln - 1 < len(lines) else None
    above = lines[ln - 2] if ln >= 2 else ""
    m = here or (ALLOW[rule].search(above) if above.lstrip().startswith("//") else None)
    return m.group(1).strip() if m else None


def judged(rule, lines, ln, finding):
    """The finding, an unreasoned mark, or None for a reasoned mark."""
    reason = allowed(rule, lines, ln)
    if reason is None:
        return (ln, rule, finding)
    if len(reason.split()) < 3:
        return (ln, rule, f"marks an exception without its reason (\"practices-allow {rule}: {reason}\"): say why in three words or more.")
    return None


def skip_ws(code, k):
    while k < len(code) and code[k].isspace():
        k += 1
    return k


def chain(code, k):
    """The modifiers after k (".a(…) .b { … }"), and the index past them."""
    mods = []
    while True:
        j = skip_ws(code, k)
        if code.startswith("#if", j):
            e = code.find("#endif", j)
            if e < 0:
                return mods, k
            inner, _ = chain(code, j + 3 + len(re.match(r"[^\n]*", code[j + 3:]).group(0)))
            mods += inner
            k = e + len("#endif")
            continue
        if j >= len(code) or code[j] != "." or not IDENT.match(code, j + 1):
            return mods, k
        m = IDENT.match(code, j + 1)
        start, j = m.start(), m.end()
        if j < len(code) and code[j] == "(":
            j = matching(code, j, "(", ")")
        t = skip_ws(code, j)
        if t < len(code) and code[t] == "{":
            j = matching(code, t, "{", "}")
        mods.append(re.sub(r"\s+", "", code[start:j]))
        k = j


def operand(code, k):
    """Past one operand of a +: a name or call, with its modifiers."""
    j = skip_ws(code, k)
    m = IDENT.match(code, j)
    if not m:
        if j < len(code) and code[j] == "(":
            return chain(code, matching(code, j, "(", ")"))[1]
        return j
    j = m.end()
    t = skip_ws(code, j)
    if t < len(code) and code[t] == "(":
        j = matching(code, t, "(", ")")
    return chain(code, j)[1]


def grouping(code, close):
    """Whether the ")" at close ends parentheses that group, not a call's arguments."""
    depth = 0
    for k in range(close, -1, -1):
        if code[k] == ")":
            depth += 1
        elif code[k] == "(":
            depth -= 1
            if depth == 0:
                # A call's "(" follows its callee on the same line; one that starts a line groups.
                b = k - 1
                while b >= 0 and code[b].isspace():
                    if code[b] == "\n":
                        return True
                    b -= 1
                return b < 0 or not (code[b].isalnum() or code[b] in "_?!])>")
    return False


def join_audit(text, code, lines):
    out = []
    for m in TEXT.finditer(code):
        open_at = m.end() - 1
        close = matching(code, open_at, "(", ")")
        # Only the first argument is what the Text shows (comment: and the rest aren't).
        depth, end = 0, close - 1
        for k in range(open_at + 1, close - 1):
            if code[k] in "([{":
                depth += 1
            elif code[k] in ")]}":
                depth -= 1
            elif code[k] == "," and depth == 0:
                end = k
                break
        # The Text's own string literals: their spans in the stripped code, read in the source.
        joined = None
        q = open_at
        while True:
            a = code.find('"', q, end)
            if a < 0:
                break
            b = code.find('"', a + 1, end)
            if b < 0:
                break
            if JOIN.search(text[a + 1:b]):
                joined = text[a + 1:b]
                break
            q = b + 1
        if joined is None:
            continue
        mods, k = chain(code, close)
        # A sum of Texts is one line: read on to its end, and out of the parentheses around it,
        # where .lineLimit goes ((Text(a) + Text(" · ") + Text(b)).lineLimit(1)). An operand's
        # own modifiers return Text (.bold, .foregroundStyle), never a line limit.
        while True:
            j = skip_ws(code, k)
            if j < len(code) and code[j] == "+" and code[j + 1:j + 2] != "=":
                k = operand(code, j + 1)
                continue
            if j < len(code) and code[j] == ")" and grouping(code, j):
                more, k = chain(code, j + 1)
                mods += more
                continue
            break
        if any(ONE_LINE.match(x) for x in mods):
            continue
        ln = line_of(code, m.start())
        shown = joined if len(joined) <= 60 else joined[:57] + "…"
        f = judged("join", lines, ln, f"Text(\"{shown}\") joins parts with a separator and may wrap: the dot can end one line or start the next.")
        if f:
            out.append(f)
    return out


BUTTON = re.compile(r"(?<![\w.])Button\s*[({]")
CONTAINER = re.compile(r"(VStack|LazyVStack|HStack|LazyHStack|ZStack|Grid|GridRow|Form|List|Section|ScrollView|Group|ViewThatFits|ToolbarItem\w*|var\s+body\s*:\s*some\s+View|\w+)\s*(?:\([^()]*(?:\([^()]*\)[^()]*)*\))?\s*$")


def button_end(code, k):
    """Past a Button's call: its arguments, trailing closures (label: too), then its modifiers."""
    k = skip_ws(code, k + len("Button"))
    if k < len(code) and code[k] == "(":
        k = matching(code, k, "(", ")")
    while True:
        j = skip_ws(code, k)
        m = re.match(r"\w+\s*:\s*\{", code[j:])
        if j < len(code) and code[j] == "{":
            k = matching(code, j, "{", "}")
        elif m:
            k = matching(code, j + m.end() - 1, "{", "}")
        else:
            break
    return chain(code, k)


def container(code, at):
    """The view that holds what starts at `at`: the nearest enclosing block that isn't an
    if, else, ForEach or closure argument, by the words before its brace."""
    depth = 0
    for k in range(at - 1, -1, -1):
        c = code[k]
        if c == "}":
            depth += 1
        elif c == "{":
            if depth:
                depth -= 1
                continue
            before = code[max(0, k - 200):k].rstrip()
            last = before.split("\n")[-1].strip()
            if re.search(r"(^|[\s}])(if|else|for|switch|case)\b|\bForEach\b|\bin$|label:$", last) or not last:
                continue
            m = re.search(r"([A-Za-z_]\w*)\s*(?:\(.*\))?\s*$", last)
            return m.group(1) if m else ""
    return ""


def stack_audit(text, code, lines):
    out = []
    for m in BUTTON.finditer(code):
        mods, k = button_end(code, m.start())
        style = next((x for x in mods if x.startswith("buttonStyle(")), None)
        if not style:
            continue
        # What follows: only closing braces and #if blocks before another Button?
        j = skip_ws(code, k)
        while j < len(code):
            if code[j] == "}":
                j = skip_ws(code, j + 1)
            elif code.startswith("#if", j) or code.startswith("#endif", j) or code.startswith("#else", j):
                j = skip_ws(code, code.find("\n", j) if code.find("\n", j) > 0 else len(code))
            else:
                break
        n = BUTTON.match(code, j)
        if not n:
            continue
        mods2, _ = button_end(code, j)
        if next((x for x in mods2 if x.startswith("buttonStyle(")), None) != style:
            continue
        holder = container(code, m.start())
        if holder in ("HStack", "LazyHStack", "GridRow", "Form", "List", "Section", "ToolbarItem", "ToolbarItemGroup") or holder.startswith("Toolbar"):
            continue
        ln = line_of(code, j)
        f = judged("stack", lines, ln, f"two buttons in .{style}, one under the other with nothing between: they read as one button twice. Make them one block (rows rounded at its ends, a separator between) or put a word between.")
        if f:
            out.append(f)
    return out


MENU = re.compile(r"(?<![\w.])Menu\s*[({]")
PICKER = re.compile(r"(?<![\w.])Picker\s*\(")
WHEELISH = re.compile(r"^pickerStyle\(\s*\.(wheel|inline|segmented|palette|navigationLink)\b|^pickerStyle\(\s*(Wheel|Inline|Segmented|Palette|NavigationLink)PickerStyle")


def menu_audit(text, code, lines):
    out, menus = [], []
    for m in MENU.finditer(code):
        k = skip_ws(code, m.end() - 1)
        if code[k] == "(":
            k = skip_ws(code, matching(code, k, "(", ")"))
        if k >= len(code) or code[k] != "{":
            continue
        end = matching(code, k, "{", "}")
        menus.append((m.start(), end))
        if re.search(r"(?<![\w.])ForEach\s*[({]", code[k:end]):
            ln = line_of(code, m.start())
            f = judged("menu", lines, ln, "a Menu built from a ForEach: choices made from data (every quarter hour) scroll in a popover. Pick a time or a long list on a wheel (DatePicker, or Picker with .pickerStyle(.wheel)).")
            if f:
                out.append(f)
    for m in PICKER.finditer(code):
        if any(a <= m.start() < b for a, b in menus):
            continue
        k = matching(code, m.end() - 1, "(", ")")
        body_start, body_end = k, k
        while True:
            j = skip_ws(code, k)
            lab = re.match(r"\w+\s*:\s*\{", code[j:])
            if j < len(code) and code[j] == "{":
                body_end = k = matching(code, j, "{", "}")
            elif lab:
                k = matching(code, j + lab.end() - 1, "{", "}")
            else:
                break
        content = code[m.start():body_end]
        if not re.search(r"(?<![\w.])ForEach\s*[({]", content):
            continue
        mods, _ = chain(code, k)
        if any(WHEELISH.match(x) for x in mods):
            continue
        ln = line_of(code, m.start())
        f = judged("menu", lines, ln, "a Picker built from a ForEach, in its default menu style: choices made from data scroll in a popover. Give it .pickerStyle(.wheel), or use a DatePicker for a time.")
        if f:
            out.append(f)
    return out


EDGED = re.compile(r"\.(overlay|background)\s*\(\s*alignment\s*:\s*\.(leading|trailing|top|bottom)\s*\)\s*\{")
SHAPE = re.compile(r"(?<![\w.])(Rectangle|RoundedRectangle|UnevenRoundedRectangle|Capsule|Color)\b|(?<![\w])\.fill\(")
THICK = {"leading": "width", "trailing": "width", "top": "height", "bottom": "height"}


def stripe_audit(text, code, lines):
    out = []
    for m in EDGED.finditer(code):
        start = m.end() - 1
        body = code[start:matching(code, start, "{", "}")]
        if not SHAPE.search(body):
            continue
        side = THICK[m.group(2)]
        size = re.search(r"\.frame\(\s*" + side + r"\s*:\s*(\d+(?:\.\d+)?)\s*[,)]", body)
        if not size or not 2 <= float(size.group(1)) <= 6:
            continue
        ln = line_of(code, m.start())
        f = judged("stripe", lines, ln, f"a {size.group(1)} pt shape along the box's {m.group(2)} edge, an accent stripe. Say it with the product's own sign (its badge or chip, an icon in the line), or the box's whole tone.")
        if f:
            out.append(f)
    return out


def audit(text):
    code = strip(text)
    lines = text.split("\n")
    return join_audit(text, code, lines) + stack_audit(text, code, lines) + menu_audit(text, code, lines) + stripe_audit(text, code, lines)


RULES = {
    "join": ("A joined line breaks between its parts", "ui/visual-craft.md",
             "one line where it fits, a line for each part where it doesn't: ViewThatFits(in: .horizontal) { the joined Text with .lineLimit(1); VStack(alignment: .leading) { the parts, without the dot } }. In a sentence that wraps anyway, tie the dot to both words with no-break spaces (\\u{00A0}·\\u{00A0}).",
             "an iPhone app's Today read \"tento týden ·\" over \"celkem 18\" (25 Nov 2026): the joined line wrapped at its dot."),
    "stack": ("Alike buttons stacked become one block", "ui/visual-craft.md",
              "make the two one block: rows rounded only at the block's ends, a hairline view between; or put a word between them (\"or\").",
              "an iPhone app's Focus card had \"Close all until 04:00\" and \"Until a date…\" alike, one under the other (25 Nov 2026): the owner read one button twice."),
    "menu": ("A time turns on a wheel; a menu holds a handful", "ui/pickers.md",
             "pick a time with a DatePicker (a wheel, in minute steps) and a list built from data with Picker(...).pickerStyle(.wheel); a menu keeps a few fixed choices.",
             "an iPhone app picked a lock's end time from a menu of 96 quarter hours (25 Nov 2026), a popover over the calendar."),
    "stripe": ("A box has no stripe down one side", "ui/visual-craft.md",
               "drop the stripe: say it with the product's own sign (the chip or badge it already uses for that meaning, an icon at the line's start), or tint the whole box.",
               "an iPhone app's lock confirmation drew its warning as a grey box with a 4 pt stripe down its left edge (25 Nov 2026): a generic callout, and the stripe's corners broke the box's."),
}


def main(argv):
    findings, broken = [], []
    for path in argv:
        try:
            text = open(path, encoding="utf-8").read()
        except (OSError, UnicodeDecodeError) as e:
            broken.append(f"{path}: {e}")
            continue
        findings += [(path, ln, rule, msg) for ln, rule, msg in sorted(audit(text))]
    if broken:
        print("check-views could not read these, so it can't say they are clean:")
        for b in broken:
            print(f"  - {b}")
    for rule, (name, doc, fix, why) in RULES.items():
        mine = [f for f in findings if f[2] == rule]
        if not mine:
            continue
        n = len(mine)
        print(f"[{rule}] {n} finding{'s' if n > 1 else ''}. Rule: {name} (.practices/docs/{doc}).")
        print(f"  Fix: {fix}")
        print(f"  Deliberate? Mark it on the line or the line above: // practices-allow {rule}: <the reason>.")
        print(f"  Why: {why}")
        for path, ln, _, msg in mine:
            print(f"  - {path}:{ln} {msg}")
    print(f"\ncheck-views: {len(argv)} file{'s' if len(argv) != 1 else ''}: {len(findings) or 'no'} finding{'s' if len(findings) != 1 else ''}.")
    return 2 if broken else 1 if findings else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
