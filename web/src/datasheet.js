// The datasheet viewer: opens a provenance entry's Source at the cited
// page with PDF.js (lazy-loaded) and highlights the locator text on it
// (locator-match.js). The PDF comes from openparts-server's
// `/v1/sources/{id}/document` (publishers don't allow cross-origin
// reads); where that is off or refused, the original link opens
// instead, at `#page=N`.
//
// Any element with `data-op-source` opens it in a side panel when
// clicked: the part page's value cells (cmd/app/view.mbt) and the part
// viewer's dimension popups (crates/viewer/openparts-viewer). Its other
// attributes: data-op-page, data-op-locator, data-op-url, data-op-title,
// data-op-revision, data-op-label (what the value is). The review page
// shows it inline: `openpartsDatasheet.inline(elementId, options)`.
// cmd/app/ffi.mbt's datasheet_* calls this; it sets the UI strings
// (`setLabels`) in the page's language.
import "./review.css";
import { findLocator } from "./locator-match.js";

let labels = {
  close: "Close",
  where: "Where",
  page: "Page",
  revision: "Revision",
  open_original: "Open the original PDF (page {page})",
  loading: "Loading the datasheet…",
  found: "Highlighted “{fragment}” on page {page}.",
  not_found: "The locator text was not found on page {page}; the cited page is shown.",
  no_locator: "No locator recorded; the cited page is shown.",
  no_page: "No page recorded, and the locator text was not found in the document.",
  searching: "No page recorded: searching the document for the locator (page {page} of {pages})…",
  found_search: "No page recorded: found “{fragment}” on page {page}.",
  failed: "The document could not be shown here: {error}",
  prev: "Previous page",
  next: "Next page",
  zoom_in: "Zoom in",
  zoom_out: "Zoom out",
  panel: "Datasheet",
};

const api = () => (globalThis.OPENPARTS_API ?? "/api").replace(/\/$/, "");
const fill = (s, vars) => s.replace(/\{(\w+)\}/g, (_, k) => vars[k] ?? "");
const originalHref = (o) => (o.url ? (o.page ? `${o.url}#page=${o.page}` : o.url) : "");

// PDF.js's legacy build: the modern one needs very recent browsers
// (Uint8Array.prototype.toHex, for one).
let pdfjs;
function loadPdfjs() {
  pdfjs ??= Promise.all([
    import("pdfjs-dist/legacy/build/pdf.min.mjs"),
    import("pdfjs-dist/legacy/build/pdf.worker.min.mjs?url"),
  ]).then(([m, worker]) => {
    m.GlobalWorkerOptions.workerSrc = worker.default;
    return m;
  });
  return pdfjs;
}

// Whether the server fetches documents: until it answers, assume so
// (a failure still falls back to the original link).
let documentsEnabled = true;
fetch(`${api()}/v1/reviews/config`)
  .then((r) => (r.ok ? r.json() : null))
  .then((c) => {
    if (c && c.documents_enabled === false) documentsEnabled = false;
  })
  .catch(() => {});

const documents = new Map();
function loadDocument(source) {
  if (!documents.has(source)) {
    const p = (async () => {
      const r = await fetch(`${api()}/v1/sources/${source}/document`);
      if (!r.ok) {
        let message = `${r.status} ${r.statusText}`;
        try {
          message = (await r.json()).error ?? message;
        } catch {}
        throw new Error(message);
      }
      const data = new Uint8Array(await r.arrayBuffer());
      const lib = await loadPdfjs();
      return lib.getDocument({ data, isEvalSupported: false }).promise;
    })();
    p.catch(() => documents.delete(source));
    documents.set(source, p);
  }
  return documents.get(source);
}

const sourceMeta = new Map();
async function loadSourceMeta(source) {
  if (!sourceMeta.has(source)) {
    sourceMeta.set(
      source,
      fetch(`${api()}/v1/sources/${source}`)
        .then((r) => (r.ok ? r.json() : null))
        .catch(() => null),
    );
  }
  return sourceMeta.get(source);
}

