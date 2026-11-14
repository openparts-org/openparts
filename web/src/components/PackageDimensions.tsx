import { t, type Lang } from "../lib/i18n";
import type { Dimension, Package } from "../lib/types";

function formatDimension(dim?: Dimension | null): string {
  if (!dim) return "-";
  // Unstated limits arrive as null (or are absent).
  if (dim.nominal != null) return `${dim.nominal} ${dim.unit}`;
  if (dim.min != null && dim.max != null) return `${dim.min}–${dim.max} ${dim.unit}`;
  if (dim.min != null) return `≥${dim.min} ${dim.unit}`;
  if (dim.max != null) return `≤${dim.max} ${dim.unit}`;
  return "-";
}

export default function PackageDimensions({
  pkg,
  lang,
}: {
  pkg: Package;
  lang: Lang;
}) {
  const copy = t[lang].part;
  const rows: Array<[string, string]> = [
    [copy.family, pkg.family],
    [copy.leadCount, String(pkg.lead_count)],
    [copy.pitch, formatDimension(pkg.pitch)],
    [copy.bodyWidth, formatDimension(pkg.dimensions.body_width)],
    [copy.bodyLength, formatDimension(pkg.dimensions.body_length)],
    [copy.bodyHeight, formatDimension(pkg.dimensions.body_height)],
  ];

  return (
    <div>
      <h3>{copy.packageHeading(pkg.id)}</h3>
      <table>
        <tbody>
          {rows.map(([label, value]) => (
            <tr key={label}>
              <th>{label}</th>
              <td>{value}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
