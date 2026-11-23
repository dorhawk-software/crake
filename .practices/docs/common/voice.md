# The product's voice

Everything people read from the product (its screens, notifications,
store listing, website and changelog) speaks as the product, or names
who acts. How a screen's wording reads is in
[ui/visual-craft.md](../ui/visual-craft.md#words-in-the-products-voice);
this practice holds who speaks, and the grammar each language checks
between its words.

## The product speaks as itself, never as "we"

**What.** The app's copy says who does each thing:
- the app ("Aplikace pošle technický popis chyby", "the app sends a
  description of the fault");
- the people behind it, by role ("ani vývojáři aplikace", "not even
  the developers");
- the service that really does it ("ISDS sends the code"); or
- nobody, impersonally ("Kód vám přišel v SMS", "Nic odpovídajícího se
  nenašlo", "Nothing matched").

Never an exclusive "we", "us" or "our": the app, the company or a server
acting. Never "I". In languages that carry the person in the verb, that
includes the verb alone: Czech "upozorníme", "odešleme", "jsme poslali"
all say "we".

One "we" may stay: the inclusive one, in a product built as a companion.
A companion goes through something hard with the person. Its "we" means
the two of them together, never the app acting. Scops, a sleep-training
companion, has its owl say "Pojďme na to" (let's do this) to a tired
parent.

**Why.** A data-box app (2026-11-15) said "Kód jsme vám poslali v SMS"
(we sent you the code by SMS). The state's system sent it, not the app:
"we" claimed an act the app never did. Its telemetry line said "budeme
posílat" (we will send) and its backup-password line "nikdo neotevře, ani
my" (nobody opens it, not even us). Who is "we" there: the app on the
phone, or people with a server? For a promise about data, that question
is the whole promise. One voice says it plainly, and a reader never has to
guess who is acting.

`check-voice` flagged 35 Czech lines on the Scops boards (Scops,
2026-11-19). Most were right. "Přečteme jen čas a název akcí…" (we read
only the events' times and names) hid who acts in a promise about data.
It became "Aplikace přečte jen…" (the app reads only…). "Pojďme na to"
on the welcome screen is the opposite case. Výreček, the owl, goes
through hard nights with the parent: you aren't alone in this. Naming
one actor removes what the line is for. The product's owner, translated:
"let's go, because you're not alone in this; I, the owl, am here with
you, and together we'll manage."

**Apply.**
1. Ask who really acts: the app on this device, the developers, an
   outside service, or the person. Name that one.
2. When nobody needs naming, say it impersonally: what happened, not who
   did it.
3. A promise about data names the people it is about ("the developers
   never see it"), never "we" and "us". No exception, not even in a
   companion's voice.
4. Words the person says themselves (a checkbox "Save my work first", a
   consent "I agree") are their voice, not the product's: those keep "my"
   and "I". A button keeps the person's "we" too: "Jdeme si hrát" (we're
   going to play), "Teď nic pevného nemáme" (we have nothing fixed now).
   The gate flags a Czech one, so mark it: `practices-allow voice: the
   parent's own words on a button`.
5. In a product built as a companion, its inclusive "we" stays. Ask
   first: does "we" mean the companion and the person, doing it together?
   If it could be read as the app acting, it is exclusive: name who acts.
   Mark each inclusive line, on its line or the line above:
   `practices-allow voice: inclusive we, <who> together`. Scops marks
   the owl's lines with the parent:
   - "Pojďme na to" (let's do this);
   - "Dnes zkusíme oba spánky…" (today we'll try both naps…);
   - "Zítra pokračujeme podle plánu" (tomorrow we keep to the plan).
6. A real quote of someone speaking keeps its words. Mark it, with its
   reason, in a comment on the same line or the line above:
   `practices-allow voice: a quote from the statute`. In an Xcode string
   catalog (`.xcstrings`) the mark goes in the entry's comment, which
   Xcode keeps: "Quote by Seneca. practices-allow voice: a real quote,
   attributed on screen".

> Open (2026-11-19): may a named companion say "I" as itself (the owl's
> "já")? Undecided. Scops avoids it for now: "ukážu vám" (I'll show you)
> became "uvidíte" (you'll see).

**Check.** `check-voice` reads every string people see (`VOICE_FILES`:
the project's strings files, and the code files that hold its copy) and
flags "we" in each language it knows, in any case ("WE"): English we,
us, our and their contractions; Czech my, nás, náš and their forms, jsme,
budeme, and the first person plural verb (a word ending in -me, but not
-ome, -ame or -ime, which are English: "Chrome", "Home"). It reads string
literals, a ternary's branches, the text between JSX tags, and the text of
web pages and design boards with their alt, title, aria-label and
placeholder. Comments, keys, scripts, styles, web addresses, a word quoted
as a word ("the word “we”") and the abbreviation "nám." in an address are
skipped. By default it reads the strings files, the changelog, a
`content/` folder (a FAQ, help pages) and the boards (`UI_BOARDS`).

The Czech verb rules need to know the copy is Czech. A string's language
comes from its path (`cs.json`, `values-cs/`), its block (`cs: {`), on a
page its element's `lang` (`<html lang="cs">`), or in Markdown its front
matter (`lang: cs`). Copy that names none takes `COPY_LANG`, the setting
check-copy reads; without it, only the words every language shares are
flagged. A Czech web page (2026-11-18) said "a říkáme to rovnou" (and we
say so plainly): its path named no language, so only its English twin's
"we" was found.

claude-hook runs it after each edit on the files that changed and
holds the agent's turn while it fails; pre-commit runs it on what is
staged, and lint on everything. "I" stays with the reviewer, because the
person's own words use it too.

An inclusive "we" is flagged like any other; its mark lets it pass. On a
board the mark is an HTML comment, `<!-- practices-allow voice: … -->`,
on the line or the line above. Its reason ("inclusive we, the owl and the
parent together") tells the reviewer which case it claims. Whether a "we"
really is inclusive, and whether the product is a companion, is
judgement: the reviewer decides.

## Each language's grammar is checked

**What.** Copy that is right word by word can still be wrong in its
language, in the grammar between words. Where that grammar is mechanical,
a script checks it. Each language the product ships has its own rules
file, and each piece of copy is checked by the rules of its own language.

**Why.** Scops showed "Ve 10:20 ho vzbuďte" (wake him at 10:20) and "Ve
10:15 vám dáme vědět" (2026-11-16). In Czech the preposition follows how
the next word is said. A time is read aloud, "v deset dvacet", so it is
"V 10:20"; "ve čtrnáct deset" gives "ve 14:10". The owner caught it on a
screenshot. No check could: `check-ui` measures geometry, and copy had
only a reviewer's eye.

**Apply.**
1. Name the language of all copy. A board says `<html lang="cs">` (a
   part in another language, `<span lang="en">`). A catalogue says it in
   its path (`cs.json`, `values-cs/`, `cs.lproj/`) or its block
   (`cs: {`); a Markdown page in its front matter (`lang: cs`). An Xcode
   string catalog's key is its `sourceLanguage` copy when the entry has
   no localization of that language, and a stale entry isn't read. Copy that names none is a finding, never a guess. A
   project whose plain files (its changelog) are in one language sets it
   once: `COPY_LANG=en` in `.practices/config`. In a code catalogue (a
   `.ts` file with `cs` and `en` blocks) the copy is in the blocks; its
   ids, imports and type names outside them are code, not copy.
2. A new language ships with its rules: `.practices/copy-rules/<lang>.mjs`.
   A language with copy and no rules file fails the build, so "we added
   German" means "we added German and its copy rules".
3. A rules file lists rules, each `{ id, what, fix, find, check }`:
   `find` is a global RegExp, and `check(match)` returns a message, or
   `""` when the match is right. It also lists `vagueButtons`, the words
   a button may not say alone ("Zrušit", "Cancel"; `check-ui` outcome
   reads them). The kit's `copy-rules/cs.mjs` is the format.
4. A deliberate exception (a quoted name, a brand) is marked with its
   reason: `data-allow="copy: the product's own name"` on a board's
   element, or `practices-allow copy: <reason>` on a string's line or the
   one above. A mark without a reason of three words is a finding.
5. Run it on the shipped strings, not only on boards: those are what
   people read.

The kit ships three languages:
- **cs**: `v-ve-time` ("ve" before 2, 3, 4, 12, 13, 14 and 20 to 23
  o'clock, "v" before the rest), `v-ve-day` (ve středu, ve čtvrtek, v
  pondělí), `v-before-v` (ve vaně, ve fázi), `s-z-before-sibilant` (se
  sestrou, ze školy), `k-before-k` (ke kočárku), `one-letter-end` (a
  one-letter preposition or conjunction, k, s, v, z, o, u, a, i, is tied
  to the next word with a no-break space, so no line ends on it: a
  review's crop of a Scops page caught "…často s / plačícím dítětem",
  2026-11-22. Copy checked by eye at one width breaks at another, so the
  copy carries the tie).
- **es**: `al-del` ("a el" is "al", "de el" is "del"), `y-e` (padres e
  hijos), `o-u` (siete u ocho).
- **en**: `a-an` (an hour, an SMS, a user, a one-off).

**Check.** `check-copy` reads the boards (`UI_BOARDS`) and the copy
files (`VOICE_FILES`), or `COPY_FILES` when set, and runs each
language's rules on its own copy. A language without rules, and copy
without a language, are findings. It runs where `check-voice` runs:
claude-hook after each edit and before the turn ends, pre-commit, lint
(so CI). Which wordings are wrong but not mechanical stays with the
reviewer.
