#!/usr/bin/env python3
"""check-motion's reading half: a selection switches at once
(ui/visual-craft.md). In Swift it finds the two ways SwiftUI animates a
change of a selection state:

  - withAnimation(...) { selected = day }   (any assignment or toggle()
    of a selection state inside the closure);
  - .animation(.snappy, value: selected)    (a view animated whenever
    the selection changes).

A selection state is a name matched whole by --names (a regex; the
default below, MOTION_SELECTION in .practices/config adds the app's
own, added to these). Animated, the old and the new choice cross-fade: for a moment both
are half lit, which reads as a flicker.

    motion-audit.py [--names REGEX] FILE...

A deliberate moving highlight (a capsule that slides to the new tab) is
marked on the line of withAnimation or .animation, or the line above,
with a reason of three words or more:
    // practices-allow motion: the capsule slides between tabs

Exit 0 clean, 1 findings, 2 a file it couldn't read.
Why: on 25 Nov 2026 an iPhone app's two-week chart flickered on every tap
on a bar: withAnimation(.snappy) set the selected day, and the old and the
new bar faded across each other.
"""
import re
import sys

sys.dont_write_bytecode = True

NAMES = r"selected\w*|selection\w*|highlighted\w*|\w+Selection|\w+Selected"
ALLOW = re.compile(r"practices-allow\s+motion\s*:\s*(.*)")
WITH = re.compile(r"\bwithAnimation\b")
MODIFIER = re.compile(r"\.animation\s*\(")


def strip(text):
    """The code with comments and string contents blanked, same length, so offsets and lines hold."""
    out, i, n = [], 0, len(text)
    while i < n:
        c = text[i]
        if text.startswith("//", i):
            j = text.find("\n", i)
            j = n if j < 0 else j
            out.append(" " * (j - i)); i = j
        elif text.startswith("/*", i):
            j = text.find("*/", i + 2)
            j = n if j < 0 else j + 2
            out.append(re.sub(r"[^\n]", " ", text[i:j])); i = j
        elif text.startswith('"""', i):
            j = text.find('"""', i + 3)
            j = n if j < 0 else j + 3
            out.append('"' + re.sub(r"[^\n]", " ", text[i + 1:j - 1]) + '"'); i = j
        elif c == '"':
            j = i + 1
            while j < n and text[j] not in '"\n':
                j += 2 if text[j] == "\\" else 1
            j = min(j, n)
            closed = j < n and text[j] == '"'
            out.append('"' + " " * (j - i - 1) + ('"' if closed else "")); i = j + (1 if closed else 0)
        else:
            out.append(c); i += 1
    return "".join(out)


def matching(code, start, open_ch, close_ch):
    """The index just past the bracket that closes the one at start."""
    depth = 0
    for k in range(start, len(code)):
        if code[k] == open_ch:
            depth += 1
        elif code[k] == close_ch:
            depth -= 1
            if depth == 0:
                return k + 1
    return len(code)


def line_of(code, offset):
    return code.count("\n", 0, offset) + 1


def allowed(lines, ln):
    """The reason marked on the line, or on a comment line just above it."""
    here = ALLOW.search(lines[ln - 1]) if ln - 1 < len(lines) else None
    above = lines[ln - 2] if ln >= 2 else ""
    m = here or (ALLOW.search(above) if above.lstrip().startswith("//") else None)
    return m.group(1).strip() if m else None


def audit(path, text, names):
    code = strip(text)
    lines = text.split("\n")
    assign = re.compile(r"(?<![\w.$])(?:self\.)?(" + names + r")\s*(?:=(?!=)|\.toggle\s*\()")
    found = []
    for m in WITH.finditer(code):
        k = m.end()
        while k < len(code) and code[k].isspace():
            k += 1
        if k < len(code) and code[k] == "(":
            k = matching(code, k, "(", ")")
            while k < len(code) and code[k].isspace():
                k += 1
        if k >= len(code) or code[k] != "{":
            continue
        body = code[k:matching(code, k, "{", "}")]
        hit = assign.search(body)
        if hit:
            found.append((line_of(code, m.start()), hit.group(1), "withAnimation sets"))
    for m in MODIFIER.finditer(code):
        args = code[m.end() - 1:matching(code, m.end() - 1, "(", ")")]
        hit = re.search(r"\bvalue\s*:\s*(?:self\.)?(" + names + r")\b", args)
        if hit:
            found.append((line_of(code, m.start()), hit.group(1), ".animation follows"))
    out = []
    for ln, name, how in found:
        reason = allowed(lines, ln)
        if reason is None:
            out.append((ln, f"{how} the selection \"{name}\": the old and the new choice cross-fade, both half lit."))
        elif len(reason.split()) < 3:
            out.append((ln, f"marks an exception without its reason (\"practices-allow motion: {reason}\"): say why in three words or more."))
    return out


def main(argv):
    names = NAMES
    if argv[:1] == ["--names"]:
        names = NAMES + ("|" + argv[1] if argv[1] else "")
        argv = argv[2:]
    findings, broken = [], []
    for path in argv:
        try:
            text = open(path, encoding="utf-8").read()
        except (OSError, UnicodeDecodeError) as e:
            broken.append(f"{path}: {e}")
            continue
        findings += [(path, ln, msg) for ln, msg in sorted(audit(path, text, names))]
    if broken:
        print("check-motion could not read these, so it can't say they are clean:")
        for b in broken:
            print(f"  - {b}")
    if findings:
        n = len(findings)
        print(f"[motion] {n} finding{'s' if n > 1 else ''}. Rule: A selection switches at once (.practices/docs/ui/visual-craft.md).")
        print("  Fix: set the selection without animation (drop withAnimation, or the .animation(_:value:) on it). Animate only what moves, such as a scroll to the chosen item.")
        print("  A highlight that slides from the old choice to the new one on purpose? Mark it on the line or the line above: practices-allow motion: <the reason>.")
        print("  Why: an iPhone app's chart flickered on every tap on a bar (25 Nov 2026): the old and the new bar faded across each other.")
        for path, ln, msg in findings:
            print(f"  - {path}:{ln} {msg}")
    print(f"\ncheck-motion: {len(argv)} file{'s' if len(argv) != 1 else ''}: {len(findings) or 'no'} finding{'s' if len(findings) != 1 else ''}.")
    return 2 if broken else 1 if findings else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