function el(tag, attrs = {}, ...children) {
  const e = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (k === "class") e.className = v;
    else if (k.startsWith("on")) e.addEventListener(k.slice(2), v);
    else e.setAttribute(k, v);
  }
  for (const c of children) e.append(c);
  return e;
}

class DatasheetView {
  constructor(root, { onClose } = {}) {
    this.root = root;
    this.zoom = 1;
    this.token = 0;
    const closeButton = onClose
      ? el("button", { type: "button", class: "ds-close", onclick: onClose }, "×")
      : "";
    if (closeButton) closeButton.setAttribute("aria-label", labels.close);
    this.title = el("div", { class: "ds-title" });
    this.meta = el("div", { class: "ds-meta" });
    this.label = el("div", { class: "ds-value" });
    this.locator = el("div", { class: "ds-locator" });
    this.status = el("div", { class: "ds-status", role: "status" });
    this.original = el("a", { class: "ds-original", target: "_blank", rel: "noopener noreferrer" });
    this.pageLabel = el("span", { class: "ds-page-label" });
    this.prev = el("button", { type: "button", class: "ds-nav", onclick: () => this.go(-1) }, "‹");
    this.next = el("button", { type: "button", class: "ds-nav", onclick: () => this.go(1) }, "›");
    const zoomOut = el("button", { type: "button", class: "ds-nav", onclick: () => this.setZoom(this.zoom / 1.25) }, "−");
    const zoomIn = el("button", { type: "button", class: "ds-nav", onclick: () => this.setZoom(this.zoom * 1.25) }, "+");
    this.buttons = { zoomOut, zoomIn };
    this.canvas = el("canvas", { class: "ds-canvas" });
    this.layer = el("div", { class: "ds-highlights" });
    this.pageBox = el("div", { class: "ds-page" }, this.canvas, this.layer);
    this.scroll = el("div", { class: "ds-scroll" }, this.pageBox);
    root.replaceChildren(
      el(
        "div",
        { class: "ds-panel" },
        el("div", { class: "ds-head" }, el("div", {}, this.title, this.meta), closeButton),
        this.label,
        this.locator,
        this.status,
        el("div", { class: "ds-toolbar" }, this.prev, this.pageLabel, this.next, zoomOut, zoomIn, this.original),
        this.scroll,
      ),
    );
    this.applyLabels();
  }

  applyLabels() {
    this.prev.title = labels.prev;
    this.next.title = labels.next;
    this.buttons.zoomOut.title = labels.zoom_out;
    this.buttons.zoomIn.title = labels.zoom_in;
  }

  async show(o) {
    const token = ++this.token;
    this.o = o;
    this.doc = null;
    this.pageNumber = o.page || 1;
    this.title.textContent = o.title || o.source;
    this.setMeta(o);
    if (!o.title || !o.revision) {
      loadSourceMeta(o.source).then((s) => {
        if (!s || token !== this.token) return;
        o.title ||= `${s.publisher.toUpperCase()} ${s.document_id}`;
        o.revision ||= s.revision;
        o.date ||= s.date;
        o.url ||= s.url;
        this.title.textContent = o.title;
        this.setMeta(o);
      });
    }
    this.label.textContent = o.label || "";
    this.label.hidden = !o.label;
    this.locator.replaceChildren();
    if (o.locator) {
      this.locator.append(el("span", { class: "ds-key" }, labels.where), " ", el("q", {}, o.locator));
    }
    this.layer.replaceChildren();
    this.status.className = "ds-status";
    this.status.textContent = labels.loading;
    const ctx = this.canvas.getContext("2d");
    ctx.clearRect(0, 0, this.canvas.width, this.canvas.height);
    try {
      const doc = await loadDocument(o.source);
      if (token !== this.token) return;
      this.doc = doc;
      // No page recorded: look for the locator in the whole document.
      this.target = o.page || 0;
      if (!o.page && o.locator) {
        this.target = await this.search(token);
        if (token !== this.token) return;
        if (this.target) {
          this.pageNumber = this.target;
          this.original.href = originalHref({ ...o, page: this.target });
          this.original.textContent = fill(labels.open_original, { page: this.target });
        }
      }
      this.pageNumber = Math.min(Math.max(1, this.pageNumber), doc.numPages);
      await this.render(token);
    } catch (e) {
      if (token !== this.token) return;
      this.status.className = "ds-status ds-status-error";
      this.status.replaceChildren(fill(labels.failed, { error: String(e?.message ?? e) }), " ");
      const href = originalHref(o);
      if (href) {
        this.status.append(
          el("a", { href, target: "_blank", rel: "noopener noreferrer" }, fill(labels.open_original, { page: o.page ?? 1 })),
        );
      }
      this.pageLabel.textContent = "";
    }
  }

