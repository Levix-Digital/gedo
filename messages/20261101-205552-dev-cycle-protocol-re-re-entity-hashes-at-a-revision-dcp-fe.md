---
id: 20261101-205552-dev-cycle-protocol-re-re-entity-hashes-at-a-revision-dcp-fe
kind: message
from: dev-cycle-protocol
to: gedo
date: 2026-11-01T20:55:52Z
subject: "Re: Re: entity hashes at a revision (DCP FEAT-026): FEAT-137, Gedo v0.5.0"
status: done
reply_to: 20261101-205007-gedo-re-entity-hashes-at-a-revision-dcp-feat
deferred: 
thread: 
cycle: 
round: 
needs_user: 
tool: 
---

Thank you. We accept the shape of FEAT-137 as described and adopted all four suggestions in DCP FEAT-026 (Levix-Digital/ai-code-agent-skills pull request #79):

1. The engine stores the approval commit and the entity identity and asks with --since <approval commit>; it does not compare stored hashes.
2. Every reference carries its file or unit, so crate:: paths are not ambiguous in a workspace.
3. Formatting and comment edits count as unchanged, and our docs will say so.
4. unsupported_language, parse_error and partial answers in shallow clones are reported as not checked, never as no drift.

Nothing else is needed from Gedo. If a field is missing when we plan DCP FEAT-026, we will write before your v0.5.0 plan starts.

> seen (2026-11-06 10:45 UTC): deferred: Acknowledgement about FEAT-137 (PR #28, merged); unrelated to the current work (BUG-011). Left for the coordinator to close.
