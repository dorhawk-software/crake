#!/usr/bin/env python3
"""check-gpui-clip's reading half: GPUI elements that round and clip.

    python3 gpui-clip-audit.py FILE...

GPUI clips a child to a rectangle (its ContentMask holds bounds, no corner
radii), so `overflow_hidden()` on a rounded element never rounds what is
inside it (rust/gpui.md, "Rounded corners don't clip"). This reads Rust
files as tokens, with comments and string literals set aside, groups them
by bracket, and splits each bracket's contents into builder chains: a head
(`div()`, `text(12.)`, a variable) and the `.method(…)` calls after it.
A chain that calls a rounding method (`rounded`, `rounded_*`, not
`rounded_none`) and a clipping one (`overflow_hidden`, `overflow_x_hidden`,
`overflow_y_hidden`) is one finding. A call inside another call's
arguments (`.child(div()…)`) is another element. A one-parameter closure
whose body goes on with its parameter (`.when(on, |d| d.rounded(px(4.)))`)
adds its calls to the element. Quiet when nothing is found; exit 1 with
findings, 2 when a file can't be read.
"""

import re
import sys

ROUND = re.compile(r"rounded(_[a-z0-9_]+)?$")
CLIP = ("overflow_hidden", "overflow_x_hidden", "overflow_y_hidden")
ALLOW = re.compile(r"practices-allow\s+clip\s*:\s*(.*)")
IDENT = re.compile(r"(?:r#)?[A-Za-z_][A-Za-z0-9_]*")
RAW = re.compile(r'(?:b|c)?r(#*)"')
STR = re.compile(r'(?:b|c)?"')
NUMBER = re.compile(r"[0-9][0-9A-Za-z_]*")
PUNCT = ("::", "=>", "->")


def tokens(text):
    """The tokens as (kind, value, line), and each line's comments."""
    out, notes = [], {}
    i, n, line = 0, len(text), 1
    while i < n:
        c = text[i]
        if c == "\n":
            line += 1
            i += 1
        elif c.isspace():
            i += 1
        elif text.startswith("//", i):
            j = text.find("\n", i)
            j = n if j < 0 else j
            notes.setdefault(line, []).append(text[i + 2:j])
            i = j
        elif text.startswith("/*", i):
            depth, j = 1, i + 2
            while j < n and depth:
                if text.startswith("/*", j):
                    depth, j = depth + 1, j + 2
                elif text.startswith("*/", j):
                    depth, j = depth - 1, j + 2
                else:
                    j += 1
            line += text.count("\n", i, j)
            i = j
        elif RAW.match(text, i):
            m = RAW.match(text, i)
            end = '"' + m.group(1)
            j = text.find(end, m.end())
            j = n if j < 0 else j + len(end)
            out.append(("str", "", line))
            line += text.count("\n", i, j)
            i = j
        elif STR.match(text, i):
            j = STR.match(text, i).end()
            while j < n and text[j] != '"':
                j += 2 if text[j] == "\\" else 1
            j += 1
            out.append(("str", "", line))
            line += text.count("\n", i, j)
            i = j
        elif c == "'" or text.startswith("b'", i):
            k = i + 1 if c == "'" else i + 2
            if k < n and text[k] == "\\":  # '\n', '\'', '\u{1F600}'
                j = text.find("'", k + 2)
                i = n if j < 0 else j + 1
                out.append(("char", "", line))
            elif k + 1 < n and text[k + 1] == "'":  # 'x'
                i = k + 2
                out.append(("char", "", line))
            else:  # a lifetime or a label: 'a
                m = IDENT.match(text, k)
                i = m.end() if m else k
                out.append(("life", "", line))
        elif IDENT.match(text, i):
            m = IDENT.match(text, i)
            out.append(("id", m.group(), line))
            i = m.end()
        elif NUMBER.match(text, i):
            i = NUMBER.match(text, i).end()
            out.append(("num", "", line))
        else:
            p = next((p for p in PUNCT if text.startswith(p, i)), c)
            out.append(("p", p, line))
            i += len(p)
    return out, notes


def tree(toks):
    """Brackets as groups: ("g", opener, [contents], line)."""
    root = []
    stack = [root]
    for t in toks:
        if t[0] == "p" and t[1] in "([{":
            g = ("g", t[1], [], t[2])
            stack[-1].append(g)
            stack.append(g[2])
        elif t[0] == "p" and t[1] in ")]}":
            if len(stack) > 1:
                stack.pop()
        else:
            stack[-1].append(t)
    return root


def is_p(t, v):
    return t[0] == "p" and t[1] == v


def generics(seq, i):
    """Past a `<…>` that starts at i (after `::`)."""
    depth = 0
    while i < len(seq):
        if is_p(seq[i], "<"):
            depth += 1
        elif is_p(seq[i], ">"):
            depth -= 1
            if depth == 0:
                return i + 1
        i += 1
    return i


