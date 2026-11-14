import { useEffect, useState } from "preact/hooks";
import { ApiError, searchParts } from "../lib/api";
import { CATEGORIES, categoryLabel } from "../lib/categories";
import { t, type Lang } from "../lib/i18n";
import type { ComponentCategory, PartSummary } from "../lib/types";
import SearchResults from "./SearchResults";

export default function SearchBox({ lang }: { lang: Lang }) {
  const copy = t[lang].search;
  const [query, setQuery] = useState("");
  const [category, setCategory] = useState<ComponentCategory | null>(null);
  const [results, setResults] = useState<PartSummary[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);

    // Empty query returns every part (server treats an empty needle as a
    // match-all), so this doubles as a browse view.
    searchParts(query, category ?? undefined)
      .then((parts) => {
        if (!cancelled) setResults(parts);
      })
      .catch((err) => {
        if (cancelled) return;
        setResults([]);
        setError(err instanceof ApiError ? err.message : copy.searchFailed);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [query, category]);

  return (
    <div>
      <input
        type="search"
        class="search-input"
        placeholder={copy.placeholder}
        value={query}
        onInput={(e) => setQuery((e.target as HTMLInputElement).value)}
        autofocus
      />

      <div class="category-filter">
        <button
          type="button"
          class={category === null ? "chip chip-active" : "chip"}
          onClick={() => setCategory(null)}
        >
          {copy.all}
        </button>
        {CATEGORIES.map((c) => (
          <button
            type="button"
            key={c}
            class={category === c ? "chip chip-active" : "chip"}
            onClick={() => setCategory(c)}
          >
            {categoryLabel(lang, c)}
          </button>
        ))}
      </div>

      <div class="section">
        {loading && <p class="text-muted">{copy.loading}</p>}
        {error && <p class="text-muted">{error}</p>}
        {!loading && !error && <SearchResults results={results} lang={lang} />}
      </div>
    </div>
  );
}
