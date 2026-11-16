#!/usr/bin/env python3
"""check-voice's reading half: finds "we" in the copy people see.

    voice-audit.py [--words REGEX] FILE...

Reads only what people see: string literals in code (not their keys, not
comments), values in JSON, .strings, .xcstrings and .arb files, messages
in .ftl, .po and Android strings.xml, and plain text (store listings).
Each string's language comes from its file's path (cs.json, cs.lproj,
values-cs) or from the block it sits in (`const cs = {`, `cs: '...'`,
`"cs": {`).

Exit 0 clean, 1 findings, 2 a file it couldn't read.
Why: common/voice.md, "The product speaks as itself, never as "we"".
"""
import json
import re
import sys

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
NOT_CZECH_VERB = re.compile(r"(?:ome|ame|ime|ume|yme)$|^(?:theme|scheme|extreme|supreme|meme|acme)$", re.I)
QUOTES_OPEN = "\"“„'‘‚«‹"
QUOTES_CLOSE = "\"”“'’‘»›"

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


def from_json(text, path):
    out = []
    data = json.loads(text)

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


def from_lines(text, path):
    """.strings, .ftl, .po, strings.xml and plain text: the message part of each line."""
    out, fenced, in_comment = [], False, False
    for ln, line in enumerate(text.split("\n"), 1):
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
        out.append((ln, s, None))
    return out


ALLOW = re.compile(r"practices-allow\s+voice\s*:\s*(.*)")


def main(argv):
    extra = None
    if argv[:1] == ["--words"]:
        extra, argv = argv[1], argv[2:]
    everywhere = word_re(EVERYWHERE + ([extra] if extra else []))
    czech = word_re(CZECH)
    findings, broken = [], []
    for path in argv:
        try:
            text = open(path, encoding="utf-8").read()
            if path.endswith(JSONISH):
                strings = from_json(text, path)
            elif path.endswith(CODE):
                strings = from_code(text, path)
                if path.endswith((".tsx", ".jsx")):
                    strings += from_jsx(text, path)
            else:
                strings = from_lines(text, path)
        except (OSError, UnicodeDecodeError, ValueError) as e:
            broken.append(f"{path}: {e}")
            continue
        lines = text.split("\n")
        fixed = path_lang(path)
        for ln, s, lang in strings:
            lang = fixed or lang
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
            if marks:
                reason = re.sub(r"\s*(\*/|-->)\s*$", "", marks[-1].group(1)).strip()
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
