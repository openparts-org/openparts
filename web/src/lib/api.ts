import type {
  ApiErrorBody,
  ArtifactKind,
  ArtifactsResponse,
  ComponentCategory,
  Device,
  Package,
  Part,
  PartSummary,
  RevisionSummary,
} from "./types";

const API_BASE = import.meta.env.PUBLIC_OPENPARTS_API_BASE ?? "/api";

export class ApiError extends Error {
  status: number;
  body: ApiErrorBody;

  constructor(status: number, body: ApiErrorBody) {
    super(body.error);
    this.status = status;
    this.body = body;
  }
}

async function request<T>(path: string): Promise<T> {
  const res = await fetch(`${API_BASE}${path}`);
  if (!res.ok) {
    const body: ApiErrorBody = await res
      .json()
      .catch(() => ({ error: res.statusText }));
    throw new ApiError(res.status, body);
  }
  return res.json();
}

export const searchParts = (q: string, category?: ComponentCategory) => {
  const params = new URLSearchParams();
  if (q) params.set("q", q);
  if (category) params.set("category", category);
  return request<PartSummary[]>(`/v1/search?${params.toString()}`);
};

export const getPart = (manufacturer: string, mpn: string) =>
  request<Part>(
    `/v1/parts/${encodeURIComponent(manufacturer)}/${encodeURIComponent(mpn)}`,
  );

export const getRevisions = (manufacturer: string, mpn: string) =>
  request<RevisionSummary[]>(
    `/v1/parts/${encodeURIComponent(manufacturer)}/${encodeURIComponent(mpn)}/revisions`,
  );

export const listArtifacts = (manufacturer: string, mpn: string) =>
  request<ArtifactsResponse>(
    `/v1/parts/${encodeURIComponent(manufacturer)}/${encodeURIComponent(mpn)}/artifacts`,
  );

// device/package ids are wildcard-captured (/v1/devices/*id) and may
// themselves contain literal "/" (e.g. "raspberrypi/RP2040") -- do not
// encode the whole id, or the route stops matching.
export const getDevice = (id: string) => request<Device>(`/v1/devices/${id}`);

export const getPackage = (id: string) =>
  request<Package>(`/v1/packages/${id}`);

export const artifactDownloadUrl = (
  manufacturer: string,
  mpn: string,
  kind: ArtifactKind,
) =>
  `${API_BASE}/v1/parts/${encodeURIComponent(manufacturer)}/${encodeURIComponent(mpn)}/artifacts/${kind}`;

export async function fetchArtifact(
  manufacturer: string,
  mpn: string,
  kind: ArtifactKind,
): Promise<Uint8Array> {
  const res = await fetch(artifactDownloadUrl(manufacturer, mpn, kind));
  if (!res.ok) {
    const body: ApiErrorBody = await res
      .json()
      .catch(() => ({ error: res.statusText }));
    throw new ApiError(res.status, body);
  }
  return new Uint8Array(await res.arrayBuffer());
}
