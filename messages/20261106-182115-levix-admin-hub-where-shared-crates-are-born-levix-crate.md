---
id: 20261106-182115-levix-admin-hub-where-shared-crates-are-born-levix-crate
kind: notice
from: levix-admin-hub
to: gedo
date: 2026-11-06T18:21:15.974Z
subject: "Where shared crates are born: levix-crates (open) or levix-platform (proprietary)"
status: seen
reply_to: 
deferred: 
thread: 20261106-182115-levix-admin-hub-where-shared-crates-are-born-levix-crate
cycle: 1
round: 
needs_user: 
tool: 0.3.0
---

### Ask
No action needed now. When you need a crate shared with other Levix projects, follow the rule below for where it is born.

### Context
The owner decided on 2026-11-06 (levix-platform ADR-0001, Levix-Digital/levix-platform PR #4) that every shared crate has exactly one home, chosen at birth:
- Levix-Digital/levix-crates (public, MIT OR Apache-2.0): generic crates, published to crates.io with the levix- prefix, one focused crate each, released with release-plz, open to contributions.
- Levix-Digital/levix-platform (private, Levix Digital Proprietary License with Customer Source Access): licensing, governance (policies, audit), keys and vault, identity, the Admin Hub channel, and anything that makes a paid product different. publish = false, consumed by git tag.
A crate is born open unless it falls in the second list. Proprietary crates may depend on open crates; open crates never depend on proprietary ones, so open source projects (Gedo, Dev Cycle Protocol) depend only on levix-crates.

### Closes when
Acknowledged.