  /// The page where the locator matches best (a specific fragment, not
  /// just the value's numbers), or 0.
  async search(token) {
    const o = this.o;
    const doc = this.doc;
    const pages = Math.min(doc.numPages, 800);
    let best = { page: 0, score: 0 };
    for (let n = 1; n <= pages; n++) {
      if (n % 10 === 1) this.status.textContent = fill(labels.searching, { page: n, pages });
      const page = await doc.getPage(n);
      const content = await page.getTextContent();
      if (token !== this.token) return 0;
      const items = content.items.filter((i) => typeof i.str === "string");
      const m = findLocator(items, o.locator, []);
      if (m && m.specific && m.score > best.score) best = { page: n, score: m.score };
    }
    return best.page;
  }

  setMeta(o) {
    const parts = [];
    if (o.revision) parts.push(`${labels.revision} ${o.revision}`);
    if (o.date) parts.push(o.date);
    if (o.page) parts.push(`${labels.page} ${o.page}`);
    this.meta.textContent = parts.join(" · ");
    const href = originalHref(o);
    this.original.hidden = !href;
    this.original.href = href || "#";
    this.original.textContent = fill(labels.open_original, { page: o.page ?? 1 });
  }

  go(delta) {
    if (!this.doc) return;
    const n = this.pageNumber + delta;
    if (n < 1 || n > this.doc.numPages) return;
    this.pageNumber = n;
    this.render(++this.token);
  }

  setZoom(z) {
    this.zoom = Math.min(4, Math.max(0.5, z));
    if (this.doc) this.render(++this.token);
  }

  async render(token) {
    const doc = this.doc;
    const o = this.o;
    const page = await doc.getPage(this.pageNumber);
    if (token !== this.token) return;
    const base = page.getViewport({ scale: 1 });
    const width = Math.max(240, this.scroll.clientWidth - 16);
    const viewport = page.getViewport({ scale: (width / base.width) * this.zoom });
    const ratio = window.devicePixelRatio || 1;
    this.canvas.width = Math.floor(viewport.width * ratio);
    this.canvas.height = Math.floor(viewport.height * ratio);
    this.canvas.style.width = `${viewport.width}px`;
    this.canvas.style.height = `${viewport.height}px`;
    this.pageBox.style.width = `${viewport.width}px`;
    this.pageBox.style.height = `${viewport.height}px`;
    this.layer.replaceChildren();
    this.pageLabel.textContent = `${labels.page} ${this.pageNumber} / ${doc.numPages}`;
    this.prev.disabled = this.pageNumber <= 1;
    this.next.disabled = this.pageNumber >= doc.numPages;
    await page.render({
      canvas: this.canvas,
      viewport,
      transform: ratio === 1 ? undefined : [ratio, 0, 0, ratio, 0, 0],
    }).promise;
    if (token !== this.token) return;
    if (!o.locator && this.pageNumber === o.page) {
      this.status.className = "ds-status";
      this.status.textContent = labels.no_locator;
      return;
    }
    if (!this.target || this.pageNumber !== this.target) {
      const missing = !this.target && o.locator;
      this.status.className = missing ? "ds-status ds-status-miss" : "ds-status";
      this.status.textContent = missing ? labels.no_page : "";
      return;
    }
    const content = await page.getTextContent();
    if (token !== this.token) return;
    const items = content.items.filter((i) => typeof i.str === "string");
    const match = findLocator(items, o.locator, o.value ? [o.value] : []);
    if (!match) {
      this.status.className = "ds-status ds-status-miss";
      this.status.textContent = fill(labels.not_found, { page: this.pageNumber });
      return;
    }
    const lib = await loadPdfjs();
    let first = null;
    const draw = (ranges, cls) => {
      for (const [i, from, to] of ranges) {
        const it = items[i];
        const n = match.itemLengths[i] || 1;
        const tx = lib.Util.transform(viewport.transform, it.transform);
        const h = Math.hypot(tx[2], tx[3]);
        const w = it.width * viewport.scale;
        const box = el("div", { class: cls });
        const x = tx[4] + (w * from) / n;
        const y = tx[5] - h;
        box.style.left = `${x - 2}px`;
        box.style.top = `${y - 2}px`;
        box.style.width = `${Math.max(4, (w * (to - from)) / n) + 4}px`;
        box.style.height = `${h * 1.2 + 4}px`;
        this.layer.append(box);
        first ??= { x, y };
      }
    };
    draw(match.primary, "ds-hl ds-hl-primary");
    const firstPrimary = first;
    draw(match.secondary, "ds-hl ds-hl-secondary");
    this.status.className = "ds-status ds-status-found";
    this.status.textContent = fill(o.page ? labels.found : labels.found_search, {
      fragment: match.fragment,
      page: this.pageNumber,
    });
    if (firstPrimary) {
      this.scroll.scrollTo({
        top: Math.max(0, firstPrimary.y - this.scroll.clientHeight / 3),
        left: Math.max(0, firstPrimary.x - this.scroll.clientWidth / 3),
        behavior: "smooth",
      });
    }
  }
}

