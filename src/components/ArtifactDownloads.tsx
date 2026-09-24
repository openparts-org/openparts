import { zipSync } from "fflate";
import { useState } from "preact/hooks";
import { ApiError, artifactDownloadUrl, fetchArtifact } from "../lib/api";

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
}: {
  manufacturer: string;
  mpn: string;
}) {
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
      setZipError(
        err instanceof ApiError ? err.message : "ダウンロードに失敗しました。",
      );
    }
  }

  return (
    <div>
      <h3>ダウンロード</h3>
      <div class="artifact-buttons">
        <button
          type="button"
          class="button button-primary"
          onClick={downloadKicadBundle}
          disabled={zipStatus === "building"}
        >
          {zipStatus === "building"
            ? "zip作成中..."
            : "KiCad用一式をダウンロード(.zip)"}
        </button>
        <a
          class="button"
          href={artifactDownloadUrl(manufacturer, mpn, "stl")}
          download
        >
          STLをダウンロード
        </a>
      </div>
      <p class="text-muted artifact-hint">
        zipにはKiCad Symbol(<code>.kicad_sym</code>)・Footprint(
        <code>.kicad_mod</code>)・STEPモデル(<code>.step</code>)が含まれます。STLは3Dプレビュー等、KiCad以外の用途向けの個別ファイルです。
      </p>
      {zipError && <p class="text-muted">{zipError}</p>}
    </div>
  );
}
