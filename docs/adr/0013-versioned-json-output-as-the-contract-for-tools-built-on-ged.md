---
id: 13
title: Versioned JSON output as the contract for tools built on Gedo
status: accepted
date: '2026-10-25'
version: v0.4.0
subsystems: [surfaces, events]
supersedes: []
superseded_by: []
amends: []
amended_by: []
---

# ADR-0013: Versioned JSON output as the contract for tools built on Gedo

## Context

Other tools will read Gedo's output instead of linking its code: the dev-cycle protocol (its ADR-0004 integrates only through Gedo's command line with `--json`), Mission Control (the paid Levix team dashboard, outside this repository) and the GitHub Action. FEAT-088 asks for a documented, versioned schema, but no rule says what a version means, how a reader finds what an installed Gedo can do, or which exit codes it can rely on. The embeddable library API (FEAT-132) comes only after v1.0.0, so until then the command line is the only integration surface. The dev-cycle protocol needs nine commands (`doctor`, `radar`, `delta`, `trace`, `check contracts`, `version`, `scope`, `stream` and the `--label` filter), agreed with Levix on 2026-10-25.

## Decision

- **Envelope.** Every `--json` output is one JSON object with `schema` (the command's schema name, for example `gedo.radar`) and `schema_version` (`MAJOR.MINOR`), then the command's fields.
- **Compatibility.** MINOR rises when optional fields are added; readers ignore fields they do not know. MAJOR rises when a field is removed, renamed or changes meaning. A new MAJOR lives next to the previous one for at least one minor release of Gedo, chosen with `--json-schema <major>`.
- **Capabilities.** `gedo version --json` returns the Gedo version and a list of capabilities (`radar`, `delta`, `check-contracts`, `scope`, `agent-streams`, ...) with the schema MAJOR of each. Tools check capabilities, not version numbers.
- **Schemas in the repository.** Each schema is a JSON Schema file `docs/schemas/<command>.v<MAJOR>.json`, and snapshot tests pin the output of every command against it. From v1.0.0, breaking a MAJOR needs an ADR.
- **Exit codes.** 0 success with nothing to report, 1 findings (a collision, a contract break, an overlap), 2 usage error, above 2 internal failure. Output on findings is still complete JSON.
- **Labels and authors.** Events and streams carry opaque labels and an author kind (FEAT-063, FEAT-068). Events recorded inside a stream inherit its author and labels; outside a stream, `GEDO_AGENT` and `GEDO_LABELS` name them. Gedo stores, returns and filters labels (`--label key=value`) without interpreting them. The prefix `gedo.` is reserved for Gedo; the dev-cycle protocol uses `dc.` (`dc.plan`, `dc.milestone`, `dc.role`, `dc.gate`). Limits: 16 labels per event or stream, keys up to 64 bytes, values up to 256 bytes, UTF-8.
- **Ownership.** Gedo detects and stores (events, streams, hidden agent checkouts, intents, collisions, contract breaks, the dependency graph); the tools on top decide what to do with it (plans, milestones, gates, scheduling, build locks). Gedo never reads a tool's configuration, and tools never read `refs/gedo/*` directly.

## Alternatives Considered

- **Unversioned JSON, stable by convention**: cheaper now, but every reader breaks silently on the first rename, and three readers are already planned.
- **Version checks by Gedo release number**: simple, but couples readers to the release train and fails when a capability moves between releases.
- **A library API before v1.0.0**: tighter and faster, but freezes internal crates while the core is still changing; kept for after v1.0.0 (FEAT-132).
- **Labels with meaning inside Gedo (plan, milestone as fields)**: easier filtering, but ties Gedo's format, frozen at v1.0.0, to one tool's vocabulary.

## Consequences

- FEAT-088 delivers the envelope, `gedo version --json`, the schema files and their snapshot tests in v0.4.0; FEAT-068, FEAT-074, FEAT-079 and FEAT-084 add their `--json` and `--label` parts; FEAT-133 (`gedo scope`) arrives in v0.5.0 with agent streams.
- Readers fail safe: a missing capability or an unknown MAJOR means the reader skips that use of Gedo; the dev-cycle protocol records such cases as `interface` feedback.
- Verify with the snapshot tests and with the dev-cycle protocol's contract tests on recorded outputs of a released Gedo.