// The side panel (one, created on first use).
let drawer;
function openDrawer(o) {
  if (!drawer) {
    const root = el("aside", { class: "ds-drawer", role: "dialog", "aria-label": labels.panel });
    document.body.append(root);
    const close = () => {
      root.classList.remove("ds-open");
      drawer.view.token++;
    };
    drawer = { root, view: new DatasheetView(root, { onClose: close }), close };
    window.addEventListener("keydown", (e) => {
      if (e.key === "Escape" && root.classList.contains("ds-open")) close();
    });
  }
  drawer.root.classList.add("ds-open");
  drawer.view.show(o);
}

function optionsOf(e) {
  const d = e.dataset;
  return {
    source: d.opSource,
    page: d.opPage ? Number(d.opPage) : undefined,
    locator: d.opLocator || "",
    url: d.opUrl || e.getAttribute("href") || "",
    title: d.opTitle || "",
    revision: d.opRevision || "",
    label: d.opLabel || "",
    value: d.opValue || "",
  };
}

window.addEventListener(
  "click",
  (e) => {
    if (e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
    const target = e.target instanceof Element ? e.target.closest("[data-op-source]") : null;
    if (!target) return;
    const o = optionsOf(target);
    if (o.url) o.url = o.url.replace(/#page=\d+$/, "");
    e.preventDefault();
    e.stopPropagation();
    if (!documentsEnabled) {
      const href = originalHref(o);
      if (href) window.open(href, "_blank", "noopener,noreferrer");
      return;
    }
    openDrawer(o);
  },
  true,
);

globalThis.openpartsDatasheet = {
  setLabels(json) {
    try {
      labels = { ...labels, ...JSON.parse(json) };
    } catch {}
    drawer?.view.applyLabels();
  },
  /// Shows `options` (JSON) in the element `id`, once it is rendered.
  inline(id, json) {
    const o = JSON.parse(json);
    let n = 0;
    const tick = () => {
      const root = document.getElementById(id);
      if (!root) {
        if (++n < 300) requestAnimationFrame(tick);
        return;
      }
      root._datasheet ??= new DatasheetView(root);
      root._datasheet.show(o);
    };
    tick();
  },
  open(json) {
    openDrawer(JSON.parse(json));
  },
  close() {
    drawer?.close();
  },
};
