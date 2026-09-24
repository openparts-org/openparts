import type {
  ApiErrorBody,
  ArtifactKind,
  ArtifactsResponse,
  Device,
  Package,
  Part,
  PartSummary,
  ProvenanceResponse,
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

export const searchParts = (q: string) =>
  request<PartSummary[]>(`/v1/search?q=${encodeURIComponent(q)}`);

export const getPart = (manufacturer: string, mpn: string) =>
  request<Part>(
    `/v1/parts/${encodeURIComponent(manufacturer)}/${encodeURIComponent(mpn)}`,
  );

export const getProvenance = (manufacturer: string, mpn: string) =>
  request<ProvenanceResponse>(
    `/v1/parts/${encodeURIComponent(manufacturer)}/${encodeURIComponent(mpn)}/provenance`,
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
