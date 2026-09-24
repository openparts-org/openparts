import { artifactDownloadUrl } from "../lib/api";
import type { ArtifactKind } from "../lib/types";

const ARTIFACT_LABELS: Record<ArtifactKind, string> = {
  "kicad-symbol": "KiCad Symbol",
  "kicad-footprint": "KiCad Footprint",
  step: "STEP",
  stl: "STL",
};

export default function ArtifactDownloads({
  manufacturer,
  mpn,
}: {
  manufacturer: string;
  mpn: string;
}) {
  const kinds = Object.keys(ARTIFACT_LABELS) as ArtifactKind[];

  return (
    <div>
      <h3>CAD成果物</h3>
      <div class="artifact-buttons">
        {kinds.map((kind) => (
          <a
            key={kind}
            class="button"
            href={artifactDownloadUrl(manufacturer, mpn, kind)}
            download
          >
            {ARTIFACT_LABELS[kind]}をダウンロード
          </a>
        ))}
      </div>
    </div>
  );
}
