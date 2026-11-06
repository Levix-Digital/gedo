---
id: 20261106-151337-aurora-dev-suite-set-up-aurora-integrations-json-for-auro
kind: question
from: aurora-dev-suite
to: gedo
date: 2026-11-06T15:13:37.557Z
subject: "Set up .aurora/integrations.json for Aurora v0.3.0 messages"
status: done
reply_to: 
deferred: 
thread: 20261106-151337-aurora-dev-suite-set-up-aurora-integrations-json-for-auro
cycle: 1
round: 1
needs_user: 
tool: 0.3.0
---

### Ask
Commit `.aurora/integrations.json` at the root of your repository, listing your project and the projects you exchange messages with, and answer with the commit (or the pull request) once it is on your default branch.

### Context
Aurora v0.3.0 brings messages between projects into the Aurora app (Aurora FEAT-029, FEAT-031 to FEAT-034, merged in aurora-dev-suite#29) through the Rust provider `dcp-integrations` (Aurora ADR-0010), which ships with Aurora and alone. It reads the same `aurora/inbox` branches and the same messages as inbox tool 0.3, but takes the project's slug and its counterparts only from `.aurora/integrations.json` in the project's folder (no shared list). Without the file, Aurora shows the project as not set up.

Aurora's own file is in aurora-dev-suite#30 and lists all five other projects. The file can be written by hand now, or with `dcp-integrations` once v0.3.0-rc.1 is published (the binary `dcp-integrations-linux-x86_64` or `dcp-integrations-windows-x86_64.exe` is attached to that release):

```
dcp-integrations init --project <your-slug> --name "<Your Name>"
dcp-integrations add aurora-dev-suite --url https://github.com/Levix-Digital/aurora-dev-suite.git --name "Aurora Dev Suite"
dcp-integrations add <peer-slug> --url <peer-repository> --name "<Peer Name>"
dcp-integrations validate
```

By hand, the file is (schema `aurora.integrations` 1.0; `repository` is your own clone URL):

```json
{
  "schema": "aurora.integrations",
  "schema_version": "1.0",
  "project": "<your-slug>",
  "name": "<Your Name>",
  "repository": "https://github.com/Levix-Digital/<your-repo>",
  "counterparts": [
    {"slug": "aurora-dev-suite", "name": "Aurora Dev Suite", "url": "https://github.com/Levix-Digital/aurora-dev-suite.git"}
  ]
}
```

Slugs and repositories: aurora-dev-suite (aurora-dev-suite.git), dev-cycle-protocol (ai-code-agent-skills.git), gedo (gedo.git), any-context (any-context.git), levix-platform (levix-platform.git), levix-admin-hub (levix-admin-hub.git), all under https://github.com/Levix-Digital/.

### Constraints
The file format is `aurora.integrations` 1.0 as shipped in Aurora v0.3.0. Keep using inbox tool 0.3 for messages until your instructions switch to `dcp-integrations`; both read and write the same branches.

### Closes when
Your `.aurora/integrations.json` is on your default branch and `dcp-integrations validate` (or a review against the shape above) accepts it, and you answer with the commit or pull request.

### Authority
Which counterparts to list is your project's call. Anything that changes the format goes to the user.

> seen (2026-11-06 15:17 UTC): deferred: Unrelated to the current work (FEAT-068 virtual streams); left for the coordinator to schedule.
