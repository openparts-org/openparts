// Finds a provenance locator ("Table 1 – Ratings, row T494C226: Rated
// Cap 22 µF", "MECHANICAL DATA: \"Case: DO-41\"") in a PDF page's text
// (PDF.js text items), so the datasheet viewer can highlight it.
//
// The locator is free text written by whoever cited the page. It is cut
// into fragments: quoted strings, the pieces between separators (a
// locator usually goes from the general -- the table -- to the specific
// -- the row and value), runs of words, and tokens with digits (part
// numbers, symbols, values with units). Each is searched for in the
// page's text with case, Unicode variants (µ/μ, ±, dashes, ligatures)
// and *all whitespace* ignored (PDF text runs split and join words at
// random). The best fragment is a specific, long, rare one: quoted
// strings and part numbers first, later pieces before earlier ones (the
// row before its table's title), fewer occurrences before more. Among
// its occurrences, the one with most other fragments nearby wins; those
// are highlighted too, more lightly. Pure functions: datasheet.js uses
// them in the browser, scripts/locator-hit-rate.mjs in Node.

const GENERIC = new Set([
  "table", "row", "rows", "column", "columns", "page", "figure", "fig", "note", "notes",
  "typ", "typical", "min", "minimum", "max", "maximum", "nom", "nominal", "value", "values",
  "characteristics", "product page", "datasheet", "symbol", "parameter", "parameters",
  "unit", "units", "conditions", "condition", "see", "and", "the", "for", "with", "top view",
  "side view", "bottom view", "front view", "drawing", "title", "header",
]);

/// Folds `s` the way both sides are compared: NFKC, lower case, one
/// spelling of micro/plus-minus/dashes/quotes/degrees, no whitespace.
export function fold(s) {
  return s
    .replace(/º/g, "°")
    .normalize("NFKC")
    .toLowerCase()
    .replace(/[µμ]/g, "u")
    .replace(/±/g, "+/-")
    .replace(/[‐‑‒–—−]/g, "-")
    .replace(/[‘’′]/g, "'")
    .replace(/[“”″]/g, '"')
    .replace(/\s+/g, "");
}

const UNIT = "(?:\\s?(?:[kMGmunpµμ]?(?:Hz|F|V|A|Ω|W|H|ppm|%|°C|ºC|mm|mil|s|dB))(?![A-Za-z]))?";
const TOKEN = new RegExp(`[A-Za-z0-9#+/.\\-]*\\d[A-Za-z0-9#+/.\\-]*${UNIT}`, "g");

