// How often the datasheet viewer finds a provenance locator on its
// cited page: samples facts from a running openparts-server (with
// OPENPARTS_SOURCE_CACHE set), opens each cited page with PDF.js and
// runs src/locator-match.js on it, as the viewer does.
//
//   node scripts/locator-hit-rate.mjs [API base] [sources] [per source]
//   node scripts/locator-hit-rate.mjs http://127.0.0.1:8084 60 4
//
// Prints one line per sampled locator and a summary; only evidence with
// both a page and a locator counts.
import { findLocator } from "../src/locator-match.js";

const api = (process.argv[2] ?? "http://127.0.0.1:8080").replace(/\/$/, "");
const maxSources = Number(process.argv[3] ?? 60);
const perSource = Number(process.argv[4] ?? 4);
const pdfjs = await import("pdfjs-dist/legacy/build/pdf.mjs");

// Every fact, in queue order; then a deterministic spread over sources.
const facts = [];
for (let offset = 0; ; offset += 100) {
  const r = await fetch(`${api}/v1/reviews/queue?status=all&limit=100&offset=${offset}`);
  const q = await r.json();
  facts.push(...q.items.map((i) => i.fact));
  if (offset + 100 >= q.total) break;
}
const bySource = new Map();
for (const f of facts) {
  for (const e of f.evidence) {
    if (!e.page || !e.locator) continue;
    const list = bySource.get(e.source) ?? [];
    if (list.length < perSource && !list.some((x) => x.e.locator === e.locator)) list.push({ f, e });
    bySource.set(e.source, list);
  }
}
const sources = [...bySource.keys()].sort().filter((_, i, all) => i % Math.max(1, Math.floor(all.length / maxSources)) === 0).slice(0, maxSources);

const tally = { specific: 0, general: 0, miss: 0, unavailable: 0, bad_page: 0 };
const misses = [];
for (const source of sources) {
  let doc;
  try {
    const r = await fetch(`${api}/v1/sources/${source}/document`);
    if (!r.ok) throw new Error((await r.json()).error);
    doc = await pdfjs.getDocument({ data: new Uint8Array(await r.arrayBuffer()), isEvalSupported: false, verbosity: 0 }).promise;
  } catch (e) {
    for (const _ of bySource.get(source)) tally.unavailable++;
    console.log(`UNAVAILABLE ${source}: ${String(e.message).slice(0, 100)}`);
    continue;
  }
  for (const { f, e } of bySource.get(source)) {
    if (e.page > doc.numPages) {
      tally.bad_page++;
      console.log(`BAD PAGE  ${source} p${e.page}/${doc.numPages}`);
      continue;
    }
    const page = await doc.getPage(e.page);
    const content = await page.getTextContent();
    const m = findLocator(content.items.filter((i) => typeof i.str === "string"), e.locator, [f.value]);
    if (m) {
      tally[m.specific ? "specific" : "general"]++;
      console.log(`${m.specific ? "HIT " : "SECT"} ${source} p${e.page} «${m.fragment}» (${m.kind}) ← ${e.locator.slice(0, 90)}`);
    } else {
      tally.miss++;
      misses.push(`${source} p${e.page}: ${e.locator}`);
      console.log(`MISS ${source} p${e.page} ← ${e.locator.slice(0, 90)}`);
    }
  }
  await doc.cleanup?.();
}
const shown = tally.specific + tally.general + tally.miss;
const pct = (n) => `${n}/${shown} = ${((100 * n) / Math.max(1, shown)).toFixed(1)}%`;
console.log("\nsources:", sources.length, tally);
console.log(`located on the cited page: ${pct(tally.specific + tally.general)}`);
console.log(`  at a specific fragment (quoted text, code, row/value): ${pct(tally.specific)}`);
console.log(`  at a general one only (the table/section title): ${pct(tally.general)}`);
