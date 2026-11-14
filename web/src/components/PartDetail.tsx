import { useEffect, useState } from "preact/hooks";
import {
  ApiError,
  getDevice,
  getPackage,
  getPart,
  getRevisions,
  listArtifacts,
} from "../lib/api";
import { localizedHref } from "../lib/base";
import { t, type Lang } from "../lib/i18n";
import type { Device, Package, Part, RevisionSummary } from "../lib/types";
import ArtifactDownloads from "./ArtifactDownloads";
import ModelViewer from "./ModelViewer";
import PackageDimensions from "./PackageDimensions";
import PinTable from "./PinTable";

interface Loaded {
  part: Part;
  device: Device;
  pkg: Package;
  revisions: RevisionSummary[];
}

function existenceBadgeClass(status: Part["existence"]["status"]): string {
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

function lifecycleBadgeClass(status: Part["lifecycle"]["status"]): string {
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

function getQueryParams(): { manufacturer: string; mpn: string } | null {
  if (typeof window === "undefined") return null;
  const params = new URLSearchParams(window.location.search);
  const manufacturer = params.get("manufacturer");
  const mpn = params.get("mpn");
  if (!manufacturer || !mpn) return null;
  return { manufacturer, mpn };
}

export default function PartDetail({ lang }: { lang: Lang }) {
  const copy = t[lang].part;
  const [loading, setLoading] = useState(true);
  const [notFound, setNotFound] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [data, setData] = useState<Loaded | null>(null);
  const [artifactsError, setArtifactsError] = useState<string | null>(null);

  useEffect(() => {
    const ids = getQueryParams();
    if (!ids) {
      setLoading(false);
      setNotFound(true);
      return;
    }

    let cancelled = false;
    const { manufacturer, mpn } = ids;

    async function load() {
      try {
        const part = await getPart(manufacturer, mpn);
        const [device, pkg, revisions] = await Promise.all([
          getDevice(part.device),
          getPackage(part.package),
          getRevisions(manufacturer, mpn),
        ]);
        if (cancelled) return;
        setData({ part, device, pkg, revisions });

        // Artifact generation can 422 independently of the rest of the
        // part data (e.g. a missing dimension) -- don't let that hide
        // everything else on the page.
        try {
          await listArtifacts(manufacturer, mpn);
        } catch (err) {
          if (!cancelled) {
            setArtifactsError(
              err instanceof ApiError ? err.message : copy.artifactsFailed,
            );
          }
        }
      } catch (err) {
        if (cancelled) return;
        if (err instanceof ApiError && err.status === 404) {
          setNotFound(true);
        } else {
          setErrorMessage(
            err instanceof ApiError ? err.message : copy.loadFailed,
          );
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    }

    load();
    return () => {
      cancelled = true;
    };
  }, []);

  if (loading) return <p class="text-muted">{copy.loading}</p>;

  if (notFound) {
    return (
      <div>
        <h1>{copy.notFoundTitle}</h1>
        <p class="text-muted">
          {copy.notFoundBefore}
          <a href={localizedHref(lang, "search")}>{copy.notFoundLink}</a>
          {copy.notFoundAfter}
        </p>
      </div>
    );
  }

  if (errorMessage || !data) {
    return <p class="text-muted">{errorMessage ?? copy.loadFailed}</p>;
  }

  const { part, device, pkg, revisions } = data;

  return (
    <div>
      <h1>
        {part.mpn} <span class="text-muted">/ {part.manufacturer}</span>
      </h1>
      <div class="result-badges">
        <span class={existenceBadgeClass(part.existence.status)}>
          {part.existence.status}
        </span>
        <span class={lifecycleBadgeClass(part.lifecycle.status)}>
          {part.lifecycle.status}
        </span>
      </div>

      <section class="section">
        <ArtifactDownloads
          manufacturer={part.manufacturer}
          mpn={part.mpn}
          lang={lang}
        />
        {artifactsError && <p class="text-muted">{artifactsError}</p>}
      </section>

      <section class="section">
        <h3>{copy.preview3d}</h3>
        <ModelViewer manufacturer={part.manufacturer} mpn={part.mpn} lang={lang} />
      </section>

      <section class="section">
        <PinTable
          device={device}
          manufacturer={part.manufacturer}
          mpn={part.mpn}
          lang={lang}
        />
      </section>

      {revisions.length > 0 && (
        <section class="section">
          <h3>{copy.revisions}</h3>
          <ul>
            {revisions.map((rev) => (
              <li key={rev.id}>
                {rev.id} ({rev.manufacturer_revision})
              </li>
            ))}
          </ul>
        </section>
      )}

      <section class="section">
        <PackageDimensions pkg={pkg} lang={lang} />
      </section>
    </div>
  );
}
