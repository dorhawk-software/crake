#!/usr/bin/env python3
"""check-voice's reading half: finds "we" in the copy people see.

    voice-audit.py [--words REGEX] FILE...
    voice-audit.py --strings FILE...    every string as JSON, for check-copy

Reads only what people see: string literals in code (not their keys, not
comments), values in JSON, .strings, .xcstrings and .arb files, messages
in .ftl, .po and Android strings.xml, plain text (store listings), and
web pages and design boards (.html): their text and the copy in alt,
title, aria-label and placeholder, never a script, a style or a comment.
Each string's language comes from its file's path (cs.json, cs.lproj,
values-cs), from the block it sits in (`const cs = {`, `cs: '...'`,
`"cs": {`), in a page from the lang of its element (<html lang="cs">,
a <span lang="en">), or in Markdown from its front matter (`lang: cs`). --lang names the language of copy that names none
(COPY_LANG); with none, that copy gets only the words every language
shares.

Exit 0 clean, 1 findings, 2 a file it couldn't read.
Why: common/voice.md, "The product speaks as itself, never as "we"".
"""
import json
import re
import sys
from html.parser import HTMLParser

LANGS = "cs|sk|en|de|fr|es|it|pl|pt|nl|sv|da|fi|nb|uk|ru|hu|ja|zh|ko"

# "we" in any language's text: words no other language spells the same.
EVERYWHERE = [
    r"[Ww]e", r"[Ww]e['’](?:re|ve|ll|d)", r"[Uu]s", r"[Oo]urs?", r"[Oo]urselves",
    r"[Nn]ás", r"[Nn]ám", r"[Nn]ámi", r"[Nn]áš", r"[Nn]aš(?:e|eho|emu|em|ím|i|í|ich|im|imi)",
    r"(?:ne)?[Jj]sme", r"(?:ne)?[Bb]udeme",
]
# Czech only: "my" (in English, the person's own word) and the first
# person plural verb, a word ending in -me ("odešleme", "upozorníme").
CZECH = [r"[Mm]y", r"[^\W\d_]{2,}me"]
# Words ending in -me that are no Czech verb: a Czech "we" ends in -eme,
# -íme, -áme, -jme, -ďme, -ěme..., never in -ome, -ame, -ime, -ume or -yme,
# which are English ("Chrome", "Home", "Same", "Time"); and these -eme ones.
# And the third person singular of vzít and of the -jmout verbs: "nikdo ti
# ho nevezme", "přijme to" say he or she (1.9.1).
NOT_CZECH_VERB = re.compile(r"(?:ome|ame|ime|ume|yme)$|^(?:theme|scheme|extreme|supreme|meme|acme)$"
                            r"|^(?:ne)?(?:pře)?vezme$|^(?:ne)?(?:při|na|za|po|vy|se|obe|ode|do|u)jme$", re.I)
QUOTES_OPEN = "\"“„'‘‚«‹"
QUOTES_CLOSE = "\"”“'’‘»›"

HTML = (".html", ".htm")
CODE = (".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs", ".swift", ".kt", ".kts", ".java", ".rs", ".dart", ".py", ".go", ".cs")
JSONISH = (".json", ".arb", ".xcstrings")


def word_re(words):
    # Any case: "WE NEVER SELL IT" says we too (1.6.1; it read only "We" and "we").
    return re.compile(r"(?<![\w'’-])(" + "|".join(words) + r")(?![\w'’-])", re.IGNORECASE)


def mention(said, m):
    """The word in quotes, as a word ("never write \u201cwe\u201d"): a mention, not the product speaking."""
    a, b = m.start(1), m.end(1)
    return a > 0 and b < len(said) and said[a - 1] in QUOTES_OPEN and said[b] in QUOTES_CLOSE


def square(said, m):
    """Czech "nám." for náměstí in an address ("Mariánské nám. 2", "nám. Míru 5"), not "to us"."""
    a, b = m.start(1), m.end(1)
    if m.group(1).lower() != "nám" or said[b:b + 1] != ".":
        return False
    after = said[b + 1:]
    return bool(re.match(r"\s*\d", after) or re.match(r"\s+[A-ZÁČĎÉĚÍŇÓŘŠŤÚŮÝŽ]\w*\s+\d", after)
                or re.search(r"\w(?:é|ého)\s+$", said[:a]))


