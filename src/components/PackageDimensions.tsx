import type { Dimension, Package } from "../lib/types";

function formatDimension(dim?: Dimension): string {
  if (!dim) return "-";
  if (dim.nominal !== undefined) return `${dim.nominal} ${dim.unit}`;
  if (dim.min !== undefined && dim.max !== undefined)
    return `${dim.min}–${dim.max} ${dim.unit}`;
  if (dim.min !== undefined) return `≥${dim.min} ${dim.unit}`;
  if (dim.max !== undefined) return `≤${dim.max} ${dim.unit}`;
  return "-";
}

export default function PackageDimensions({ pkg }: { pkg: Package }) {
  const rows: Array<[string, string]> = [
    ["ファミリー", pkg.family],
    ["リード数", String(pkg.lead_count)],
    ["ピッチ", formatDimension(pkg.pitch)],
    ["本体幅", formatDimension(pkg.dimensions.body_width)],
    ["本体長", formatDimension(pkg.dimensions.body_length)],
    ["本体高さ", formatDimension(pkg.dimensions.body_height)],
  ];

  return (
    <div>
      <h3>パッケージ ({pkg.id})</h3>
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
