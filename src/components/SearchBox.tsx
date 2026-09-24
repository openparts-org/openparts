import { useEffect, useState } from "preact/hooks";
import { ApiError, searchParts } from "../lib/api";
import type { PartSummary } from "../lib/types";
import SearchResults from "./SearchResults";

export default function SearchBox() {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<PartSummary[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);

    // Empty query returns every part (server treats an empty needle as a
    // match-all), so this doubles as a browse view.
    searchParts(query)
      .then((parts) => {
        if (!cancelled) setResults(parts);
      })
      .catch((err) => {
        if (cancelled) return;
        setResults([]);
        setError(err instanceof ApiError ? err.message : "検索に失敗しました。");
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [query]);

  return (
    <div>
      <input
        type="search"
        class="search-input"
        placeholder="MPNまたはメーカー名で検索(例: nRF52840)"
        value={query}
        onInput={(e) => setQuery((e.target as HTMLInputElement).value)}
        autofocus
      />

      <div class="section">
        {loading && <p class="text-muted">読み込み中...</p>}
        {error && <p class="text-muted">{error}</p>}
        {!loading && !error && <SearchResults results={results} />}
      </div>
    </div>
  );
}
