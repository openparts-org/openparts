# OpenParts agent skills

Procedures for AI agents working on OpenParts (Architecture
Specification section 18, Agent Ingestion Specification). Each skill is
a directory with a `SKILL.md` (Claude Code skill format: YAML front
matter with `name` and `description`, then the procedure).

| Skill | Use |
|---|---|
| [`add-part`](add-part/SKILL.md) | Add parts (Part/Device/Package/Source) to openparts-data from datasheets, with provenance, and verify footprints and 3D models generate. |

Skills share the Canonical Data Schema and the validator as their
interface; their output is always a candidate change for review, never
a direct write to production data.