def chains(seq):
    """Each builder chain in one bracket's contents: (head, [(method, line, args)])."""
    out, cur, i = [], None, 0
    while i < len(seq):
        t = seq[i]
        if is_p(t, ".") and cur is not None and i + 1 < len(seq) and seq[i + 1][0] == "id":
            name, line = seq[i + 1][1], seq[i + 1][2]
            j = i + 2
            if j + 1 < len(seq) and is_p(seq[j], "::") and is_p(seq[j + 1], "<"):
                j = generics(seq, j + 1)
            if j < len(seq) and seq[j][0] == "g" and seq[j][1] == "(":
                cur[1].append((name, line, seq[j][2]))
                i = j + 1
            else:  # a field, or .await
                i = j
        elif cur is not None and (is_p(t, "?") or (t[0] == "g" and t[1] in "([")):
            i += 1  # div(), a call's result called or indexed, ?
        elif t[0] == "id":
            cur = (t[1], [])
            out.append(cur)
            i += 1
            while i + 1 < len(seq) and is_p(seq[i], "::"):  # a path: gpui::div
                i = generics(seq, i + 1) if is_p(seq[i + 1], "<") else i + 2
            if i + 1 < len(seq) and is_p(seq[i], "!") and seq[i + 1][0] == "g":
                i += 1  # a macro: vec![…]
        elif t[0] == "g" and t[1] == "(":
            cur = (None, [])  # (a).b()
            out.append(cur)
            i += 1
        else:
            cur = None
            i += 1
    return out


def closures(args):
    """The calls a one-parameter closure in a call's arguments makes on its parameter."""
    calls, i = [], 0
    while i < len(args):
        if not is_p(args[i], "|"):
            i += 1
            continue
        j = i + 1
        while j < len(args) and not is_p(args[j], "|"):
            j += 1
        params = args[i + 1:j]
        names = [t[1] for t in params if t[0] == "id" and t[1] != "mut"]
        if j >= len(args) or any(is_p(t, ",") for t in params) or not names:
            i = j + 1
            continue
        k = j + 1
        if k < len(args) and is_p(args[k], "->"):
            while k < len(args) and not (args[k][0] == "g" and args[k][1] == "{"):
                k += 1
        if k < len(args) and args[k][0] == "g" and args[k][1] == "{":
            body = args[k][2]
        else:
            body = []
            while k < len(args) and not is_p(args[k], ","):
                body.append(args[k])
                k += 1
        for head, chain in chains(body):
            if head == names[0]:
                calls += element(chain)
        i = k + 1
    return calls


def element(chain):
    """Every call on one element: its chain's, and its closures' on it."""
    calls = []
    for name, line, args in chain:
        calls.append((name, line))
        calls += closures(args)
    return calls


def groups(seq):
    """Every bracket's contents, the outermost first."""
    yield seq
    for t in seq:
        if t[0] == "g":
            yield from groups(t[2])


def audit(path, text):
    """The file's findings as (line, message)."""
    toks, notes = tokens(text)
    found = {}
    for seq in groups(tree(toks)):
        for _, chain in chains(seq):
            calls = element(chain)
            rounds = [(n, ln) for n, ln in calls if ROUND.match(n) and n != "rounded_none"]
            clips = [(n, ln) for n, ln in calls if n in CLIP]
            if not rounds or not clips:
                continue
            clip, at = min(clips, key=lambda c: c[1])
            if at in found:
                continue
            marks = [m for m in (ALLOW.search(x) for ln in (at - 1, at) for x in notes.get(ln, [])) if m]
            if marks:
                reason = marks[-1].group(1).strip()
                if len(reason.split()) >= 3:
                    continue
                found[at] = f"marks the clip without its reason (\"practices-allow clip: {reason}\"): say why in three words or more."
                continue
            r, rl = rounds[0]
            found[at] = f"rounds (.{r}, line {rl}) and clips (.{clip}) one element: GPUI clips it square."
    return sorted(found.items())


def main(argv):
    findings, broken = [], []
    for path in argv:
        try:
            text = open(path, encoding="utf-8").read()
        except (OSError, UnicodeDecodeError) as e:
            broken.append(f"{path}: {e}")
            continue
        findings += [(path, ln, msg) for ln, msg in audit(path, text)]
    if broken:
        print("check-gpui-clip could not read these, so it can't say they are clean:")
        for b in broken:
            print(f"  - {b}")
    if findings:
        many = "s" if len(findings) > 1 else ""
        print(f"[gpui-clip] {len(findings)} finding{many}. Rule: Rounded corners don't clip (.practices/docs/rust/gpui.md).")
        print("  Fix: round the child that fills the corner with the parent's radius minus its border, and drop overflow_hidden.")
        print("  Not here? When the clip is needed for something else and every corner child carries its own radius, say so")
        print("  on the clip's line or the one above: // practices-allow clip: <reason of three or more words>.")
        print("  Why: GPUI clips children to a rectangle, never to rounded corners, so a child's square corner shows past the rounding.")
        for path, ln, msg in findings:
            print(f"  - {path}:{ln} {msg}")
    if broken:
        return 2
    return 1 if findings else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
