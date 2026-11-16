# The product's voice

Everything people read from the product (its screens, notifications,
store listing, website and changelog) speaks as the product, or names
who acts. How a screen's wording reads is in
[ui/visual-craft.md](../ui/visual-craft.md#words-in-the-products-voice);
this practice holds who speaks.

## The product speaks as itself, never as "we"

**What.** The app's copy says who does each thing:
- the app ("Aplikace pošle technický popis chyby", "the app sends a
  description of the fault");
- the people behind it, by role ("ani vývojáři aplikace", "not even
  the developers");
- the service that really does it ("ISDS sends the code"); or
- nobody, impersonally ("Kód vám přišel v SMS", "Nic odpovídajícího se
  nenašlo", "Nothing matched").

Never "we", "us" or "our", and never "I". In languages that carry the
person in the verb, that includes the verb alone: Czech "upozorníme",
"odešleme", "jsme poslali" all say "we".

**Why.** A data-box app (2026-11-15) said "Kód jsme vám poslali v SMS"
(we sent you the code by SMS). The state's system sent it, not the app:
"we" claimed an act the app never did. Its telemetry line said "budeme
posílat" (we will send) and its backup-password line "nikdo neotevře, ani
my" (nobody opens it, not even us). Who is "we" there: the app on the
phone, or people with a server? For a promise about data, that question
is the whole promise. One voice says it plainly, and a reader never has to
guess who is acting.

**Apply.**
1. Ask who really acts: the app on this device, the developers, an
   outside service, or the person. Name that one.
2. When nobody needs naming, say it impersonally: what happened, not who
   did it.
3. A promise about data names the people it is about ("the developers
   never see it"), never "we" and "us".
4. Words the person says themselves (a checkbox "Save my work first", a
   consent "I agree") are their voice, not the product's: those keep "my"
   and "I".
5. A real quote of someone speaking keeps its words. Mark it, with its
   reason, in a comment on the same line or the line above:
   `practices-allow voice: a quote from the statute`.

**Check.** `check-voice` reads every string people see (`VOICE_FILES`:
the project's strings files, and the code files that hold its copy) and
flags "we" in each language it knows, in any case ("WE"): English we,
us, our and their contractions; Czech my, nás, náš and their forms, jsme,
budeme, and the first person plural verb (a word ending in -me, but not
-ome, -ame or -ime, which are English: "Chrome", "Home"). It reads string
literals, a ternary's branches, and the text between JSX tags. Comments,
keys, web addresses, a word quoted as a word ("the word “we”") and the
abbreviation "nám." in an address are skipped. By default it reads the
strings files, the changelog and a `content/` folder (a FAQ, help
pages). claude-hook runs it after each edit on the files that changed and
holds the agent's turn while it fails; pre-commit runs it on what is
staged, and lint on everything. "I" stays with the reviewer, because the
person's own words use it too.
