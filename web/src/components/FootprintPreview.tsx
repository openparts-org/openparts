import { useState } from "preact/hooks";
import { artifactDownloadUrl } from "../lib/api";
import { t, type Lang } from "../lib/i18n";

/** The footprint as the server draws it (the same pads KiCad gets). */
export default function FootprintPreview({
  manufacturer,
  mpn,
  lang,
}: {
  manufacturer: string;
  mpn: string;
  lang: Lang;
}) {
  const copy = t[lang].part;
  const [failed, setFailed] = useState(false);
  if (failed) return <p class="text-muted">{copy.artifactsFailed}</p>;
  return (
    <figure class="footprint-preview">
      <img
        src={artifactDownloadUrl(manufacturer, mpn, "footprint-svg")}
        alt={copy.footprintAlt(mpn)}
        onError={() => setFailed(true)}
      />
      <figcaption class="text-muted">{copy.footprintLegend}</figcaption>
    </figure>
  );
}
