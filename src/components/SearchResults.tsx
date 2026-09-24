import { categoryLabel } from "../lib/categories";
import { localizedHref } from "../lib/base";
import { t, type Lang } from "../lib/i18n";
import type { PartSummary } from "../lib/types";

function existenceBadgeClass(status: PartSummary["existence"]): string {
  switch (status) {
    case "verified":
    case "historical_verified":
      return "badge badge-good";
    case "probable":
      return "badge badge-warn";
    case "rejected":
      return "badge badge-bad";
    default:
      return "badge badge-neutral";
  }
}

function lifecycleBadgeClass(status: PartSummary["lifecycle"]): string {
  switch (status) {
    case "active":
    case "pre_release":
      return "badge badge-good";
    case "nrnd":
    case "eol_announced":
      return "badge badge-warn";
    case "obsolete":
      return "badge badge-bad";
    default:
      return "badge badge-neutral";
  }
}

export default function SearchResults({
  results,
  lang,
}: {
  results: PartSummary[];
  lang: Lang;
}) {
  if (results.length === 0) {
    return <p class="text-muted">{t[lang].search.noResults}</p>;
  }

  return (
    <ul class="card-list">
      {results.map((part) => (
        <li class="card" key={`${part.manufacturer}/${part.mpn}`}>
          <a
            href={localizedHref(
              lang,
              `parts?manufacturer=${encodeURIComponent(part.manufacturer)}&mpn=${encodeURIComponent(part.mpn)}`,
            )}
          >
            <strong>{part.mpn}</strong>
          </a>
          <span class="text-muted"> / {part.manufacturer}</span>
          <div class="result-badges">
            {part.category && (
              <span class="badge badge-neutral">
                {categoryLabel(lang, part.category)}
              </span>
            )}
            <span class={existenceBadgeClass(part.existence)}>
              {part.existence}
            </span>
            <span class={lifecycleBadgeClass(part.lifecycle)}>
              {part.lifecycle}
            </span>
          </div>
        </li>
      ))}
    </ul>
  );
}