def path_lang(path):
    m = re.search(r"(?:^|[/._-])(" + LANGS + r")(?:[-_][A-Za-z]{2,4})?(?:\.lproj)?(?=[/._-]|$)", path)
    return m.group(1) if m else None


def line_at(text, i):
    return text.count("\n", 0, i) + 1


# A string that names a module (`from './strings'`, `require('x')`) or is a member of a type
# (`type Group = 'app' | 'isds'`) is code. 1.7.1 read Obálka's import paths and type names as copy
# in no language, 56 findings in two catalogues.
MODULE = re.compile(r"(?:\bfrom|\bimport|\brequire\s*\(|\bimport\s*\(|\bmock\s*\()\s*$")
TYPE_LITERAL = re.compile(r"(?:(?<!\|)\||\btype\s+\w+(?:<[^=]*>)?\s*=)$")


def from_code(text, path):
    """String literals that aren't keys, outside comments, with the language of their block."""
    out = []  # (line, string, lang, allow-or-None)
    i, n = 0, len(text)
    stack = []  # (depth, lang)
    depth = 0
    hash_comments = path.endswith(".py")
    while i < n:
        c = text[i]
        if text.startswith("//", i) or (hash_comments and c == "#"):
            j = text.find("\n", i)
            i = n if j < 0 else j
            continue
        if text.startswith("/*", i):
            j = text.find("*/", i + 2)
            i = n if j < 0 else j + 2
            continue
        if c == "{":
            before = text[text.rfind("\n", 0, i) + 1:i]
            m = re.search(r"(?:^|[^\w])[\"']?(" + LANGS + r")[\"']?\s*(?::\s*[^={]*)?[=:]\s*$", before)
            depth += 1
            if m:
                stack.append((depth, m.group(1)))
            i += 1
            continue
        if c == "}":
            if stack and stack[-1][0] == depth:
                stack.pop()
            depth -= 1
            i += 1
            continue
        if c in "'\"`":
            start, j, buf = i, i + 1, []
            while j < n and text[j] != c:
                if text[j] == "\\" and j + 1 < n:
                    buf.append(" " if text[j + 1] in "nrt" else text[j + 1])
                    j += 2
                    continue
                if c == "`" and text.startswith("${", j):  # a template's code part isn't copy
                    k, d = j + 2, 1
                    while k < n and d:
                        d += {"{": 1, "}": -1}.get(text[k], 0)
                        k += 1
                    buf.append(" ")
                    j = k
                    continue
                if c != "`" and text[j] == "\n":
                    break
                buf.append(text[j])
                j += 1
            i = j + 1
            rest = text[i:i + 40].lstrip()
            ternary = text[:start].rstrip().endswith("?")  # `k ? 'We…' : …` is copy, not a key
            if rest.startswith(":") and not rest.startswith("::") and not ternary:
                continue  # a key
            said = text[:start].rstrip()
            if MODULE.search(said) or TYPE_LITERAL.search(said):
                continue  # a module's path, or a type's name: code, not copy
            before = text[text.rfind("\n", 0, start) + 1:start]
            m = re.search(r"(?:^|[^\w])[\"']?(" + LANGS + r")[\"']?\s*:\s*$", before)
            lang = m.group(1) if m else (stack[-1][1] if stack else None)
            out.append((line_at(text, start), "".join(buf), lang))
            continue
        i += 1
    return out


# Code between two tags, not text: operators, a comment, the end of a ternary's branch (`) : (`).
JSX_CODE = re.compile(r"&&|\|\||;|=>|[=!<>]=|\breturn\b|//|/\*|\)\s*[:?]\s*\(")


