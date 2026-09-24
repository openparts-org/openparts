import type { ProvenanceMap } from "../lib/types";

function ProvenanceSection({
  title,
  map,
}: {
  title: string;
  map: ProvenanceMap;
}) {
  const entries = Object.entries(map);
  if (entries.length === 0) return null;

  return (
    <div>
      <h4>{title}</h4>
      <ul>
        {entries.map(([path, evidence]) => (
          <li key={path}>
            <code>{path}</code>:{" "}
            {evidence
              .map((e) =>
                e.page !== undefined ? `${e.source} (p.${e.page})` : e.source,
              )
              .join(", ")}
          </li>
        ))}
      </ul>
    </div>
  );
}

export default function ProvenanceList({
  part,
  device,
  pkg,
}: {
  part: ProvenanceMap;
  device: ProvenanceMap;
  pkg: ProvenanceMap;
}) {
  return (
    <div>
      <h3>Provenance(出典)</h3>
      <ProvenanceSection title="部品" map={part} />
      <ProvenanceSection title="デバイス" map={device} />
      <ProvenanceSection title="パッケージ" map={pkg} />
    </div>
  );
}
