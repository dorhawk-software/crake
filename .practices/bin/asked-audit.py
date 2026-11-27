#!/usr/bin/env python3
"""check-asked's reading half: did the agent's last message ask the person
something in prose, in a turn that never opened the question dialog?

    asked-audit.py TRANSCRIPT [MESSAGE_FILE]

TRANSCRIPT is Claude Code's session file (JSON lines). MESSAGE_FILE, when
given and not empty, is the turn's last message; otherwise the last text
the assistant wrote in the transcript is read. Exit 0: nothing asked in
prose, or the dialog was used. Exit 1: findings. Exit 2: can't read.
No packages.
"""
import json
import re
import sys

DIALOGS = {"AskUserQuestion"}
RULE = "Feedback is asked in a questionnaire (.practices/docs/common/agent-working-rules.md)"
# A heading or a bold line that hands decisions over.
HANDOVER = re.compile(
    r"\b(decisions?|questions?) for you\b|\bopen (questions?|decisions?)\b|"
    r"\bfor you to decide\b|\bneeds? your (answer|decision|call)\b|\byour call\b",
    re.I,
)
# A request for an answer that carries no question mark.
REQUEST = re.compile(
    r"\b(let me know|say if|say whether|say which|tell me (if|whether|which)|"
    r"which (one )?(do|would) you (want|prefer)|up to you)\b",
    re.I,
)


def turn(path):
    """The dialog calls and the assistant's texts since the person last wrote."""
    tools, texts = [], []
    with open(path, encoding="utf-8") as lines:
        for line in lines:
            line = line.strip()
            if not line:
                continue
            try:
                entry = json.loads(line)
            except ValueError:
                continue
            if entry.get("isSidechain"):
                continue
            content = (entry.get("message") or {}).get("content")
            kind = entry.get("type")
            if kind == "user" and not entry.get("isMeta"):
                blocks = content if isinstance(content, list) else []
                answered = any(isinstance(b, dict) and b.get("type") == "tool_result" for b in blocks)
                if isinstance(content, str) or not answered:
                    tools, texts = [], []  # the person wrote: a new turn starts
            elif kind == "assistant" and isinstance(content, list):
                for block in content:
                    if not isinstance(block, dict):
                        continue
                    if block.get("type") == "tool_use":
                        tools.append(block.get("name", ""))
                    elif block.get("type") == "text" and block.get("text", "").strip():
                        texts.append(block["text"])
    return tools, texts


def prose(text):
    """The message without what it only quotes: code, block quotes, quoted speech."""
    text = re.sub(r"```.*?```", " ", text, flags=re.S)
    text = re.sub(r"`[^`\n]*`", " ", text)
    text = "\n".join(line for line in text.splitlines() if not line.lstrip().startswith(">"))
    text = re.sub(r"\"[^\"\n]*\"|“[^”\n]*”", " ", text)
    return text


def findings(text):
    text = prose(text)
    found = []
    for line in text.splitlines():
        bare = line.strip().strip("#*_ ").strip()
        heading = line.lstrip().startswith("#") or (line.strip().startswith("**") and line.strip().endswith("**"))
        if heading and HANDOVER.search(bare):
            found.append(bare)
    blocks = [b.strip() for b in re.split(r"\n\s*\n", text) if b.strip()]
    last = blocks[-1] if blocks else ""
    questions = [q.strip() for q in re.findall(r"(?:[^.!?\n]|\.(?=\S))*\?", text) if len(q.strip()) > 3]
    closing = [q for q in questions if q in last]
    if closing:
        found.extend(closing)
    elif len(questions) >= 2:
        found.extend(questions)
    for match in REQUEST.finditer(last):
        sentence = re.search(r"(?:[^.!?\n]|\.(?=\S))*" + re.escape(match.group(0)) + r"(?:[^.!?\n]|\.(?=\S))*[.!?]?", last)
        found.append((sentence.group(0) if sentence else match.group(0)).strip())
    seen, unique = set(), []
    for item in found:
        if item not in seen:
            seen.add(item)
            unique.append(item)
    return unique


def main(argv):
    if len(argv) < 2:
        print("usage: asked-audit.py TRANSCRIPT [MESSAGE_FILE]", file=sys.stderr)
        return 2
    try:
        tools, texts = turn(argv[1])
    except OSError as err:
        print(f"check-asked: can't read the transcript {argv[1]}: {err}; the turn's questions are unchecked.")
        return 2
    message = ""
    if len(argv) > 2:
        try:
            with open(argv[2], encoding="utf-8") as given:
                message = given.read()
        except OSError:
            message = ""
    if not message.strip():
        message = texts[-1] if texts else ""
    if not message.strip() or DIALOGS & set(tools):
        return 0
    found = findings(message)
    if not found:
        return 0
    n = len(found)
    print(f"[asked] {n} finding{'' if n == 1 else 's'}. Rule: {RULE}.")
    print(
        "  Fix: Ask in the question dialog (AskUserQuestion): one step per question, two to four options,"
        " the recommended one first, each step standing alone. Then report what was done. A question that"
        " isn't for the person (rhetorical, or quoted) becomes a statement or goes in quotes."
    )
    for item in found:
        print(f"  - the last message asks in prose: {item[:160]}")
    return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