def from_jsx(text, path):
    """Text between JSX tags (`<Text>We sent it.</Text>`): copy that is no string literal."""
    out = []
    for m in re.finditer(r"(?<=[\w\"'}/])>([^<>{}]*)<(?=[A-Za-z/>])", text):
        body = m.group(1)
        line_start = text.rfind("\n", 0, m.start()) + 1
        lead = text[line_start:m.start()].lstrip()
        if not re.search(r"[^\W\d_]{2,}", body) or JSX_CODE.search(body) or lead.startswith(("//", "*", "/*")):
            continue
        opened = text.rfind("<", 0, m.start())  # the tag this > closes: `Array<number>` is a type, not a tag
        if opened > 0 and (text[opened - 1].isalnum() or text[opened - 1] == "_") and text[opened + 1:opened + 2] != "/":
            continue
        out.append((line_at(text, m.start(1) + len(body) - len(body.lstrip())), " ".join(body.split()), None))
    return out


# Voice marks in string catalog entries' comments, by (path, line of each
# string the entry shows): from_catalog fills it, the voice check reads it.
CATALOG_MARKS = {}


def from_json(text, path):
    out = []
    data = json.loads(text)
    if path.endswith(".xcstrings") and isinstance(data, dict) and isinstance(data.get("strings"), dict):
        return from_catalog(text, path, data)

    def walk(v, lang):
        if isinstance(v, dict):
            for k, x in v.items():
                if k in ("comment", "@@locale") or k.startswith("@"):
                    continue
                walk(x, k if re.fullmatch(LANGS + r"(?:[-_][A-Za-z]{2,4})?", k) else lang)
        elif isinstance(v, list):
            for x in v:
                walk(x, lang)
        elif isinstance(v, str):
            at = text.find(json.dumps(v, ensure_ascii=False)[1:-1])
            out.append((line_at(text, at) if at >= 0 else 1, v, lang))

    walk(data, None)
    return [(ln, s, lang.split("-")[0].split("_")[0] if lang else None) for ln, s, lang in out]


def catalog_values(v):
    """The values a catalog localization shows: stringUnit values, in its variations too."""
    if isinstance(v, dict):
        for k, x in v.items():
            if k == "value" and isinstance(x, str):
                yield x
            elif k not in ("comment", "state"):
                yield from catalog_values(x)
    elif isinstance(v, list):
        for x in v:
            yield from catalog_values(x)


def from_catalog(text, path, data):
    """An Xcode string catalog. Its key is the source language's copy when the
    entry has no localization of that language (1.9.1: an app's English was
    never read). A stale entry doesn't ship. A real quote is marked in the
    entry's comment, which Xcode keeps: practices-allow voice: <reason>."""
    src = data.get("sourceLanguage")
    out = []
    for key, entry in data["strings"].items():
        entry = entry if isinstance(entry, dict) else {}
        if entry.get("extractionState") == "stale":
            continue
        at = text.find(json.dumps(key, ensure_ascii=False))
        ln = line_at(text, at) if at >= 0 else 1
        mark = ALLOW.search(entry.get("comment") or "")
        first = len(out)
        locs = entry.get("localizations") if isinstance(entry.get("localizations"), dict) else {}
        if src and src not in locs and key.strip():
            out.append((ln, key, src))
        for lang, unit in locs.items():
            for value in catalog_values(unit):
                found = text.find(json.dumps(value, ensure_ascii=False)[1:-1], max(at, 0))
                out.append((line_at(text, found) if found >= 0 else ln, value, lang))
        if mark:
            for line, _, _ in out[first:]:
                CATALOG_MARKS[(path, line)] = mark.group(1).strip()
    return [(ln, s, lang.split("-")[0].split("_")[0] if lang else None) for ln, s, lang in out]


FRONT_LANG = re.compile(r"^lang:\s*[\"']?([A-Za-z]{2,3})(?:[-_][A-Za-z]{2,4})?[\"']?\s*$")


def front_matter(text):
    """A Markdown file's front matter: (the number of its lines, its lang or None).

    Until 1.7.3 a Markdown page named its language only in its path: a
    website's Czech index.md beside English ones could say neither."""
    lines = text.split("\n")
    if lines[0].strip() != "---":
        return 0, None
    for i, line in enumerate(lines[1:], 2):
        if line.strip() == "---":
            lang = next((m.group(1).lower() for m in map(FRONT_LANG.match, lines[1:i - 1]) if m), None)
            return i, lang
    return 0, None


