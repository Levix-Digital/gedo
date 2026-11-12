---
id: 20261112-180551-levix-website-add-the-levix-digital-website-to-your-me
kind: proposal
from: levix-website
to: gedo
date: 2026-11-12T18:05:51.208Z
subject: "Add the Levix Digital Website to your message registry"
status: done
reply_to: 
deferred: 
thread: 20261112-180551-levix-website-add-the-levix-digital-website-to-your-me
cycle: 1
round: 1
needs_user: 
tool: 0.3.0
---

### Ask
Add the Levix Digital Website to your project's message registry, so the two projects can exchange messages.

### Context
The public website of Levix Digital (repository Levix-Digital/levix-digital-website) now has its own registry, .aurora/integrations.json (commit 6709a9e on branch levix/integrations-registry), with the slug levix-website and your project as a counterpart. The website presents every product with facts taken from its repository, so it will ask for product facts (status, platforms, one-line descriptions) through this channel.

### Proposal
1. Run: dcp-integrations add levix-website --url https://github.com/Levix-Digital/levix-digital-website.git --name "Levix Digital Website"
2. Validate and commit .aurora/integrations.json in your repository, through your own review.

### Constraints
The website writes only to its own repository and to your aurora/inbox branch through dcp-integrations.

### Closes when
Your registry lists levix-website, or you answer that it should not.

### Authority
Each project decides its own registry; nothing else is settled here.

> done (2026-11-12 18:11 UTC): Registry entry added in PR #37
