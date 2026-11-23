#!/usr/bin/env python3
"""check-permission-words' reading half: a button that leads to a system
permission prompt says Continue or Next, never "Allow" (mobile/app-review.md).

    permission-audit.py [--verbs REGEX] [--nouns REGEX] FILE...

Reads the strings people see the way check-voice does (voice-audit.py,
beside this file): string literals in code, values in JSON, .strings,
.arb, .po, Android strings.xml. A string catalog (.xcstrings) is read
whole: its keys are the source language's copy, and each translation is
read too. An entry Xcode marked stale ("extractionState": "stale") is
skipped: no code uses it, so no screen shows it. An entry that isn't
marked stale counts, used or not: it ships in the app.

Two findings:
  - a button-like string (the whole string) that is a "grant" verb and a
    permission: "Allow notifications", "Povolit Čas u obrazovky",
    "Permitir acceso a tus fotos", "Turn on location";
  - in a code file that asks for a permission (requestAuthorization,
    requestAccess, PermissionsAndroid.request...), a bare "Allow",
    "Enable", "Povolit", "Permitir".

Exit 0 clean, 1 findings, 2 a file it couldn't read.
Why: App Review refused an iPhone app on 16 Nov 2026 (Guideline 5.1.1(iv))
for an "ALLOW SCREEN TIME ACCESS" button before the Screen Time prompt.
"""
import importlib.util
import json
import os
import re
import sys

sys.dont_write_bytecode = True  # no __pycache__ in the project's .practices/bin
here = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location("voice_audit", os.path.join(here, "voice-audit.py"))
voice = importlib.util.module_from_spec(spec)
spec.loader.exec_module(voice)

# Per language: the verbs that ask for a permission, and what they ask for.
# The words are matched as the whole string (a button), any case.
LANGS = {
    "en": (
        r"allow|grant|enable|turn on|give|permit|authori[sz]e",
        r"(?:push )?notifications?|alerts|location(?: services)?|camera|microphone|mic|photos?|photo library"
        r"|contacts|tracking|bluetooth|health(?: data)?|screen time|calendars?|reminders|motion(?: (?:&|and) fitness)?"
        r"|speech recognition|face id|local network|media library|music|home(?: data)?",
        r"access|permissions?",
    ),
    "cs": (
        r"povol(?:it|te|)|uděl(?:it|te)|zapn(?:out|ěte|i)|d(?:át|ejte)|umožn(?:it|ěte)|poskytn(?:out|ěte)",
        r"oznámení|upozornění|notifikac\w*|poloh\w*|kamer\w*|fotoaparát\w*|mikrofon\w*|fotk\w*|fotografi\w*"
        r"|kontakt\w*|sledování|bluetooth|zdrav\w*|čas\w* u obrazovky|kalendář\w*|připomín\w*|face id",
        r"přístup\w*|oprávnění",
    ),
    "es": (
        r"permit(?:ir|e|a)|habilit(?:ar|a|e)|activ(?:ar|a|e)|conced(?:er|e|a)|otorg(?:ar|a|ue)|dar|da|autoriz(?:ar|a|e)",
        r"(?:las |los |la |el |tus |tu )?(?:notificaciones|alertas|ubicación|cámara|micrófono|fotos|contactos|rastreo"
        r"|seguimiento|bluetooth|salud|tiempo en pantalla|calendarios?|recordatorios|face id)",
        r"(?:el )?acceso|(?:los )?permisos?",
    ),
    "vi": (
        r"cho phép|bật|cấp(?: quyền)?",
        r"thông báo|vị trí|máy ảnh|camera|micrô|ảnh|danh bạ|theo dõi|bluetooth|sức khỏe|thời gian sử dụng|lịch",
        r"(?:quyền )?truy cập|quyền",
    ),
}

# A file that asks the OS for a permission: its bare "Allow" is the button before the prompt.
ASKS = re.compile(
    r"\b(?:requestAuthorization|requestAccess|requestWhenInUseAuthorization|requestAlwaysAuthorization"
    r"|requestTrackingAuthorization|requestFullAccessTo\w+|requestWriteOnlyAccessTo\w+|requestRecordPermission"
    r"|requestPermissions?|requestPermissionsAsync|PermissionsAndroid\.request\w*|RequestPermission|requestNotifications"
    r"|requestMultiple|launchPermissionRequest)\b")

ALLOW = re.compile(r"practices-allow\s+permission\s*:\s*(.*)")
TAIL = r"\s*(?:[.!…]|\.\.\.)?"


def patterns(extra_verbs, extra_nouns):
    whole, bare = [], []
    for code, (verbs, nouns, access) in LANGS.items():
        if extra_verbs:
            verbs += "|" + extra_verbs
        if extra_nouns:
            nouns += "|" + extra_nouns
        det = r"(?:(?:the|your|all|my|a|tvoj\w*|váš|vaš\w*|svoj\w*|tu|tus|su|sus)\s+)?"
        one = r"(?:\S+\s+)?"  # one word between: the app's name ("Allow Acme notifications")
        obj = (rf"{det}{one}(?:{nouns})(?:\s+(?:{access}))?"  # notifications, Screen Time access
               rf"|{one}(?:{access})(?:\s+(?:to|k|ke|a|al|vào)\s+.{{1,40}})?"  # access, access to your photos
               rf"|{one}(?:to|para)\s+(?:send|use|access|read|see|enviar|usar|acceder a)\s+{det}(?:{nouns})")
        whole.append((code, re.compile(rf"(?:{verbs})\s+(?:{obj}){TAIL}", re.I)))
        bare.append(re.compile(rf"(?:{verbs})(?:\s+(?:it|now|all|access|ahora|vše|nyní))?{TAIL}", re.I))
    return whole, bare