def from_lines(text, path):
    """.strings, .ftl, .po, strings.xml and plain text: the message part of each line."""
    out, fenced, in_comment = [], False, False
    skip, page_lang = front_matter(text) if path.endswith(".md") else (0, None)
    for ln, line in enumerate(text.split("\n"), 1):
        if ln <= skip:
            continue
        s = line
        if path.endswith(".strings"):
            m = re.match(r'\s*"(?:[^"\\]|\\.)*"\s*=\s*"((?:[^"\\]|\\.)*)"', s)
            s = m.group(1) if m else ""
        elif path.endswith(".ftl"):
            if s.lstrip().startswith("#"):
                continue
            m = re.match(r"\s*-?[\w.-]+\s*=\s*(.*)", s)
            s = m.group(1) if m else (s if s[:1].isspace() else "")
        elif path.endswith((".po", ".pot")):
            m = re.match(r'\s*msgstr(?:\[\d+\])?\s*"(.*)"', s)
            s = m.group(1) if m else ""
        elif path.endswith(".xml"):
            if in_comment or "<!--" in s:
                in_comment = "-->" not in s.split("<!--")[-1]
                s = re.sub(r"<!--.*?(-->|$)", "", s)
            s = re.sub(r"<[^>]+>", " ", s)
        elif path.endswith(".md"):
            if s.startswith("```"):
                fenced = not fenced
                continue
            if fenced:
                continue
            s = re.sub(r"`[^`]*`|<!--.*?-->", " ", s)  # inline code and comments aren't read
        out.append((ln, s, page_lang))
    return out


# Elements that never close, and ones whose content no one reads as text.
VOID = {"area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source", "track", "wbr"}
UNREAD = {"script", "style", "template", "noscript", "svg", "code", "pre"}
SPOKEN = ("alt", "title", "aria-label", "placeholder")


class Page(HTMLParser):
    """A page's text and spoken attributes, each with the lang of the element it sits in.

    Until 1.7.3 a page was read line by line with no language: on a Czech web
    page "a říkáme to rovnou" (and we say so plainly) passed, its English
    twin's "we" did not."""

    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.stack = []  # (tag, lang or None)
        self.out = []  # (line, text, lang)

    def lang(self):
        return next((lang for _, lang in reversed(self.stack) if lang), None)

    def unread(self):
        return any(tag in UNREAD for tag, _ in self.stack)

    def handle_starttag(self, tag, attrs):
        a = dict(attrs)
        lang = (a.get("lang") or "").split("-")[0].split("_")[0].lower() or None
        if not self.unread():
            for k in SPOKEN:
                if a.get(k):
                    self.out.append((self.getpos()[0], a[k], lang or self.lang()))
        if tag not in VOID:
            self.stack.append((tag, lang))

    def handle_startendtag(self, tag, attrs):
        self.handle_starttag(tag, attrs)
        if tag not in VOID:
            self.stack.pop()

    def handle_endtag(self, tag):
        if any(t == tag for t, _ in self.stack):
            while self.stack and self.stack.pop()[0] != tag:
                pass

    def handle_data(self, data):
        if data.strip() and not self.unread():
            self.out.append((self.getpos()[0], " ".join(data.split()), self.lang()))


def from_html(text):
    page = Page()
    page.feed(text)
    page.close()
    return page.out


ALLOW = re.compile(r"practices-allow\s+voice\s*:\s*(.*)")
COPY_ALLOW = re.compile(r"practices-allow\s+copy\s*:\s*(.*)")


def read(path, default=None):
    """The file's text, and its strings as (line, string, language or None)."""
    text = open(path, encoding="utf-8").read()
    if path.endswith(HTML):
        strings = from_html(text)
    elif path.endswith(JSONISH):
        strings = from_json(text, path)
    elif path.endswith(CODE):
        strings = from_code(text, path)
        if path.endswith((".tsx", ".jsx")):
            strings += from_jsx(text, path)
    else:
        strings = from_lines(text, path)
    fixed = path_lang(path)
    return text, [(ln, s, fixed or lang or default) for ln, s, lang in strings]


