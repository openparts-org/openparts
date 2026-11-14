import { t, type Lang } from "../lib/i18n";
import type { PartSource } from "../lib/types";

/** Every document the part's data was read from, with its link. */
export default function SourceList({
  sources,
  lang,
}: {
  sources: PartSource[];
  lang: Lang;
}) {
  const copy = t[lang].part;
  if (sources.length === 0) return <p class="text-muted">{copy.sourcesNone}</p>;
  return (
    <div>
      <p class="text-muted">{copy.sourcesNote}</p>
      <ul class="source-list">
        {sources.map(({ source, cited_by }) => (
          <li key={source.id}>
            <a href={source.url} target="_blank" rel="noopener noreferrer">
              {source.publisher.toUpperCase()} {source.document_id}
            </a>{" "}
            <span class="text-muted">
              {source.document_type} · {copy.sourceRevision(source.revision, source.date)}
              {source.retrieved ? ` · ${copy.sourceRetrieved(source.retrieved)}` : ""}
              {" · "}
              {copy.sourceCitedBy}: {cited_by.map((c) => copy.citedBy[c] ?? c).join(", ")}
            </span>
          </li>
        ))}
      </ul>
    </div>
  );
}
