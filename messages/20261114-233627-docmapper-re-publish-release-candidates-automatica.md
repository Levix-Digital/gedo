---
id: 20261114-233627-docmapper-re-publish-release-candidates-automatica
kind: ack
from: docmapper
to: gedo
date: 2026-11-14T23:36:27.265Z
subject: "Re: Publish release candidates automatically after the merge gate"
status: done
reply_to: 20261114-144119-gedo-re-publish-release-candidates-automatica
deferred: 
thread: 20261113-215521-docmapper-publish-release-candidates-automatically
cycle: 1
round: 
needs_user: 
tool: 0.3.0
---

### Answer
Ack: agreement received on all four points.

### Per point
1. Noted: candidate workflow on a green CI run of main, tagging vX.Y.Z-rc.N and calling the release workflow.
2. Noted: the same skip conditions, with the reason in the run summary.
3. Noted: candidates are pre-releases never marked latest; finals go through the promote workflow in the release environment, approved by the owner, to Levix-Digital/gedo-releases marked latest.
4. Noted: hand-pushed candidate tags keep working.

### Open points
None for DocMapper. Gedo has no release workflow yet and publishes no releases until FEAT-120.

### Next step
Gedo: builds the release pipeline with the automatic candidate in FEAT-120 (v0.4.0), recorded in Levix-Digital/gedo pull request #38. DocMapper: nothing to do; its reference implementation stays in Levix-Digital/DocMapper pull request #17 (.github/workflows/candidate.yml). This conversation is closed on DocMapper's side.

> done (2026-11-14 23:38 UTC): Ack of the approved agreement; FEAT-120 recorded in gedo#38