def norm(s):
    s = re.sub(r"\{[^{}]*\}|%\d*\$?[@sdif]|\\\(.*?\)", "X", s)  # placeholders are one word
    return " ".join(s.replace(" ", " ").split())


def catalog(text, path):
    """An .xcstrings file: (line, string, lang, allow-or-None) for each key and translation, and the stale count."""
    data = json.loads(text)
    source = data.get("sourceLanguage", "en")
    out, stale = [], 0

    def values(v):
        if isinstance(v, dict):
            unit = v.get("stringUnit")
            if isinstance(unit, dict) and isinstance(unit.get("value"), str):
                yield unit["value"]
            for k, x in v.items():
                if k != "stringUnit":
                    yield from values(x)
        elif isinstance(v, list):
            for x in v:
                yield from values(x)

    for key, entry in data.get("strings", {}).items():
        entry = entry or {}
        if entry.get("extractionState") == "stale":
            stale += 1
            continue
        at = text.find(json.dumps(key, ensure_ascii=False))
        line = voice.line_at(text, at) if at >= 0 else 1
        mark = ALLOW.search(entry.get("comment", "") or "")
        allow = mark.group(1).strip() if mark else None
        locs = entry.get("localizations", {}) or {}
        if source not in locs:
            out.append((line, key, source, allow))
        for lang, loc in locs.items():
            for s in values(loc):
                out.append((line, s, lang.split("-")[0].split("_")[0], allow))
    return out, stale


def main(argv):
    verbs = nouns = None
    while argv[:1] in (["--verbs"], ["--nouns"]):
        if argv[0] == "--verbs":
            verbs = argv[1] or None
        else:
            nouns = argv[1] or None
        argv = argv[2:]
    whole, bare = patterns(verbs, nouns)
    findings, broken, stale_total = [], [], 0
    for path in argv:
        try:
            if path.endswith(".xcstrings"):
                text = open(path, encoding="utf-8").read()
                strings, stale = catalog(text, path)
                stale_total += stale
            else:
                text, read = voice.read(path)
                lines = text.split("\n")
                strings = []
                for ln, s, lang in read:
                    marks = [m for m in (ALLOW.search(x) for x in lines[max(0, ln - 2):ln]) if m]
                    allow = re.sub(r"\s*(\*/|-->)\s*$", "", marks[-1].group(1)).strip() if marks else None
                    strings.append((ln, s, lang, allow))
        except (OSError, UnicodeDecodeError, ValueError) as e:
            broken.append(f"{path}: {e}")
            continue
        asks = path.endswith(voice.CODE) and ASKS.search(text)
        for ln, s, lang, allow in strings:
            said = norm(s)
            if not said or len(said) > 80:
                continue
            why = None
            if any(p.fullmatch(said) for _, p in whole):
                why = "asks for a permission with a grant verb"
            elif asks and said[:1].isupper() and any(p.fullmatch(said) for p in bare):
                why = "is a grant verb in a file that asks the OS for a permission"
            if not why:
                continue
            if allow is not None:
                if len(allow.split()) < 3:
                    findings.append((path, ln, f"marks an exception without its reason (\"practices-allow permission: {allow}\"): say why in three words or more."))
                continue
            findings.append((path, ln, f"{'[' + lang + '] ' if lang else ''}\"{said}\" {why}."))
    if broken:
        print("check-permission-words could not read these, so it can't say they are clean:")
        for b in broken:
            print(f"  - {b}")
    if findings:
        n = len(findings)
        print(f"\n[permission] {n} finding{'s' if n > 1 else ''}. Rule: A button that leads to a permission prompt says Continue (.practices/docs/mobile/app-review.md).")
        print("  Fix: the button says \"Continue\" or \"Next\" (\"Pokračovat\", \"Continuar\", \"Tiếp tục\"); the line above it says why the app needs the permission. A catalog entry no code uses: delete it.")
        print("  Not the button before a prompt (a step in a guide, a setting's name)? Mark it on its line or the line above, or in the catalog entry's comment: practices-allow permission: <the reason>.")
        print("  Why: App Review refuses \"Allow …\" before the system's own prompt (Guideline 5.1.1(iv)): only the system's prompt may ask. An iPhone app was refused for it on 16 Nov 2026.")
        for path, ln, msg in findings:
            print(f"  - {path}:{ln} {msg}")
    stale_note = f" ({stale_total} stale catalog entr{'ies' if stale_total != 1 else 'y'} skipped)" if stale_total else ""
    print(f"\ncheck-permission-words: {len(argv)} file{'s' if len(argv) != 1 else ''}: {len(findings) or 'no'} finding{'s' if len(findings) != 1 else ''}{stale_note}.")
    return 2 if broken else 1 if findings else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
