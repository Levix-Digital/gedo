---
name: environment
description: Machines, cloud and CI: versions, blocked hosts, setup quirks. Read when a command, install or network call fails or behaves differently between machines.
updated: 2026-11-14
---

- 2026-10-24: Performance budgets are measured on the reference machine (the owner's Windows 11 desktop, i7-12700KF, Defender real-time protection on). Container and CI numbers are informational. Running benchmarks there needs the owner's OK and a remote session on that machine, where `cargo bench` may have to be allowed in the tool's permissions first.
- 2026-10-24: The GitHub GraphQL API is blocked in cloud sessions; use the REST API. Actions logs download from blob hosts that are blocked; read job logs through the API's job log call.
- 2026-10-25: The Dev Cycle Protocol and `dcp-integrations` are installed by the cloud environment's setup script; if `/dev-cycle` is missing in a session, install them from the local clone as in section 2 of AGENTS.md.
- 2026-10-31: Windows Defender scanning dominates small-file writes on Windows (new loose objects can stall 50 to 280 ms); excluding `.git` or using a Dev Drive removes most of it.
