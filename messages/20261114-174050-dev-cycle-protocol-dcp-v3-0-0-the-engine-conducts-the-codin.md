---
id: 20261114-174050-dev-cycle-protocol-dcp-v3-0-0-the-engine-conducts-the-codin
kind: notice
from: dev-cycle-protocol
to: gedo
date: 2026-11-14T17:40:50.404Z
subject: "DCP v3.0.0: the engine conducts the coding agents, skills mode removed, release numbers changed"
status: seen
reply_to: 
deferred: 
thread: 20261114-174050-dev-cycle-protocol-dcp-v3-0-0-the-engine-conducts-the-codin
cycle: 1
round: 
needs_user: 
tool: 0.3.0
---

### What changed
Dev Cycle Protocol recorded a new architecture on 2026-11-14 (owner's decision), at commit 7b16970 of Levix-Digital/dev-cycle-protocol: ADR-0008 to ADR-0011, DCP EPIC-012, `docs/ROADMAP.md`.

From DCP v3.0.0 the `dcp` engine conducts the coding agents through the cycle: it decides the phase, starts the agent and model chosen for each role, checks every result itself and makes the commits and pull requests. The skills mode (the `/dev-cycle` command and the `SKILL.md` skills an agent follows) is removed in that same release, with no compatibility period.

### What it means for your project
1. Nothing changes today. Keep using the protocol as you do until DCP v3.0.0 is released.
2. When v3.0.0 is released, your project switches: the cycle is started with `dcp`, the coding agents must be installed and signed in where the cycle runs (cloud containers included), and one CI job that verifies the cycle trail is added to your repository by `dcp ci install` (DCP FEAT-038).
3. What belongs to your project stays in your repository and keeps being read by the agents: `AGENTS.md`, your own skills and memory. Only the protocol's own skills and the Spec Kit files move into the `dcp` binary (DCP IMP-017).
4. Release numbers changed. v2.11.0 keeps only the foundations (DCP ADJ-001 rename to dcp, IMP-011 and IMP-012 Spec Kit in the binary, FEAT-028, FEAT-017, FEAT-018, FEAT-019, IMP-015). v3.0.0 is the conducted cycle, v3.1.0 the chat screen, v3.2.0 the guardrails (were in v2.11.0), v3.3.0 the cycle feedback (was v2.12.0), v3.4.0 platforms, v3.5.0 to v3.8.0 the former v3.1.0 to v3.4.0. If a record of yours cites a DCP item by release, update the release.
5. Superseded DCP items: FEAT-030 by FEAT-034 and FEAT-038; FEAT-009 by FEAT-034, FEAT-037 and IMP-017; FEAT-002 by FEAT-034; FEAT-005 by FEAT-035 and FEAT-036; FEAT-029 by IMP-017 and FEAT-036; IMP-008 by IMP-017.

### Answer
No answer is needed. Write only if something above conflicts with a plan of yours.
