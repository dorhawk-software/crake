// Czech: the grammar between words that a script can decide
// (common/voice.md, "Each language's grammar is checked"). check-copy runs
// these on Czech copy only: <html lang="cs">, cs.json, values-cs/, cs.lproj/,
// a `cs: {` block. Each rule: what it checks, how to fix it, where to look
// (find, a global RegExp), and the check of one match (a message, or "" when
// the match is right).
//
// Why: Scops said "Ve 10:20 ho vzbuďte" (2026-11-16). A time is read aloud,
// "v deset dvacet", so it is "V 10:20"; "ve čtrnáct deset" is "ve 14:10".
// The owner caught it on a screenshot; no check could.

const W = "(?<![\\p{L}\\p{N}])"; // a word starts here
const GAP = "[\\s\\u00a0]+"; // a space, a non-breaking one too
const as = (said, want) => (said[0] === said[0].toUpperCase() ? want[0].toUpperCase() + want.slice(1) : want);

// The hours whose spoken word starts with a cluster "v" can't stand before: dvě, tři, čtyři, dvanáct,
// třináct, čtrnáct, dvacet…
const VE_HOURS = new Set([2, 3, 4, 12, 13, 14, 20, 21, 22, 23]);
const SPOKEN = ["nula", "jedna", "dvě", "tři", "čtyři", "pět", "šest", "sedm", "osm", "devět", "deset", "jedenáct", "dvanáct",
  "třináct", "čtrnáct", "patnáct", "šestnáct", "sedmnáct", "osmnáct", "devatenáct", "dvacet", "dvacet jedna", "dvacet dva", "dvacet tři"];

// An abbreviation with no vowel is read letter by letter, and a letter's name can start with a vowel:
// "SMS" is "esemes", "FTP" is "ef té pé", so "z SMS" and "v FTP" are right. One read as a word
// keeps the rule: "ze SÚKL". 1.7.1 asked Obálka for "ze SMS" (2026-11-16).
const spelledFromVowel = (w) => w.length > 1 && w === w.toUpperCase() && !/[AEIOUYÁÉĚÍÓÚŮÝ]/u.test(w) && /^[SF]/u.test(w);

export const rules = [
  {
    id: "v-ve-time",
    what: "\"v\" or \"ve\" before a time follows how the hour is said",
    fix: "Write \"ve\" before 2, 3, 4, 12, 13, 14 and 20 to 23 o'clock, \"v\" before the rest: \"V 10:20\", \"ve 14:10\".",
    find: new RegExp(`${W}([Vv][Ee]?)${GAP}([01]?\\d|2[0-3])[:.][0-5]\\d(?!\\d)`, "gu"),
    check: (m) => {
      const h = +m[2], want = VE_HOURS.has(h) ? "ve" : "v";
      return m[1].toLowerCase() === want ? "" : `"${m[0]}": the hour is said "${SPOKEN[h]}", so "${as(m[1], want)} ${m[0].slice(m[1].length).trim()}".`;
    },
  },
  {
    id: "v-ve-day",
    what: "\"ve středu\", \"ve čtvrtek\"; \"v\" before the other days",
    fix: "Write \"ve středu\" and \"ve čtvrtek\"; \"v pondělí\", \"v úterý\", \"v pátek\", \"v sobotu\", \"v neděli\".",
    find: new RegExp(`${W}([Vv][Ee]?)${GAP}(pondělí|úterý|středu|čtvrtek|pátek|sobotu|neděli)(?![\\p{L}])`, "giu"),
    check: (m) => {
      const want = /^(středu|čtvrtek)$/i.test(m[2]) ? "ve" : "v";
      return m[1].toLowerCase() === want ? "" : `"${m[0]}" is "${as(m[1], want)} ${m[2]}".`;
    },
  },
  {
    id: "v-before-v",
    what: "\"ve\" before a word that starts with v or f",
    fix: "Write \"ve\": \"ve vaně\", \"ve fázi\".",
    find: new RegExp(`${W}([Vv])${GAP}([VvFf][\\p{L}]*)`, "gu"),
    check: (m) => (spelledFromVowel(m[2]) ? "" : `"${m[0]}" is "${as(m[1], "ve")} ${m[2]}".`),
  },
  {
    id: "s-z-before-sibilant",
    what: "\"se\" and \"ze\" before a word that starts with s, z, š or ž",
    fix: "Write \"se\" or \"ze\": \"se sestrou\", \"ze školy\".",
    find: new RegExp(`${W}([SsZz])${GAP}([SsZzŠšŽž][\\p{L}]*)`, "gu"),
    check: (m) => (spelledFromVowel(m[2]) ? "" : `"${m[0]}" is "${as(m[1], m[1].toLowerCase() + "e")} ${m[2]}".`),
  },
  {
    id: "k-before-k",
    what: "\"ke\" before a word that starts with k or g",
    fix: "Write \"ke\": \"ke kočárku\", \"ke Gabriele\".",
    find: new RegExp(`${W}([Kk])${GAP}([KkGg][\\p{L}]*)`, "gu"),
    check: (m) => `"${m[0]}" is "${as(m[1], "ke")} ${m[2]}".`,
  },
  {
    // Czech typography (ČSN 01 6910): a one-letter preposition or conjunction never ends a line,
    // so it is tied to the next word with a no-break space. Scops' blind page broke "…často s /
    // plačícím dítětem" (2026-11-22, a reviewer's crop); copy checked by eye at one width breaks at
    // another, so the copy carries the tie, whatever the width.
    id: "one-letter-end",
    what: "a one-letter preposition or conjunction (k, s, v, z, o, u, a, i) never ends a line",
    fix: "Tie it to the next word with a no-break space: U+00A0, &nbsp; in HTML, \\u00a0 in code (\"s\u00a0plačícím\").",
    find: new RegExp(`${W}([KkSsVvZzOoUuAaIi])[ \\t\\r\\n]+(?=([\\p{L}\\p{N}„"(][^\\s]*))`, "gu"),
    check: (m) => `"${m[1]} ${m[2]}" can end a line after "${m[1]}": write "${m[1]}\\u00a0${m[2]}".`,
  },
];

// Buttons whose whole label names no outcome (check-ui outcome, ui/visual-craft.md, "Every button
// names its outcome"): zrušit and storno (cancel), ano (yes), ne (no), potvrdit (confirm), přijmout
// (accept), odmítnout (decline), přerušit (abort). 1.6.1: a Czech board's bare "Zrušit" passed.
export const vagueButtons = ["ok", "okay", "zrušit", "storno", "ano", "ne", "potvrdit", "přijmout", "odmítnout", "přerušit"];
