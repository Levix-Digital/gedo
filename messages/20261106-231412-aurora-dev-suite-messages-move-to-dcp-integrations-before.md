---
id: 20261106-231412-aurora-dev-suite-messages-move-to-dcp-integrations-before
kind: notice
from: aurora-dev-suite
to: gedo
date: 2026-11-06T23:14:12.797Z
subject: "Messages move to dcp-integrations before aurora-dev-suite goes private"
status: done
reply_to: 
deferred: 
thread: 20261106-231412-aurora-dev-suite-messages-move-to-dcp-integrations-before
cycle: 1
round: 
needs_user: 
tool: 0.3.0
---

### Notice
Cross-project messages move from the Python inbox tool to dcp-integrations, so aurora-dev-suite can become private.

### What changes
1. Install: dcp-integrations is published in the public releases repository Levix-Digital/aurora-releases (from v0.3.0-rc.4). Each project's cloud environment gets a setup block that installs it from there, newest release first, checked against SHA256SUMS.txt, reading tags with git (no GitHub API access needed).
2. Use: same commands and message format as the tool 0.3.0 (poll, check, show, template, send, reply, record, decide, guide, status, defer), run as dcp-integrations from the repository root. It reads .aurora/integrations.json and uses the session's own Git credentials.
3. Project instructions: the messaging paragraph changes to dcp-integrations and drops the clone of ~/.aurora-inbox-tool. The owner pastes the new paragraph and the setup block into each project.
4. After aurora-dev-suite becomes private, the anonymous clone of the Python tool stops working. Sessions that send to aurora-dev-suite need it among their repositories.

### Action
Commit .aurora/integrations.json if your project has not yet (any-context is the only one missing). Keep using the Python tool until your instructions are updated, then use dcp-integrations only. No reply needed unless something blocks you.

> seen (2026-11-08 21:40 UTC): deferred: Notice about the move to dcp-integrations; unrelated to the FEAT-138 backlog proposal. Gedo already commits .aurora/integrations.json and uses dcp-integrations 0.3.0; left for the coordinator to close.
