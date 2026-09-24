import { zipSync } from "fflate";
import { useState } from "preact/hooks";
import { ApiError, artifactDownloadUrl, fetchArtifact } from "../lib/api";
import { t, type Lang } from "../lib/i18n";

type ZipStatus = "idle" | "building" | "error";

function triggerDownload(blob: Blob, filename: string) {
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}

export default function ArtifactDownloads({
  manufacturer,
  mpn,
  lang,
}: {
  manufacturer: string;
  mpn: string;
  lang: Lang;
}) {
  const copy = t[lang].part;
  const [zipStatus, setZipStatus] = useState<ZipStatus>("idle");
  const [zipError, setZipError] = useState<string | null>(null);

  async function downloadKicadBundle() {
    setZipStatus("building");
    setZipError(null);
    try {
      // KiCad needs all three of these together (symbol + footprint +
      // the STEP model the footprint's 3D viewer points at) to actually
      // be usable, so bundle them into one zip instead of three separate
      // downloads.
      const [symbol, footprint, step] = await Promise.all([
        fetchArtifact(manufacturer, mpn, "kicad-symbol"),
        fetchArtifact(manufacturer, mpn, "kicad-footprint"),
        fetchArtifact(manufacturer, mpn, "step"),
      ]);

      const zipped = zipSync({
        [`${mpn}.kicad_sym`]: symbol,
        [`${mpn}.kicad_mod`]: footprint,
        [`${mpn}.step`]: step,
      });

      triggerDownload(
        new Blob([zipped], { type: "application/zip" }),
        `${mpn}-kicad.zip`,
      );
      setZipStatus("idle");
    } catch (err) {
      setZipStatus("error");
      setZipError(err instanceof ApiError ? err.message : copy.zipFailed);
    }
  }

  return (
    <div>
      <h3>{copy.download}</h3>
      <div class="artifact-buttons">
        <button
          type="button"
          class="button button-primary"
          onClick={downloadKicadBundle}
          disabled={zipStatus === "building"}
        >
          {zipStatus === "building" ? copy.zipBuilding : copy.zipButton}
        </button>
        <a
          class="button"
          href={artifactDownloadUrl(manufacturer, mpn, "stl")}
          download
        >
          {copy.stlButton}
        </a>
      </div>
      {zipError && <p class="text-muted">{zipError}</p>}
    </div>
  );
}