/// The fragments of `locator` worth searching for:
/// `{text, folded, weight, kind, general}`, `kind` one of quoted, piece,
/// words, code (a token with letters and digits), number and value;
/// `general` for those from the first of several pieces. `extra` (the
/// fact's value) is searched for too, weakly.
export function locatorFragments(locator, extra = []) {
  const out = [];
  const seen = new Set();
  const add = (text, weight, kind, general = false) => {
    const t = text.trim().replace(/^[\s,;:.()\-–]+|[\s,;:.(\-–]+$/g, "").trim();
    const f = fold(t);
    if (f.length < 3 || GENERIC.has(t.toLowerCase())) return;
    const old = out.find((o) => o.folded === f);
    if (old) {
      old.weight = Math.max(old.weight, weight);
      old.general &&= general;
      return;
    }
    if (seen.has(f)) return;
    seen.add(f);
    out.push({ text: t, folded: f, weight, kind, general });
  };
  const inner = (text, base, general = false) => {
    // Runs of words without digits ("Molded Package Thickness").
    for (const run of text.split(/[\d"“”;,:()=/|]+/)) {
      const r = run.replace(/\s+/g, " ").trim();
      const n = r.split(" ").length;
      if (fold(r).length >= 6 && n >= 2 && n <= 8) add(r, 0.9 * base, "words", general);
    }
    // "Table 5", "Figure 166": a caption.
    for (const m of text.matchAll(/\b(?:Table|Figure|Fig\.)\s*\d+[A-Za-z0-9.\-]*/gi)) {
      add(m[0], 1.5 * base, "code", general);
      // "Table 615-621" (a range of tables): its first.
      const head = m[0].replace(/-\d+$/, "");
      if (head !== m[0]) add(head, 1.2 * base, "code", general);
    }
    for (const m of text.matchAll(TOKEN)) {
      const t = m[0].replace(/[.\-/]+$/, "");
      if (fold(t).length < 4) continue;
      const value = /^[-+]?\d[\d.,]*(\s?[a-zµμΩ%°]{1,4})?$/i.test(t);
      if (!value && /[A-Za-z]/.test(t)) {
        add(t, 2 * base, "code", general);
      } else {
        add(t, 1.1 * base, "number", general);
        // The number alone: tables put the unit in its own column.
        const n = t.match(/^[-+]?\d[\d.,]*/)?.[0] ?? t;
        if (n !== t && fold(n).length >= 3 && /[.,]/.test(n)) add(n, 0.8 * base, "number", general);
      }
    }
  };
  const s = String(locator ?? "");
  for (const m of s.matchAll(/"([^"]{2,})"|“([^”]{2,})”/g)) {
    const q = m[1] ?? m[2];
    add(q, 3, "quoted");
    inner(q, 1.2);
  }
  const unquoted = s.replace(/"[^"]*"|“[^”]*”/g, " ; ");
  const pieces = unquoted
    .split(/[;,:()=]|\s[–—-]\s|\.\s/)
    .map((p) => p.replace(/\b(row|rows|column|col)\b\.?/gi, " ").replace(/\s+/g, " ").trim())
    .filter((p) => fold(p).length >= 3);
  pieces.forEach((piece, i) => {
    // The general first, the specific last: later pieces weigh more.
    const general = pieces.length > 1 && i === 0;
    const w = 1 + (pieces.length > 1 ? (1.5 * i) / (pieces.length - 1) : 0.5);
    const words = piece.split(" ");
    if (words.length <= 12) add(piece, w, "piece", general); // else a sentence
    // A row label without its trailing symbol ("Drain Current ID"):
    // tables put the symbol in a column of its own.
    if (words.length >= 3 && words.length <= 12) add(words.slice(0, -1).join(" "), 0.9 * w, "piece", general);
    inner(piece, w / 1.25, general);
  });
  for (const e of extra) {
    if (!e) continue;
    add(String(e), 0.8, "value");
    // The value's numbers ("0.18 mm (0.13–0.23)" -> 0.18, 0.13, 0.23):
    // a dimension table's row is where they sit together.
    for (const m of String(e).matchAll(/\d+(?:\.\d+)?/g)) {
      if (m[0].includes(".") || m[0].length >= 2) add(m[0], 0.7, "value-number");
    }
  }
  return out;
}

/// The page's searchable text: the folded characters of every item and,
/// per character, its item and offset in that item's folded text.
export function pageIndex(items) {
  let text = "";
  const item = [];
  const offset = [];
  const lengths = [];
  const pos = [];
  items.forEach((it, i) => {
    const f = fold(it.str ?? "");
    lengths.push(f.length);
    const t = it.transform ?? [1, 0, 0, 1, 0, 0];
    pos.push({ x: t[4], y: t[5], h: Math.hypot(t[2], t[3]) || 1 });
    for (let k = 0; k < f.length; k++) {
      item.push(i);
      offset.push(k);
    }
    text += f;
  });
  return { text, item, offset, lengths, pos };
}

function occurrences(text, needle) {
  const out = [];
  for (let i = text.indexOf(needle); i !== -1; i = text.indexOf(needle, i + 1)) out.push(i);
  return out;
}

/// The best place of `locator` on the page, or null:
/// `{fragment, kind, score, primary: [[item, from, to]...], secondary,
/// itemLengths}` -- ranges in each item's folded characters (from
/// inclusive, to exclusive) for drawing. `kind` says what matched
/// (see `locatorFragments`); `specific` is false when only a general
/// fragment (from the first of several pieces -- the table or section
/// title -- or the fact's value) was found.
export function findLocator(items, locator, extra = []) {
  const idx = pageIndex(items);
  if (!idx.text) return null;
  const fragments = locatorFragments(locator, extra);
  const found = [];
  for (const f of fragments) {
    const at = occurrences(idx.text, f.folded);
    // A general fragment (the table's title) is only a fallback: it
    // places the specific ones (cluster below), it isn't the answer.
    const w = f.general ? f.weight * 0.35 : f.weight;
    if (at.length) found.push({ ...f, at, score: (w * Math.min(f.folded.length, 24)) / at.length });
  }
  if (!found.length) return null;
  // Every occurrence of every fragment is a candidate place, worth its
  // own score (twice) plus that of each other fragment found nearby. A
  // dimension table's row -- two or more of the value's numbers within a
  // few characters -- is worth most.
  const near = 300;
  // Two places on one table row: the same baseline, not far apart (PDF
  // text order may run down a table's columns, so this is geometric).
  const sameRow = (a, b) => {
    const p = idx.pos[idx.item[a]];
    const q = idx.pos[idx.item[b]];
    return Math.abs(p.y - q.y) < 0.5 * Math.max(p.h, q.h) && Math.abs(p.x - q.x) < 25 * Math.max(p.h, q.h);
  };
  let best = null;
  for (const f of found) {
    for (const a of f.at.slice(0, 200)) {
      let total = f.score * 2;
      const nums = new Set(f.kind === "value-number" ? [f.folded] : []);
      for (const g of found) {
        if (g === f) continue;
        if (g.at.some((b) => Math.abs(b - a) < near)) total += g.score;
        if (g.kind === "value-number" && g.at.some((b) => sameRow(a, b))) nums.add(g.folded);
      }
      if (nums.size >= 2) total += 15 * nums.size;
      if (!best || total > best.total) best = { f, a, total, nums: nums.size };
    }
  }
  const ranges = (start, len) => {
    const out = [];
    for (let k = start; k < start + len; k++) {
      const i = idx.item[k];
      const last = out[out.length - 1];
      if (last && last[0] === i) last[2] = idx.offset[k] + 1;
      else out.push([i, idx.offset[k], idx.offset[k] + 1]);
    }
    return out;
  };
  const { f, a: bestAt } = best;
  const rowCluster = best.nums >= 2;
  const primary = ranges(bestAt, f.folded.length);
  const secondary = [];
  for (const g of found) {
    if (g === f) continue;
    const inRow = rowCluster && g.kind === "value-number" && g.at.find((b) => sameRow(bestAt, b));
    if (inRow !== undefined && inRow !== false) {
      primary.push(...ranges(inRow, g.folded.length));
      continue;
    }
    const a = g.at.find((b) => Math.abs(b - bestAt) < near);
    if (a !== undefined) secondary.push(...ranges(a, g.folded.length));
  }
  const general = f.general || f.kind === "value" || (f.kind === "value-number" && !rowCluster);
  return {
    fragment: rowCluster && f.kind === "value-number" ? extra.join(" ") : f.text,
    kind: rowCluster ? `${f.kind}+row` : f.kind,
    specific: !general,
    score: best.total,
    primary,
    secondary,
    itemLengths: idx.lengths,
  };
}
