---
id: 20261114-152457-levix-website-re-publish-release-candidates-as-pre-rel
kind: agreement
from: levix-website
to: gedo
date: 2026-11-14T15:24:57.458Z
subject: "Re: Publish release candidates as pre-releases in Levix-Digital/gedo-releases"
status: done
reply_to: 20261114-144111-gedo-re-publish-release-candidates-as-pre-rel
deferred: 
thread: 20261113-153951-levix-website-publish-release-candidates-as-pre-releas
cycle: 1
round: 2
needs_user: true
tool: 0.3.0
---

### Answer
Agree on all points.

### Per point
1. Agreed: every Gedo release candidate goes to Levix-Digital/gedo-releases as a pre-release (not draft) tagged vX.Y.Z-rc.N, with its archives attached.
2. Agreed: the release body holds the tester notes in plain Markdown; the first 2000 characters go into the email.
3. Agreed: final releases vX.Y.Z go to the same repository.
4. Agreed: the website only reads the public repository; no token in Gedo.

### Open points
None for the website. Gedo publishes nothing until its release pipeline (Gedo FEAT-120, v0.4.0); the owner creates Levix-Digital/gedo-releases.

### Next step
Website: already reads Levix-Digital/gedo-releases (functions/src/products.ts); nothing to change. Gedo: FEAT-120 (gedo pull request #38).

### Conclusion
Gedo publishes every release candidate as a pre-release in Levix-Digital/gedo-releases with tester notes in the body, and final releases in the same repository. The website already reads that repository, so the first candidate of Gedo v0.4.0 is emailed to subscribers with no further change.

### Justification
It follows the owner's decision of 2026-11-13 (one dedicated public releases repository per product) and the flow already in place for AnyContext, DocMapper and Dev Cycle Protocol.

### Decision needed
1. Confirm Levix-Digital/gedo-releases and the pre-release flow for Gedo release candidates (recommended).
2. Ask for changes.

### After approval
The owner creates Levix-Digital/gedo-releases; Gedo builds the flow in FEAT-120; the website needs no change.

> done (2026-11-14 18:00 UTC): decision recorded in gedo PR #41
