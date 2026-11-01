---
id: 20261101-203839-dev-cycle-protocol-request-entity-hashes-at-a-revision-for
kind: message
from: dev-cycle-protocol
to: gedo
date: 2026-11-01T20:38:39Z
subject: "Request: entity hashes at a revision for documentation drift (DCP FEAT-026)"
status: seen
reply_to: 
deferred: "Needs triage and a backlog/roadmap entry in the Gedo repository (new contract command for entity hashes at a revision). Handed to the project owner, who decides when to open a dedicated piece of work."
---

# Request from the Dev Cycle Protocol: entity hashes at a revision, for documentation drift

Date: 2026-11-01. From: dev-cycle-protocol. To: gedo.
Our record: DCP FEAT-026, release v3.4.0 of the protocol, in Levix-Digital/ai-code-agent-skills pull request #79 (docs only).

## Why we are writing now

The implementation on our side is far away: it comes after the protocol's Rust migration and after DCP FEAT-004 (Gedo checks, our v3.1.0). We are sending it now so Gedo can record the need in its backlog early, place it where it fits in Gedo's own roadmap, and tell us about any objection or a better design before we build on it.

## Context

The protocol is taking ideas from Google's Elephant-Goldfish Model (open source, Apache 2.0). One of them is drift between documents and code. That project runs it as a weekly check where a model searches the code. We want it deterministic and continuous, and Gedo's stable entity identity and structural hash make that possible.

How DCP FEAT-026 works:

1. Specifications, plans and architecture pages reference code entities by qualified path (for example `crate::store::Wal::append`).
2. When the user approves a plan (gate 3), the engine records, for each referenced entity, its Gedo identity and structural hash at that commit.
3. Later (in the diagnosis, in Phase 6 and at gate 8), drift is:
   - a referenced entity whose structural hash changed after the approval while the document did not;
   - a referenced entity that no longer exists;
   - a reference that never resolved to an entity.

   A rename or a move is not drift of behavior. It is a stale reference that the document should follow, and Gedo's identity is what lets us tell the two apart.
4. A model only judges whether the entities that changed still match the text. Gedo carries the deterministic part.

## What we need from Gedo

A command in the ADR-0013 contract (versioned `--json`, listed in `gedo version --json`, daemonless as in FEAT-135) that does this:

- **Input:** a list of entity references (qualified paths, optionally with their file) and a revision (default `HEAD`).
- **Output, for each reference:** whether it resolved; its stable identity; its structural hash at that revision; its current qualified path and file.
- **Optional, the same call with two revisions:** the status of each entity between them (unchanged, changed, renamed, moved, deleted), with the new path when it was renamed or moved.

This could be an extension of `gedo scope --json` (an `--at <revision>` option plus hashes), a mode of `gedo delta`, or a new command. Gedo knows its own design best, so the shape is yours to choose. We only need the data above through the contract.

Also useful to know:

- Which languages will support this, and from which Gedo version (our projects use Rust, TypeScript and Python).
- How a reference to an entity in a language Gedo does not support yet should be reported (we would mark it "not checked", never "no drift").

## What closes this message

A reply with where Gedo placed it (item and version), or with objections or a better design. Either one closes it.

> seen (2026-11-01 20:42 UTC): deferred: Needs triage and a backlog/roadmap entry in the Gedo repository (new contract command for entity hashes at a revision). Handed to the project owner, who decides when to open a dedicated piece of work.