def dump(argv):
    """--strings: every string as JSON, for check-copy (copy-audit.mjs), which checks each language's grammar."""
    out = {"strings": [], "broken": []}
    for path in argv:
        try:
            text, strings = read(path)
        except (OSError, UnicodeDecodeError, ValueError) as e:
            out["broken"].append(f"{path}: {e}")
            continue
        lines = text.split("\n")
        for ln, s, lang in strings:
            marks = [m for m in (COPY_ALLOW.search(x) for x in lines[max(0, ln - 2):ln]) if m]
            allow = re.sub(r"\s*(\*/|-->)\s*$", "", marks[-1].group(1)).strip() if marks else None
            out["strings"].append({"path": path, "line": ln, "text": s, "lang": lang, "allow": allow})
    json.dump(out, sys.stdout, ensure_ascii=False)
    return 0


def main(argv):
    if argv[:1] == ["--strings"]:
        return dump(argv[1:])
    extra, default = None, None
    while argv[:1] in (["--words"], ["--lang"]):
        if argv[0] == "--words":
            extra = argv[1] or None
        else:
            default = argv[1] or None
        argv = argv[2:]
    everywhere = word_re(EVERYWHERE + ([extra] if extra else []))
    czech = word_re(CZECH)
    findings, broken = [], []
    for path in argv:
        try:
            text, strings = read(path, default)
        except (OSError, UnicodeDecodeError, ValueError) as e:
            broken.append(f"{path}: {e}")
            continue
        lines = text.split("\n")
        for ln, s, lang in strings:
            said = re.sub(r"\{[^{}]*\}|%\d*\$?[@sdif]|\$\w+", " ", s)  # placeholders aren't words
            said = re.sub(r"\b[a-z][a-z0-9+.-]*://\S+|\bwww\.\S+", " ", said)  # nor web addresses (/us/)
            hits = [m.group(1) for m in everywhere.finditer(said)
                    if m.group(1) != "US" and not mention(said, m) and not square(said, m)]
            if lang == "cs":
                hits += [m.group(1) for m in czech.finditer(said)
                         if not mention(said, m) and not NOT_CZECH_VERB.search(m.group(1))]
            if not hits:
                continue
            marks = [m for m in (ALLOW.search(x) for x in lines[max(0, ln - 2):ln]) if m]
            if marks or (path, ln) in CATALOG_MARKS:
                reason = CATALOG_MARKS.get((path, ln)) or re.sub(r"\s*(\*/|-->)\s*$", "", marks[-1].group(1)).strip()
                if len(reason.split()) < 3:
                    findings.append((path, ln, f"marks an exception without its reason (\"practices-allow voice: {reason}\"): say why in three words or more."))
                continue
            short = s.strip().replace("\n", " ")
            short = short if len(short) <= 90 else short[:87] + "…"
            words = ", ".join(dict.fromkeys(f"\"{h}\"" for h in hits))
            findings.append((path, ln, f"{'[' + lang + '] ' if lang else ''}\"{short}\" says {words}."))
    if broken:
        print("check-voice could not read these, so it can't say they are clean:")
        for b in broken:
            print(f"  - {b}")
    if findings:
        print(f"\n[voice] {len(findings)} finding{'s' if len(findings) > 1 else ''}. Rule: The product speaks as itself, never as \"we\" (.practices/docs/common/voice.md).")
        print("  Fix: say who acts: the app (\"Aplikace pošle…\", \"the app sends…\"), the developers, the service that really does it (\"ISDS sends the code\"), or nobody (\"Nic odpovídajícího se nenašlo\", \"Nothing matched\").")
        print("  A real quote? Mark it on its line or the line above: practices-allow voice: <the reason>.")
        print("  Not here? A language it doesn't know: add its we-words to VOICE_WORDS in .practices/config.")
        print("  Why: \"we\" hides who acts. The app on the phone, its developers or an outside service: a reader, and a promise about data, needs to know which.")
        for path, ln, msg in findings:
            print(f"  - {path}:{ln} {msg}")
    print(f"\ncheck-voice: {len(argv)} file{'s' if len(argv) != 1 else ''}: {len(findings) or 'no'} finding{'s' if len(findings) != 1 else ''}.")
    return 2 if broken else 1 if findings else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
