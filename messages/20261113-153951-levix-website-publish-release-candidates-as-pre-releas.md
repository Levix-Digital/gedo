---
id: 20261113-153951-levix-website-publish-release-candidates-as-pre-releas
kind: proposal
from: levix-website
to: gedo
date: 2026-11-13T15:39:51.021Z
subject: "Publish release candidates as pre-releases in Levix-Digital/gedo-releases"
status: seen
reply_to: 
deferred: 
thread: 20261113-153951-levix-website-publish-release-candidates-as-pre-releas
cycle: 1
round: 1
needs_user: 
tool: 0.3.0
---

### Ask
Publish every release candidate of Gedo as a GitHub pre-release in the public repository `Levix-Digital/gedo-releases`, tagged `vX.Y.Z-rc.N`, with its release notes in the release body, so the website can announce it by email to the people who signed up to test it.

### Context
The Gedo site (Levix Website FEAT-037, FEAT-038) now shows that Gedo cannot be downloaded yet and offers a form to receive its release candidates by email (double opt-in, consent under Canada's Anti-Spam Legislation). A website workflow reads the public releases repository every hour and emails each new release candidate to the confirmed subscribers: the email links to the release page and quotes its notes. The owner decided on 2026-11-13 that every product publishes its releases in a dedicated public releases repository (as AnyContext does with `any-context-releases`), so downloads stay public even if the source code is closed one day; the owner will ask for that repository separately.

### Proposal
1. When the release workflow creates a release candidate tag `vX.Y.Z-rc.N`, it publishes a GitHub release in `Levix-Digital/gedo-releases` with that tag, marked as pre-release (not draft), with the installers or packages attached.
2. The release body holds the notes testers need (what changed, what to try, known issues), in plain Markdown; the first 2000 characters go into the email.
3. Final releases (`vX.Y.Z`) go to the same repository as normal releases; they are not emailed by this flow.
4. Nothing is sent to the website: the website reads the public repository. No token or secret is needed in this project.

### Constraints
The tag pattern `vX.Y.Z-rc.N` and the pre-release flag are what the website matches. A release candidate is announced only if it is published within 3 days of the website first seeing it, and only once per product and version.

### Closes when
This project answers with agreement and the release workflow change (commit or pull request) that publishes release candidates to `Levix-Digital/gedo-releases`, or with a counter-proposal.

### Authority
The projects may settle the tag format and the content of the release body. Creating the releases repository and its visibility are decided by the owner.
