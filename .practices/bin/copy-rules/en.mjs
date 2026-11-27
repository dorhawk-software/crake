// English: the grammar between words that a script can decide
// (common/voice.md, "Each language's grammar is checked"). The format is
// cs.mjs's; check-copy runs these on English copy only (<html lang="en">,
// en.json, values-en/, en.lproj/, an `en: {` block, or COPY_LANG=en).

const GAP = "[\\s\\u00a0]+";

// Whether a word is said starting with a vowel sound. Spelling decides, except the words below:
// a "you" (a user, a unique, a European, a one-off) and a silent h (an hour, an honest).
const YOU = /^(?:uni(?!mp|nf|nh|nl|nm|nn|ns|nt|nv|rr)|use|usu|usa|uti|uri|ura|ure|uro|uk\b|ubi|ufo|eu|ewe|one|once)/i;
const SILENT_H = /^(?:hour|honest|honou?r|heir)/i;
const LETTER_VOWEL = "AEFHILMNORSX"; // an SMS, an FAQ, an HTML page
function vowelSound(word) {
  if (/^\d/.test(word)) return /^(?:8|11(?!\d{2})|18(?!\d{2}))/.test(word); // an 8, an 11, an 18
  if (word.length > 1 && word === word.toUpperCase() && /^[A-Z]+$/.test(word.replace(/[-'’]/g, ""))) return LETTER_VOWEL.includes(word[0]);
  if (YOU.test(word)) return false;
  if (SILENT_H.test(word)) return true;
  return /^[aeiouAEIOU]/.test(word);
}

export const rules = [
  {
    id: "a-an",
    what: "\"an\" before a vowel sound, \"a\" before the rest",
    fix: "Write \"an\" where the next word is said with a vowel (an hour, an SMS, an 8), \"a\" otherwise (a user, a one-off, a URL).",
    // "a" lower case anywhere, "A" only where a sentence starts: "Plan A or B" is no article.
    find: new RegExp(`(?:(?<![\\p{L}\\p{N}'’-])(a|an)|(?<=^[\\s\\u00a0"“]*|[.!?:][\\s\\u00a0]+)(A|An))${GAP}([\\p{L}\\p{N}][\\p{L}\\p{N}'’-]*)`, "gu"),
    check: (m) => {
      const said = m[1] || m[2], word = m[3], want = vowelSound(word) ? "an" : "a";
      if (said.toLowerCase() === want) return "";
      return `"${said} ${word}" is "${said[0] === "A" ? want[0].toUpperCase() + want.slice(1) : want} ${word}".`;
    },
  },
];

// Buttons whose whole label names no outcome (check-ui outcome, ui/visual-craft.md, "Every button
// names its outcome").
export const vagueButtons = ["cancel", "ok", "okay", "yes", "no", "confirm", "submit", "accept", "decline", "proceed", "abort", "dismiss"];
