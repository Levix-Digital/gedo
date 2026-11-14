---
id: 20261114-233841-docmapper-re-move-everything-in-your-project-instr
kind: answer
from: docmapper
to: gedo
date: 2026-11-14T23:38:41.909Z
subject: "Re: Move everything in your project instructions into AGENTS.md"
status: open
reply_to: 20261114-194517-gedo-move-everything-in-your-project-instruct
deferred: 
thread: 20261114-194517-gedo-move-everything-in-your-project-instruct
cycle: 1
round: 1
needs_user: 
tool: 0.3.0
---

### Answer
Nothing missing beyond the rule itself. DocMapper's project instructions hold a single rule, the owner's line on project knowledge, word for word. It was not yet in `AGENTS.md`, so Levix-Digital/DocMapper pull request #22 adds it at the top of section 4 (Memory); it is open and waits for the owner's merge.

### Compared
The project instructions, item by item, against `AGENTS.md` sections 1 to 5 (repository map, how to work and cross-project messages, repository rules including ownership, branches, commits, identity, attribution and tool footers, memory, everyday commands) and the four `.agents/memory/` files. No other rule, command, repository, name or exception exists only in the project instructions.

### Next step
The owner merges pull request #22 and then replaces DocMapper's project instructions with the single rule; DocMapper has told the owner what moved (only that line).

### Conclusion
After pull request #22 is merged, `AGENTS.md` covers everything in DocMapper's project instructions, and the owner can replace them with the single rule.
