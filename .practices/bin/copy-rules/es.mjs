// Spanish: the grammar between words that a script can decide
// (common/voice.md, "Each language's grammar is checked"). The format is
// cs.mjs's; check-copy runs these on Spanish copy only (<html lang="es">,
// es.json, values-es/, es.lproj/, an `es: {` block).

const W = "(?<![\\p{L}\\p{N}])";
const GAP = "[\\s\\u00a0]+";
const as = (said, want) => (said[0] === said[0].toUpperCase() ? want[0].toUpperCase() + want.slice(1) : want);

export const rules = [
  {
    id: "al-del",
    what: "\"a el\" is \"al\", \"de el\" is \"del\"",
    fix: "Contract them: \"al parque\", \"del niño\". A name keeps its capital and its article: \"de El Salvador\".",
    find: new RegExp(`${W}([Aa]|[Dd]e)${GAP}el(?![\\p{L}])`, "gu"),
    check: (m) => `"${m[0]}" is "${as(m[1], m[1].toLowerCase() === "a" ? "al" : "del")}".`,
  },
  {
    id: "y-e",
    what: "\"y\" becomes \"e\" before an i sound",
    fix: "Write \"e\" before i- and hi-: \"padres e hijos\"; \"y\" stays before hie- and hia- (\"agua y hielo\").",
    find: new RegExp(`(?<![\\p{L}\\p{N}¿¡])([yY])${GAP}(h?[iíIÍ][\\p{L}]*)`, "gu"),
    check: (m) => (/^h[iíIÍ][aeouáéóú]/i.test(m[2]) ? "" : `"${m[0]}" is "${as(m[1], "e")} ${m[2]}".`),
  },
  {
    id: "o-u",
    what: "\"o\" becomes \"u\" before an o sound",
    fix: "Write \"u\" before o- and ho-: \"siete u ocho\", \"mujer u hombre\".",
    find: new RegExp(`${W}([oO])${GAP}(h?[oóOÓ][\\p{L}]*)`, "gu"),
    check: (m) => `"${m[0]}" is "${as(m[1], "u")} ${m[2]}".`,
  },
];

// Buttons whose whole label names no outcome (check-ui outcome).
export const vagueButtons = ["ok", "aceptar", "cancelar", "sí", "no", "confirmar", "enviar", "rechazar", "proceder", "abortar"];
