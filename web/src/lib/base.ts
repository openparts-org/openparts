import type { Lang } from "./i18n";

// GitHub Pages project sites are served under a subpath
// (https://openparts-org.github.io/openparts-web/), configured as
// `base` in astro.config.mjs. Internal links must be prefixed with it,
// or they resolve against the domain root instead of the deployed site.
// import.meta.env.BASE_URL does not reliably carry a trailing slash
// (observed as "/openparts-web" -- verified in the built output), so
// join explicitly rather than assuming one.
const BASE = import.meta.env.BASE_URL.replace(/\/$/, "");

export function withBase(path: string): string {
  return `${BASE}/${path.replace(/^\//, "")}`;
}

// English pages live under /en/*; Japanese (default) pages are
// unprefixed. Combines with the base-path prefix above.
export function localizedHref(lang: Lang, path: string): string {
  const clean = path.replace(/^\//, "");
  return withBase(lang === "en" ? `en/${clean}` : clean);
}
