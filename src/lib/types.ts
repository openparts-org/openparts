// The JSON documents openparts-server returns. Generated from the Rust
// types by scripts/gen-api-types.sh into ./api -- never edit those
// files; regenerate them.

export type { ComponentCategory } from "./api/ComponentCategory";
export type { Device } from "./api/Device";
export type { DeviceRevision } from "./api/DeviceRevision";
export type { DiagnosticBody } from "./api/DiagnosticBody";
export type { Dimension } from "./api/Dimension";
export type { ErrorBody as ApiErrorBody } from "./api/ErrorBody";
export type { ExistenceStatus } from "./api/ExistenceStatus";
export type { LifecycleStatus } from "./api/LifecycleStatus";
export type { Package } from "./api/Package";
export type { PackageDimensions } from "./api/PackageDimensions";
export type { Part } from "./api/Part";
export type { PartSummary } from "./api/PartSummary";
export type { Pin } from "./api/Pin";
export type { ProvenanceEntry } from "./api/ProvenanceEntry";
export type { ProvenanceResponse } from "./api/ProvenanceResponse";
export type { RevisionSummary } from "./api/RevisionSummary";
export type { ArtifactsResponse } from "./api/ArtifactsResponse";

import type { ArtifactsResponse } from "./api/ArtifactsResponse";
import type { ProvenanceEntry } from "./api/ProvenanceEntry";

/// JSON-pointer field path -> evidence entries.
export type ProvenanceMap = Partial<Record<string, ProvenanceEntry[]>>;

export type ArtifactKind = keyof ArtifactsResponse;
