// Mirrors the JSON shapes served by openparts-server (see
// openparts-server/src/routes.rs and openparts-core's Part/Device/Package
// structs). All ID newtypes are #[serde(transparent)] over String, so on
// the wire they are plain strings.

export type ExistenceStatus =
  | "unverified"
  | "probable"
  | "verified"
  | "historical_verified"
  | "rejected";

export type LifecycleStatus =
  | "pre_release"
  | "active"
  | "nrnd"
  | "eol_announced"
  | "obsolete"
  | "unknown";

export interface PartSummary {
  manufacturer: string;
  mpn: string;
  existence: ExistenceStatus;
  lifecycle: LifecycleStatus;
}

export interface Part {
  schema_version: string;
  kind: string;
  id: string;
  manufacturer: string;
  mpn: string;
  device: string;
  package: string;
  existence: { status: ExistenceStatus };
  lifecycle: { status: LifecycleStatus; replacement: string[] };
  sources: string[];
  provenance: ProvenanceMap;
}

export interface Dimension {
  nominal?: number;
  min?: number;
  max?: number;
  unit: string;
}

export interface Pin {
  name: string;
  type: string;
  alternate_functions: string[];
}

export interface DeviceRevision {
  manufacturer_revision: string;
  detection: unknown[];
  overrides: { pins: Record<string, Partial<Pin>> };
}

export interface Device {
  schema_version: string;
  kind: string;
  id: string;
  manufacturer: string;
  family?: string;
  // keyed by pin-number STRING, e.g. "1", "A1", "EP"
  pins: Record<string, Pin>;
  revisions: Record<string, DeviceRevision>;
  provenance: ProvenanceMap;
}

export interface PackageDimensions {
  body_width: Dimension;
  body_length: Dimension;
  body_height?: Dimension;
  exposed_pad?: { width: Dimension; length: Dimension };
}

export interface Package {
  schema_version: string;
  kind: string;
  id: string;
  family: string;
  lead_count: number;
  pitch: Dimension;
  dimensions: PackageDimensions;
  lead_layout?: number[];
  geometry?: { type: string; generator?: string };
  provenance: ProvenanceMap;
}

export interface ProvenanceEntry {
  source: string;
  page?: number;
  locator?: string;
}

// JSON-pointer-ish field path -> evidence entries
export type ProvenanceMap = Record<string, ProvenanceEntry[]>;

export interface ProvenanceResponse {
  part: ProvenanceMap;
  device: ProvenanceMap;
  package: ProvenanceMap;
}

export interface RevisionSummary {
  id: string;
  manufacturer_revision: string;
}

export interface ArtifactsResponse {
  "kicad-symbol": string;
  "kicad-footprint": string;
  step: string;
  stl: string;
}

export type ArtifactKind = "kicad-symbol" | "kicad-footprint" | "step" | "stl";

export interface DiagnosticBody {
  rule_id: string;
  severity: string;
  entity: string;
  field_path: string | null;
  message: string;
}

export interface ApiErrorBody {
  error: string;
  diagnostics?: DiagnosticBody[];
}
